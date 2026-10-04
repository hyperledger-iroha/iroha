// Retained caller expectations have no wire implementation, cloning, default or public fields.

/// One privately phased observation attempt, retired on success or completed rejection.
///
/// The runtime supplies fresh unpredictable entropy and retires its bounded pending attempt
/// in place through verification. Success, completed rejection and unwind retire the attempt;
/// only the original retryable admission error restores readiness for the same reply. The runtime
/// must never reconstruct a retired challenge, select floors from a candidate, or discard
/// monotonic history on reconnect. These constructors cannot prove entropy
/// quality or maintain a process-wide replay cache. Decoded requests cannot become this type.
/// Current-only attempts must remain attached to their exact local in-flight operation; a
/// same-binding qualification cannot be pooled, cached or used for another operation's fence.
pub struct SignerStreamTokenObservationExpectedV1 {
    request: SignerStreamTokenObservationRequestV1,
    state: AttemptState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttemptState {
    Ready,
    Retired,
}
impl SignerStreamTokenObservationExpectedV1 {
    /// Prepare a current-only query from independently pinned provider scope and runtime floors.
    ///
    /// # Errors
    /// Rejects wrong binding/purpose, a completed phase, zero challenge or invalid time/finality.
    pub fn current(
        binding: &SignerCustodyBindingV1,
        phase: SignerStreamTokenObservationPhaseV1,
        challenge: [u8; 32],
        minimum_anchor: SignerCustodyAnchorV1,
        not_before_unix_ms: u64,
    ) -> Result<Self, SignerStreamTokenEvidenceAdmissionErrorV1> {
        let binding_digest = stream_token_binding_digest_v1(binding)
            .map_err(SignerStreamTokenEvidenceErrorV1::Receipt)?;
        Self::new(
            phase,
            SignerStreamTokenObservationRequestSubjectV1::CurrentCustody { binding_digest },
            challenge,
            minimum_anchor,
            not_before_unix_ms,
        )
    }

    /// Prepare a completed query only after bounded exact receipt and strict token prevalidation.
    ///
    /// The caller generates the challenge after receiving this receipt. This constructor checks
    /// shape and role signature only, and never creates a fake verified custody/completion marker.
    ///
    /// # Errors
    /// Rejects any substituted expected body/binding, invalid canonical receipt/signature shape,
    /// wrong phase, challenge or floors. Full authority is deferred until signed observation use.
    #[expect(
        clippy::too_many_arguments,
        reason = "the independent prepared operation, phase and runtime floors remain explicit"
    )]
    pub fn completed(
        receipt_bytes: &[u8],
        token: &StreamTokenV1,
        prepared: &SignerStreamTokenExpectedV1,
        binding: &SignerCustodyBindingV1,
        phase: SignerStreamTokenObservationPhaseV1,
        challenge: [u8; 32],
        minimum_anchor: SignerCustodyAnchorV1,
        not_before_unix_ms: u64,
    ) -> Result<Self, SignerStreamTokenEvidenceAdmissionErrorV1> {
        let subject = completed_request_subject(receipt_bytes, token, prepared, binding)?;
        Self::new(
            phase,
            subject,
            challenge,
            minimum_anchor,
            not_before_unix_ms,
        )
    }

    fn new(
        phase: SignerStreamTokenObservationPhaseV1,
        subject: SignerStreamTokenObservationRequestSubjectV1,
        challenge: [u8; 32],
        minimum_anchor: SignerCustodyAnchorV1,
        not_before_unix_ms: u64,
    ) -> Result<Self, SignerStreamTokenEvidenceAdmissionErrorV1> {
        let request = SignerStreamTokenObservationRequestV1 {
            magic: REQUEST_MAGIC,
            phase,
            subject,
            challenge,
            minimum_anchor,
            not_before_unix_ms,
        };
        request.validate()?;
        Ok(Self {
            request,
            state: AttemptState::Ready,
        })
    }

    // Retire before entering arbitrary verification work. An unwind, success or
    // completed rejection cannot restore this challenge. Only the actual error
    // returned by the original checker may reopen an unfinished local attempt.
    fn verify<T>(
        &mut self,
        check: impl FnOnce(&Self) -> Result<T, SignerStreamTokenEvidenceAdmissionErrorV1>,
    ) -> Result<T, SignerStreamTokenEvidenceAdmissionErrorV1> {
        if self.state != AttemptState::Ready {
            return Err(SignerStreamTokenEvidenceErrorV1::SourceMismatch.into());
        }
        self.state = AttemptState::Retired;
        let result = check(self);
        if result.as_ref().is_err_and(|error| error.is_retryable()) {
            self.state = AttemptState::Ready;
        }
        result
    }

    #[cfg(test)]
    fn is_retired(&self) -> bool {
        self.state == AttemptState::Retired
    }

    /// Borrow the immutable public query, including after this private attempt retires.
    #[must_use]
    pub const fn request(&self) -> &SignerStreamTokenObservationRequestV1 {
        &self.request
    }

    /// Send the exact canonical request without exposing a raw-to-retained constructor.
    ///
    /// # Errors
    /// Rejects bounded canonical encoding failure.
    pub fn request_bytes(&self) -> Result<Vec<u8>, SignerStreamTokenEvidenceAdmissionErrorV1> {
        self.request.encode_canonical()
    }
}
impl fmt::Debug for SignerStreamTokenObservationExpectedV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("SignerStreamTokenObservationExpectedV1")
            .finish_non_exhaustive()
    }
}

fn completed_request_subject(
    receipt_bytes: &[u8],
    token: &StreamTokenV1,
    prepared: &SignerStreamTokenExpectedV1,
    binding: &SignerCustodyBindingV1,
) -> Result<SignerStreamTokenObservationRequestSubjectV1, SignerStreamTokenEvidenceAdmissionErrorV1>
{
    // TODO: preserve original receipt/custody codec admission throughout this
    // prevalidation owner; its payload-free receipt errors remain a separate gate.
    let (_, signatures_digest) =
        prevalidate_stream_token_receipt_v1(receipt_bytes, token, prepared, binding)
            .map_err(SignerStreamTokenEvidenceErrorV1::Receipt)?;
    Ok(
        SignerStreamTokenObservationRequestSubjectV1::CompletedOperation {
            binding_digest: prepared.binding_digest(),
            operation_id: prepared.operation_id(),
            signing_payload_digest: prepared.signing_payload_digest(),
            signing_payload_size: prepared.signing_payload_size(),
            receipt_digest: digest_parts(RECEIPT_DOMAIN, &[receipt_bytes]),
            signatures_digest,
        },
    )
}
