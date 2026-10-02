//! Historical StateAdvance acknowledgments borrow authentic captured FI and Native clock custody.
//! A decoded record supplies no current loan, and later CAS floors do not rewrite an older cut.
use super::*;

impl KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1<'_> {
    /// Check the exact captured post-StateAdvance FI decision and retained signed clock originals.
    /// This is historical acknowledgment verification only; current exposure separately requires
    /// `recheck_for_effect` with a freshly admitted current FI loan.
    pub(crate) fn recheck_captured_effect(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        captured: &KagemushaCapturedOrdinaryFinancialControlDecisionV1<'_>,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<()> {
        self.recheck_historical(financial)?;
        self.owner.recheck_captured_effect_for_request(
            self.request_sha256,
            financial,
            captured,
            clock,
        )?;
        self.recheck_historical(financial)
    }
}
impl KagemushaAuthenticatedOrdinaryIncomingCommitReceiptV1<'_> {
    /// Recheck this actual incoming Commit against the same acknowledged Native FI decision and
    /// complete retained signed clock. Historical replay does not renew a live DATA/FI grant.
    pub(crate) fn recheck_captured_effect(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        captured: &KagemushaCapturedOrdinaryFinancialControlDecisionV1<'_>,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<()> {
        self.recheck_historical(financial)?;
        self.commit()?;
        self.owner.recheck_captured_effect_for_request(
            self.request_sha256,
            financial,
            captured,
            clock,
        )?;
        self.recheck_historical(financial)
    }
}
impl KagemushaOrdinaryLineageCasOwnerV1 {
    // Shared custody kernel; only actual outgoing/incoming acknowledged receipts may call it.
    // Authentic older acknowledged floors remain valid historical evidence after later payments.
    fn recheck_captured_effect_for_request(
        &self,
        request_sha256: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        captured: &KagemushaCapturedOrdinaryFinancialControlDecisionV1<'_>,
        clock: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<()> {
        self.recheck_historical(financial)?;
        captured.recheck_financial_owner(financial)?;
        let control: KagemushaSignedOrdinaryCurrentControlV1 = decode(
            captured.original()?,
            KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1,
        )?;
        let actual = self.acknowledged(request_sha256, financial)?;
        let result = &actual.result.subject;
        clock.validate_shape().map_err(|_| Rejected)?;
        clock
            .validate_within_original_window(
                control.subject.issued_at_ms,
                control.subject.expires_at_ms,
            )
            .map_err(|_| Rejected)?;
        let signed_clock = financial.verified_retained_cash_clock_originals(clock)?;
        if control.subject.request.owner != result.request.operation.lineage().owner
            || control.subject.request.enrollment_original_sha256
                != self.initialize.enrollment_original_sha256
            || control.subject.request.credential_original_sha256
                != self.initialize.credential_original_sha256
            || control.subject.data_incarnation_digest != result.data_incarnation_digest
            || control.subject.data_revision < result.data_revision
            || control.subject.data_policy_epoch != result.data_policy_epoch
            || control.subject.data_schema_epoch != result.data_schema_epoch
            || control.subject.release_id != result.release_id
            || control.subject.authority_height < result.authority_height
            || signed_clock.certified_height().map_err(|_| Custody)?
                < control.subject.authority_height
            || clock.lower_at_ms < captured.captured_lower_ms()
            || clock.upper_at_ms < captured.captured_upper_ms()
            || clock.lower_at_ms < actual.acknowledgement.captured_clock.lower_at_ms
            || clock.upper_at_ms < actual.acknowledgement.captured_clock.upper_at_ms
            || captured.captured_lower_ms() < result.issued_at_ms
        {
            return Err(Rejected);
        }
        // The captured decision was admitted by this genuine owner under its original floor.
        // Comparing it to the final recovery floor would reject authentic earlier transitions.
        // No historical loan or record is promoted to the current floor/live authority here.
        captured.recheck_financial_owner(financial)?;
        self.recheck_historical(financial)
    }
}
