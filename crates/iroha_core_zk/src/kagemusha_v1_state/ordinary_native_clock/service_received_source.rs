//! Portable genuine Receive proof source, without a receiver private key or Financial borrower.
use super::*;
use crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryServiceReceivedCashOutputV1;
use iroha_data_model::kagemusha::*;

/// Exact installed receipt, actual Wrapper-admitted output, original receiver FI/C/PI and both
/// signature/finality-admitted request clocks. A decoder, callback or Native borrower cannot
/// construct this historical capability. Incoming DATA/one-use key/current FI remain separate.
pub struct KagemushaAuthenticatedOrdinaryServiceReceivedSourceV1<'a> {
    assertion: &'a KagemushaAuthenticatedOrdinaryServiceReceivedLineageCommitAssertionV1<'a>,
    output: KagemushaVerifiedOrdinaryServiceReceivedCashOutputV1,
    receiver: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    request_integrity: Option<&'a KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    financial_enrollment_original: Vec<u8>,
    request_signature_capture_context: KagemushaOrdinaryCashClockContextV1,
    request_clocks: [&'a KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 2],
    sender_admission_clock: &'a KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
}
impl KagemushaAuthenticatedOrdinaryServiceReceivedLineageCommitAssertionV1<'_> {
    /// Retain the actual distinct compact Wrapper admission and independently admitted original
    /// receiver context under this same installed assertion. This supplies no Native secret,
    /// private counter floor, measured current elapsed time or funding authority.
    /// # Errors
    /// Refuses another assertion/output/receiver, old PI or full request clock original.
    pub fn authenticate_received_source<'a>(
        &'a self,
        output: KagemushaVerifiedOrdinaryServiceReceivedCashOutputV1,
        receiver: &'a KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
        request_integrity: Option<&'a KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
        request_signature_capture_context: &KagemushaOrdinaryCashClockContextV1,
        request_clocks: [&'a KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1; 2],
        sender_admission_clock: &'a KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryServiceReceivedSourceV1<'a>> {
        self.recheck_retained_custody()?;
        let source = KagemushaAuthenticatedOrdinaryServiceReceivedSourceV1 {
            assertion: self,
            output,
            receiver,
            request_integrity,
            financial_enrollment_original: receiver
                .certificate()
                .canonical_bytes()
                .map_err(|_| Rejected)?,
            request_signature_capture_context: request_signature_capture_context.clone(),
            request_clocks,
            sender_admission_clock,
        };
        source.recheck_retained_custody()?;
        Ok(source)
    }
}
impl KagemushaAuthenticatedOrdinaryServiceReceivedSourceV1<'_> {
    pub(super) fn retained_clock_for_service_assertion(
        &self,
    ) -> Result<&KagemushaOrdinaryNativeClockOwnerV1> {
        self.recheck_retained_custody()?;
        self.assertion.retained_clock_for_received_source()
    }
    pub(super) fn retained_release_for_service_assertion(
        &self,
    ) -> Result<&KagemushaAuthenticatedReleaseV1> {
        self.recheck_retained_custody()?;
        self.assertion.retained_release_for_received_source()
    }

    /// Same closed stateless Wrapper result; no Native request-key loan is returned.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn received_output(&self) -> Result<&KagemushaVerifiedOrdinaryServiceReceivedCashOutputV1> {
        self.recheck_retained_custody()?;
        Ok(&self.output)
    }
    /// Complete actual Core assertion transport original.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn received_assertion_transport_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        self.assertion.transport_original()
    }
    /// Complete original receiver FI certificate; never the sender's certificate or a renewal.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn financial_enrollment_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.financial_enrollment_original)
    }
    /// Same issuer/authenticator-admitted receiver C originally used for this request.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn receiver_credential_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(self.receiver.app_credential().original())
    }
    /// Exact old request PI, separate from fresh incoming W2/W1 integrity.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn receiver_integrity_original(&self) -> Result<Option<&[u8]>> {
        self.recheck_retained_custody()?;
        Ok(self.request_integrity.map(|lease| lease.original()))
    }
    /// Signed C enrollment minimum only, never the Native journal's private previous floor.
    /// # Errors
    /// Refuses unsupported profile or changed retained original custody.
    pub fn request_enrollment_counter_minimum(&self) -> Result<Option<u32>> {
        self.recheck_retained_custody()?;
        enrollment_minimum(self.receiver.app_credential())
    }
    /// Immutable request signature-capture context; this lends no current elapsed time.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn request_signature_capture_context(
        &self,
    ) -> Result<&KagemushaOrdinaryCashClockContextV1> {
        self.recheck_retained_custody()?;
        Ok(&self.request_signature_capture_context)
    }
    /// Actual full request preparation/capture signed originals, in that exact order.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn request_signed_clock_originals(&self) -> Result<[&[u8]; 2]> {
        self.recheck_retained_custody()?;
        Ok([
            self.request_clocks[0].original(),
            self.request_clocks[1].original(),
        ])
    }
    /// Actual installed receiver FI policy. Offered sender keys cannot replace this original.
    /// # Errors
    /// Refuses changed retained original custody.
    pub fn issuer_policy(&self) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        self.recheck_retained_custody()?;
        self.assertion.issuer_policy()
    }
    /// Authentic immutable receiver/source custody only. Current FI/PI/clock, actual globally
    /// exclusive DATA successor/credit consumption and Native key opening are separate duties.
    /// # Errors
    /// Refuses changed full originals, issuer/platform signatures, clock joins or historical PI.
    pub fn recheck_retained_custody(&self) -> Result<()> {
        self.assertion.recheck_retained_custody()?;
        let issuer = self.assertion.issuer_policy()?;
        let fi = self.receiver.certificate();
        let credential = self.receiver.app_credential();
        let request = self.output.request();
        let body = &request.body;
        let capture = &self.request_signature_capture_context;
        require_request_window(body, capture)?;
        fi.signature
            .verify(
                &issuer.issuer_public_key,
                &fi.subject.approval_payload().map_err(|_| Rejected)?,
            )
            .map_err(|_| Rejected)?;
        if self.output.commit() != self.assertion.commit()?
            || self.output.received_assertion_original_sha256()
                != <[u8; 32]>::from(Sha256::digest(self.assertion.transport_original()?))
            || self.output.source_semantic_digest()
                != self
                    .output
                    .output()
                    .binding_digest()
                    .map_err(|_| Rejected)?
            || self.output.request_original() != request.canonical_bytes().map_err(|_| Rejected)?
            || self.financial_enrollment_original != fi.canonical_bytes().map_err(|_| Rejected)?
            || fi.subject.owner.runtime != issuer.runtime
            || fi.subject.issuer_policy_id != issuer.issuer_policy_id
            || fi.subject.issuer_audience != issuer.issuer_audience
            || fi.subject.issued_at_ms < issuer.valid_from_ms
            || fi.subject.expires_at_ms > issuer.expires_at_ms
            || fi.subject.ordinary_app_credential_digest != credential.digest()
            || fi
                .subject
                .issuance
                .credential
                .canonical_bytes()
                .map_err(|_| Rejected)?
                != credential.original()
            || body.recipient_credential_digest != credential.digest()
        {
            return Err(Rejected);
        }
        request
            .authenticate_receiver_signature(credential, enrollment_minimum(credential)?)
            .map_err(|_| Rejected)?;
        let installed_clock = self.assertion.retained_clock_for_received_source()?;
        let outgoing = crate::kagemusha_v1_recursion::KagemushaOrdinaryCashOutgoingOriginalV1::decode_original(self.output.outgoing_original()).map_err(|_| Rejected)?;
        installed_clock
            .authenticate_received_historical_signed_original(
                self.sender_admission_clock.original(),
            )?
            .recheck_cash_context(outgoing.admission_clock_context())?;
        for (clock, context) in [
            (self.request_clocks[0], &body.clock_context),
            (self.request_clocks[1], capture),
        ] {
            installed_clock
                .authenticate_received_historical_signed_original(clock.original())?
                .recheck_cash_context(context)?;
            clock.recheck_cash_context(context)?;
            for at in [context.lower_at_ms, context.upper_at_ms] {
                match self.request_integrity {
                    Some(lease) => self.receiver.recheck_with_integrity_lease(lease, at),
                    None => self.receiver.recheck_at_trusted_time(at),
                }
                .map_err(|_| Rejected)?;
            }
        }
        self.assertion.recheck_retained_custody()
    }
}
fn enrollment_minimum(c: &KagemushaVerifiedOrdinaryAppCredentialV1) -> Result<Option<u32>> {
    match c.subject().platform_class {
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => Ok(None),
        KagemushaHardwarePlatformClassV1::AppleAppAttest => {
            Ok(Some(c.subject().app_attest_counter_floor))
        }
        _ => Err(Rejected),
    }
}
fn require_request_window(
    body: &KagemushaOrdinaryPaymentRequestBodyV1,
    capture: &KagemushaOrdinaryCashClockContextV1,
) -> Result<()> {
    body.validate_shape().map_err(|_| Rejected)?;
    capture.validate_shape().map_err(|_| Rejected)?;
    if body.issued_at_ms != body.clock_context.lower_at_ms
        || capture.lower_at_ms < body.clock_context.lower_at_ms
        || capture.upper_at_ms < body.clock_context.upper_at_ms
        || body.clock_context.upper_at_ms >= body.expires_at_ms
        || capture.upper_at_ms >= body.expires_at_ms
    {
        return Err(Rejected);
    }
    Ok(())
}
