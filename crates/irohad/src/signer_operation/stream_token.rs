//! Exact provider-scoped stream-token production over authoritative opaque-key operations.
//!
//! The source authenticates current custody and a fresh audit predecessor for each issuance,
//! atomically reserves that pair, and independently verifies durable receipt bytes before commit.
//! The returned bounded receipt is still an untrusted claim to the external consumer: only its
//! independent, fresh challenged BeforeRelease evidence verification may release a token.
//! Authenticated software providers use this same path. No embedded service key, observer-picked
//! trust, or alternate signing/recovery path exists here.
//! The owner-only software-credential constructor assembles this producer over one injected
//! source; it does not qualify that source or activate generic signer dispatch.
//! TODO: supply and qualify the authoritative journal/finality, observer and configured runtime
//! adapters; injected source/provider tests exercise races and persistence, not a deployed service.
//! TODO: the genuine state/provider adapters must reconstruct the same window and request digest
//! from exact canonical body bytes plus independently pinned custody before admission. The generic
//! reservation interface currently exposes the bound intent digest, not a separately authenticated
//! prepared time window; caller-supplied times alone cannot authorize retention or provider use.

use super::journal::{
    SignerReceiptJournalErrorV1, SignerReceiptJournalReaderV1, SignerReceiptJournalV1,
    SignerReceiptPurposeV1,
};
use super::*;
use iroha_data_model::sorafs::stream_token_authority::StreamTokenReviewedV1;
use sorafs_manifest::{
    StreamTokenBodyV1, StreamTokenV1,
    signer::{
        protocol::{
            SignerOperationSignatureV1, signer_operation_message_digest_v1,
            signer_operation_signatures_digest_v1,
        },
        receipt::SignerOperationProvenanceV1,
        stream_token::{
            SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1, SignerStreamTokenExpectedV1,
            SignerStreamTokenReceiptErrorV1, SignerStreamTokenReceiptV1,
            SignerStreamTokenRequestV1, prepare_stream_token_signing_payload_v1,
            signer_stream_token_audit_v1, signer_stream_token_response_digest_v1,
            stream_token_binding_digest_v1, validate_stream_token_signatures_v1,
        },
    },
};
use std::path::Path;
use zeroize::Zeroize as _;

/// Fixed public failure classes; no request, filesystem path or backend detail is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerStreamTokenErrorV1 {
    /// Exact canonical body, purpose, token time or receipt binding was rejected.
    Receipt(SignerStreamTokenReceiptErrorV1),
    /// Current custody, exclusive reservation, signing or authoritative completion failed.
    Operation(SignerOperationErrorV1),
    /// Private bounded receipt persistence or retained identity failed.
    Journal,
    /// Finite local inventory resources are busy; reconcile the original operation.
    LocalCapacity,
}
impl fmt::Display for SignerStreamTokenErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Receipt(_) => "stream-token signing request or receipt rejected",
            Self::Operation(_) => "stream-token authoritative operation failed",
            Self::LocalCapacity => "local signer-journal inventory capacity unavailable",
            Self::Journal => "stream-token private receipt journal unavailable",
        })
    }
}
impl std::error::Error for SignerStreamTokenErrorV1 {}
impl From<SignerOperationErrorV1> for SignerStreamTokenErrorV1 {
    fn from(error: SignerOperationErrorV1) -> Self {
        Self::Operation(error)
    }
}
impl From<SignerStreamTokenReceiptErrorV1> for SignerStreamTokenErrorV1 {
    fn from(error: SignerStreamTokenReceiptErrorV1) -> Self {
        Self::Receipt(error)
    }
}
impl From<SignerReceiptJournalErrorV1> for SignerStreamTokenErrorV1 {
    fn from(error: SignerReceiptJournalErrorV1) -> Self {
        if error.is_local_capacity() {
            Self::LocalCapacity
        } else {
            Self::Journal
        }
    }
}

/// Bounded canonical public receipt bytes whose in-memory copy is scrubbed on drop.
///
/// This is neither token authority nor a verified observer result. Its consumer must authenticate
/// the exact prepared body and fresh challenged completion using independently configured trust.
pub struct SignerStreamTokenReceiptBytesV1(Zeroizing<Vec<u8>>);
impl SignerStreamTokenReceiptBytesV1 {
    /// Borrow the exact immutable receipt bytes for transport and independent verification.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl fmt::Debug for SignerStreamTokenReceiptBytesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerStreamTokenReceiptBytesV1")
            .finish_non_exhaustive()
    }
}

/// Provider-free read capability for one purpose-bound private receipt journal.
///
/// This retains only configured public custody, a read-only journal lease and an independently
/// supplied finalized read source. It cannot reserve, complete, sign or renew an operation.
/// Receipt bytes are released only after exact completed-row observations at both recovery phases
/// and a final journal identity check. The source is responsible for proving actual native
/// execution and finality; injected sources do not qualify a deployment.
pub struct SignerStreamTokenCompletedReceiptCheckV1 {
    binding: SignerCustodyBindingV1,
    record: Vec<u8>,
    trust: SignerCustodyTrustV1,
    source: Arc<dyn SignerOperationFinalizedReadSourceV1>,
    journal: SignerReceiptJournalReaderV1,
}
impl fmt::Debug for SignerStreamTokenCompletedReceiptCheckV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerStreamTokenCompletedReceiptCheckV1")
            .finish_non_exhaustive()
    }
}
impl SignerStreamTokenCompletedReceiptCheckV1 {
    /// Pin exact configured role-11 custody and the original journal's read-only lease.
    ///
    /// # Errors
    /// Rejects a wrong journal family, malformed binding or unavailable current custody before
    /// retaining a checker. This does not itself establish a completed operation.
    pub fn new(
        binding: SignerCustodyBindingV1,
        record: Vec<u8>,
        trust: SignerCustodyTrustV1,
        source: Arc<dyn SignerOperationFinalizedReadSourceV1>,
        journal: &SignerReceiptJournalV1,
    ) -> Result<Self, SignerStreamTokenErrorV1> {
        if journal.purpose() != SignerReceiptPurposeV1::StreamToken {
            return Err(SignerStreamTokenErrorV1::Journal);
        }
        stream_token_binding_digest_v1(&binding)?;
        let authority = SignerOperationAuthorityV1 {
            binding: &binding,
            record: &record,
            trust: &trust,
            source: source.as_ref(),
        };
        authority.verify(&source.observe(&binding)?)?;
        Ok(Self {
            binding,
            record,
            trust,
            source,
            journal: journal.reader(),
        })
    }

    /// Verify and release only the original, finalized, completed receipt for this exact body.
    ///
    /// # Errors
    /// Rejects a missing or changed private receipt, invalid signatures or token times, a
    /// non-finalized or substituted completion, and stale or revoked original custody.
    pub fn check(
        &self,
        payload: &[u8],
    ) -> Result<SignerStreamTokenReceiptBytesV1, SignerStreamTokenErrorV1> {
        recover_completed_receipt(
            payload,
            &SignerOperationAuthorityV1 {
                binding: &self.binding,
                record: &self.record,
                trust: &self.trust,
                source: self.source.as_ref(),
            },
            &self.journal,
        )
    }
}

/// Long-lived exact provider/key/policy signer with a single bounded private receipt journal.
///
/// It never retains a caller or constructor audit predecessor. Each new body obtains a fresh
/// authenticated snapshot and an exclusive authoritative CAS; concurrent head races fail closed.
/// Recovery reads the original intent and cannot substitute a newer audit head or sign again.
pub struct SignerStreamTokenServiceV1 {
    coordinator: SignerOperationCoordinatorV1,
    journal: SignerReceiptJournalV1,
}
impl fmt::Debug for SignerStreamTokenServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SignerStreamTokenServiceV1")
            .finish_non_exhaustive()
    }
}
impl SignerStreamTokenServiceV1 {
    /// Assemble the provider-scoped service from an owner-only software credential.
    ///
    /// The caller supplies one independently authenticated finalized operation source for both
    /// the key provider and coordinator. This constructor does not create native Reserve, Check,
    /// completion or observer authority. It rejects an absent source or wrong role/journal before
    /// opening the credential; a current custody read is still required before construction.
    ///
    /// # Errors
    /// Rejects absent or unavailable authoritative state, wrong purpose, journal, or credential,
    /// and any current custody mismatch without producing a signing capability.
    pub fn from_software_supervisor_credential(
        path: &Path,
        binding: SignerCustodyBindingV1,
        record: Vec<u8>,
        trust: SignerCustodyTrustV1,
        source: Option<Arc<dyn SignerOperationStateSourceV1>>,
        journal: SignerReceiptJournalV1,
    ) -> Result<Self, SignerStreamTokenErrorV1> {
        let source = source.ok_or(SignerOperationErrorV1::StateUnavailable)?;
        if journal.purpose() != SignerReceiptPurposeV1::StreamToken {
            return Err(SignerStreamTokenErrorV1::Journal);
        }
        stream_token_binding_digest_v1(&binding)?;
        let coordinator = SignerOperationCoordinatorV1::from_software_supervisor_credential(
            path,
            binding,
            record,
            trust,
            Some(source),
        )?;
        Self::new(coordinator, journal)
    }

    /// Split off a provider-free completed-receipt checker over this exact journal lease.
    ///
    /// The returned capability can outlive the signer and has no Reserve, Complete or key method.
    /// Its source still must authenticate native finality for every completed observation.
    ///
    /// # Errors
    /// Rejects a changed or unavailable current custody before retaining the read capability.
    pub fn completed_receipt_check(
        &self,
    ) -> Result<SignerStreamTokenCompletedReceiptCheckV1, SignerStreamTokenErrorV1> {
        let read_source: Arc<dyn SignerOperationFinalizedReadSourceV1> =
            Arc::new(Arc::clone(&self.coordinator.source));
        SignerStreamTokenCompletedReceiptCheckV1::new(
            self.coordinator.binding.clone(),
            self.coordinator.record.clone(),
            self.coordinator.trust.clone(),
            read_source,
            &self.journal,
        )
    }

    /// Bind one exact configured provider to an already independently qualified coordinator.
    ///
    /// # Errors
    /// Rejects a wrong journal purpose, unsupported role/algorithm/provider/key revision or
    /// unavailable current custody before any provider operation or reservation.
    pub fn new(
        coordinator: SignerOperationCoordinatorV1,
        journal: SignerReceiptJournalV1,
    ) -> Result<Self, SignerStreamTokenErrorV1> {
        if journal.purpose() != SignerReceiptPurposeV1::StreamToken {
            return Err(SignerStreamTokenErrorV1::Journal);
        }
        stream_token_binding_digest_v1(&coordinator.binding)?;
        coordinator.verify(&coordinator.source.observe(&coordinator.binding)?)?;
        Ok(Self {
            coordinator,
            journal,
        })
    }

    /// Sign one exact locally prepared canonical body, without silently retrying a failed attempt.
    ///
    /// # Errors
    /// Malformed or unauthorized bodies fail before reading signing state or reserving. Every
    /// later failure retains its replay tombstone and keeps all signature copies internal.
    pub fn sign(
        &self,
        payload: &[u8],
    ) -> Result<SignerStreamTokenReceiptBytesV1, SignerStreamTokenErrorV1> {
        let (body, expected) =
            prepare_stream_token_signing_payload_v1(payload, &self.coordinator.binding)?;
        // A genuine native source must permanently tombstone admitted IDs. Also fence a staged
        // private receipt before another source read, Reserve or key use if that source rolled back.
        self.journal.ensure_unstaged(expected.operation_id())?;
        let snapshot = self
            .coordinator
            .source
            .observe_signing_state(&self.coordinator.binding)?;
        let custody = self.coordinator.verify(&snapshot.custody)?;
        validate_token_time(&expected, &custody)?;
        let request = SignerStreamTokenRequestV1::new(&custody, &expected, &body)?;
        let intent = SignerOperationIntentV1 {
            action: SignerOperationActionV1::Sign,
            operation_id: expected.operation_id(),
            request_digest: request.digest()?,
            previous_audit: snapshot.audit_head,
        };
        // begin observes custody again; reservation CAS must compare this exact prepared head.
        let reviewed = StreamTokenReviewedV1 { request, intent };
        let mut operation = self
            .coordinator
            .begin_stream_token(intent, &body, payload, &expected, &reviewed)?;
        if SignerOperationCustodyV1::from_verified(&operation.custody) != request.original_custody
            || !operation.custody.continues_active_state(&custody)
        {
            return Err(SignerOperationErrorV1::CustodyChanged.into());
        }
        validate_token_time(&expected, &operation.custody)?;
        let mut signatures = WorkingSignatures(Vec::with_capacity(4));
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::RolePayload,
            payload,
        )?;
        let audit = signer_stream_token_audit_v1(
            &request,
            &intent,
            operation.reservation,
            &signatures.0[0].signature,
        )?;
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::AuditRecord,
            &audit.signing_message(),
        )?;
        let provenance = SignerOperationProvenanceV1 {
            original_custody: request.original_custody,
            signing_anchor: operation.custody.current_anchor(),
            intent_digest: operation.intent_digest,
            reservation: operation.reservation,
            audit,
        };
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::Provenance,
            &provenance
                .signing_message()
                .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?,
        )?;
        let commitment = SignerOperationCommitmentV1 {
            audit,
            response_digest: signer_stream_token_response_digest_v1(
                &request,
                &provenance,
                &signatures.0,
            )?,
        };
        signatures.push(
            &mut operation,
            SignerKeyOperationPurposeV1::Response,
            &commitment.response_signing_message(),
        )?;
        let candidate = PendingReceipt(SignerStreamTokenReceiptV1 {
            magic: SIGNER_STREAM_TOKEN_RECEIPT_MAGIC_V1,
            version: 1,
            custody_record: self.coordinator.record.clone(),
            request,
            intent,
            reservation: operation.reservation,
            provenance,
            commitment,
            signatures: std::mem::take(&mut signatures.0),
        });
        let token = PendingToken(token_for(&body, &candidate.0)?);
        let signatures_digest = validate_stream_token_signatures_v1(
            &candidate.0,
            &token.0,
            &expected,
            &operation.custody,
        )?;
        let encoded = Zeroizing::new(candidate.0.encode_canonical()?);
        drop(token);
        drop(candidate);
        let staged = self.journal.stage(expected.operation_id(), &encoded)?;
        let completed = operation.finish(commitment)?;
        if completed.signatures_digest() != signatures_digest {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidSignature.into());
        }
        staged.recheck()?;
        let final_custody =
            revalidate_completed(&self.coordinator.authority(), &intent, &completed)?;
        validate_token_time(&expected, &final_custody)?;
        staged.recheck()?;
        Ok(SignerStreamTokenReceiptBytesV1(Zeroizing::new(
            staged.bytes().to_vec(),
        )))
    }

    /// Recover the exact original completed operation without reserve, commit or provider use.
    ///
    /// # Errors
    /// Rejects malformed bodies, missing/changed private receipts, incomplete or substituted
    /// completion, same-key custody renewal, revocation, or an expired/future-dated token.
    pub fn recover(
        &self,
        payload: &[u8],
    ) -> Result<SignerStreamTokenReceiptBytesV1, SignerStreamTokenErrorV1> {
        recover_completed_receipt(
            payload,
            &self.coordinator.authority(),
            &self.journal.reader(),
        )
    }
}

fn recover_completed_receipt(
    payload: &[u8],
    authority: &SignerOperationAuthorityV1<'_>,
    journal: &SignerReceiptJournalReaderV1,
) -> Result<SignerStreamTokenReceiptBytesV1, SignerStreamTokenErrorV1> {
    let (body, expected) = prepare_stream_token_signing_payload_v1(payload, authority.binding)?;
    let custody = authority.verify(&authority.source.observe(authority.binding)?)?;
    validate_token_time(&expected, &custody)?;
    let request = SignerStreamTokenRequestV1::new(&custody, &expected, &body)?;
    let staged = journal.recover(expected.operation_id())?;
    let candidate = PendingReceipt(SignerStreamTokenReceiptV1::decode_canonical(
        staged.bytes(),
    )?);
    let receipt = &candidate.0;
    if receipt.custody_record.as_slice() != authority.record || receipt.request != request {
        return Err(SignerStreamTokenReceiptErrorV1::TokenMismatch.into());
    }
    let token = PendingToken(token_for(&body, receipt)?);
    let signatures_digest =
        validate_stream_token_signatures_v1(receipt, &token.0, &expected, &custody)?;
    drop(token);
    let audit_message = receipt.commitment.audit.signing_message();
    let provenance_message = receipt
        .provenance
        .signing_message()
        .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidReceipt)?;
    let response_message = receipt.commitment.response_signing_message();
    let mut recovered = Vec::with_capacity(4);
    for (signature, (purpose, message)) in receipt.signatures.iter().zip([
        (SignerKeyOperationPurposeV1::RolePayload, payload),
        (
            SignerKeyOperationPurposeV1::AuditRecord,
            audit_message.as_slice(),
        ),
        (
            SignerKeyOperationPurposeV1::Provenance,
            provenance_message.as_slice(),
        ),
        (
            SignerKeyOperationPurposeV1::Response,
            response_message.as_slice(),
        ),
    ]) {
        // The shared validator already established exactly four ordered signatures.
        if signature.purpose != purpose
            || signature.message_digest != signer_operation_message_digest_v1(message)
        {
            return Err(SignerStreamTokenReceiptErrorV1::InvalidSignature.into());
        }
        recovered.push(RecoveredSignerSignatureV1 {
            purpose,
            message: Zeroizing::new(message.to_vec()),
            signature: Zeroizing::new(signature.signature.clone()),
        });
    }
    let completed = authority.recover_completed(RecoveredSignerOperationV1 {
        original_custody: receipt.request.original_custody,
        intent: receipt.intent,
        reservation: receipt.reservation,
        commitment: receipt.commitment,
        signatures: recovered,
    })?;
    if signatures_digest != completed.signatures_digest()
        || signer_operation_signatures_digest_v1(&receipt.signatures)
            .map_err(|_| SignerStreamTokenReceiptErrorV1::InvalidSignature)?
            != signatures_digest
    {
        return Err(SignerStreamTokenReceiptErrorV1::InvalidSignature.into());
    }
    staged.recheck()?;
    let final_custody = revalidate_completed(authority, &receipt.intent, &completed)?;
    validate_token_time(&expected, &final_custody)?;
    staged.recheck()?;
    Ok(SignerStreamTokenReceiptBytesV1(Zeroizing::new(
        staged.bytes().to_vec(),
    )))
}

fn revalidate_completed(
    authority: &SignerOperationAuthorityV1<'_>,
    intent: &SignerOperationIntentV1,
    completed: &CompletedSignerOperationV1,
) -> Result<VerifiedSignerCustodyV1, SignerStreamTokenErrorV1> {
    let commit = SignerOperationCommitRequestV1 {
        check: SignerOperationReservationCheckV1 {
            request: SignerOperationReservationRequestV1 {
                intent,
                intent_digest: completed.intent_digest,
                custody: &completed.custody,
            },
            reservation: completed.reservation,
        },
        commitment: completed.commitment,
        signatures_digest: completed.signatures_digest,
        original_custody: completed.original_custody,
    };
    let current = authority.verify(
        &authority
            .source
            .observe_committed(&commit, SignerCommittedObservationPhaseV1::BeforeRelease)?,
    )?;
    if !current.continues_active_state(&completed.custody) {
        return Err(SignerOperationErrorV1::CustodyChanged.into());
    }
    Ok(current)
}

fn validate_token_time(
    expected: &SignerStreamTokenExpectedV1,
    custody: &VerifiedSignerCustodyV1,
) -> Result<(), SignerStreamTokenErrorV1> {
    expected
        .validate_time_at(custody.verified_at_unix_ms())
        .map_err(Into::into)
}

fn token_for(
    body: &StreamTokenBodyV1,
    receipt: &SignerStreamTokenReceiptV1,
) -> Result<StreamTokenV1, SignerStreamTokenErrorV1> {
    let signature = Zeroizing::new(receipt.role_signature_claim()?);
    Ok(StreamTokenV1 {
        body: body.clone(),
        signature: signature.to_vec(),
    })
}
struct WorkingSignatures(Vec<SignerOperationSignatureV1>);
impl WorkingSignatures {
    fn push(
        &mut self,
        operation: &mut SignerOperationV1<'_>,
        purpose: SignerKeyOperationPurposeV1,
        message: &[u8],
    ) -> Result<(), SignerOperationErrorV1> {
        let signature = operation.sign(purpose, message)?;
        self.0.push(SignerOperationSignatureV1 {
            purpose,
            message_digest: signer_operation_message_digest_v1(message),
            signature: signature.to_vec(),
        });
        Ok(())
    }
}
impl Drop for WorkingSignatures {
    fn drop(&mut self) {
        for signature in &mut self.0 {
            signature.signature.zeroize();
        }
    }
}
struct PendingReceipt(SignerStreamTokenReceiptV1);
impl Drop for PendingReceipt {
    fn drop(&mut self) {
        for signature in &mut self.0.signatures {
            signature.signature.zeroize();
        }
    }
}
struct PendingToken(StreamTokenV1);
impl Drop for PendingToken {
    fn drop(&mut self) {
        self.0.signature.zeroize();
    }
}

mod transport;
