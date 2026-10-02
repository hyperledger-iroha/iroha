//! Closed actual ordinary finalized Mint source, distinct from data-only top-up decoding.
//! Actual neutral reserve finality and genuine Mint113 authorization are both mandatory.
//! Historical source custody grants no current FI, incoming reservation or funded State.
use super::*;
use crate::kagemusha_v1_recursion::KagemushaVerifiedOrdinaryMintAuthorizationV1;
use sha2::{Digest as _, Sha256};

/// Genuine same-owner finalized debit source. There is no decoder, Clone or offered-clock/root
/// constructor. Main separately retains its one-use opening, actual global incoming pending
/// predecessor and newly fresh FI/control/PI before exposing funded State.
pub struct KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1<'a> {
    financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
    captured: &'a KagemushaCapturedOrdinaryFinancialControlDecisionV1<'a>,
    authorization: KagemushaVerifiedOrdinaryMintAuthorizationV1,
    finalized: KagemushaOrdinaryTopUpFinalizedOriginalV1,
    finalized_original: Vec<u8>,
    credit_statement: KagemushaMintCreditStatementV1,
}
impl KagemushaOrdinaryEnrolledFinancialOwnerV1 {
    /// Require same-owner real Mint proof and actual acknowledged FI capture, using only this
    /// Native clock's anchored contiguous validator prefix. Offered finality cannot select or
    /// extend that prefix. Global incoming DATA reservation and current effects stay separate.
    pub(crate) fn authenticate_finalized_mint_source<'a>(
        &'a self,
        captured: &'a KagemushaCapturedOrdinaryFinancialControlDecisionV1<'a>,
        authorization: KagemushaVerifiedOrdinaryMintAuthorizationV1,
        original: &[u8],
    ) -> Result<KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1<'a>> {
        self.recheck_historical_proof_custody()?;
        captured.recheck_financial_owner(self)?;
        let finalized = KagemushaOrdinaryTopUpFinalizedOriginalV1::decode_canonical_exact(original)
            .map_err(|_| Rejected)?;
        let credit_statement = authorization
            .authorization()
            .finalized_credit_statement(
                finalized
                    .finality
                    .reserve_receipt_witness
                    .receipt
                    .committed_at_ms,
            )
            .map_err(|_| Rejected)?;
        let value = KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1 {
            financial: self,
            captured,
            authorization,
            finalized,
            finalized_original: original.to_vec(),
            credit_statement,
        };
        value.recheck_historical(self)?;
        Ok(value)
    }
}
impl KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1<'_> {
    /// Exact complete finalized request, original signed effect decision and actual finality.
    /// # Errors
    /// Refuses changed retained financial/control/clock/source custody.
    pub fn finalized_original(&self) -> Result<&[u8]> {
        self.recheck_retained_custody()?;
        Ok(&self.finalized_original)
    }
    /// Genuine both-parity dedicated Mint113 admission, never an OEM authorization.
    /// # Errors
    /// Refuses changed retained financial/control/clock/source custody.
    pub fn authorization(&self) -> Result<&KagemushaVerifiedOrdinaryMintAuthorizationV1> {
        self.recheck_retained_custody()?;
        Ok(&self.authorization)
    }
    /// Exact finalized neutral credit/lifecycle under the actually committed debit time.
    /// # Errors
    /// Refuses changed retained financial/control/clock/source custody.
    pub fn credit_statement(&self) -> Result<&KagemushaMintCreditStatementV1> {
        self.recheck_retained_custody()?;
        Ok(&self.credit_statement)
    }
    /// Complete finalized source data after actual closed finality admission.
    /// # Errors
    /// Refuses changed retained financial/control/clock/source custody.
    pub fn finalized(&self) -> Result<&KagemushaOrdinaryTopUpFinalizedOriginalV1> {
        self.recheck_retained_custody()?;
        Ok(&self.finalized)
    }
    /// Recheck this capability's actual retained owner without accepting an offered owner.
    /// This preserves historical proof custody and creates no fresh FI or funds grant.
    /// # Errors
    /// Refuses any exact retained financial/control/clock/source custody change.
    pub fn recheck_retained_custody(&self) -> Result<()> {
        self.recheck_historical(self.financial)
    }
    /// Canonical semantic selector for genuine incoming source equality, not a funds grant.
    /// # Errors
    /// Refuses changed retained source custody or canonical statement encoding.
    pub fn source_semantic_digest(&self) -> Result<[u8; 32]> {
        self.recheck_historical(self.financial)?;
        self.credit_statement
            .canonical_digest()
            .map_err(|_| Rejected)
    }
    /// Require exact actual financial owner, acknowledged capture, original Mint/C/FI/scope and
    /// retained validator prefix. Historical custody lends no elapsed time, current FI/PI, DATA
    /// successor or credit-consumption capability.
    /// # Errors
    /// Refuses another owner, request, release, C, signed issuer decision, finality membership
    /// or any retained Native financial/control/clock WAL custody change.
    pub fn recheck_historical(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<()> {
        if !std::ptr::eq(self.financial, financial) {
            return Err(Rejected);
        }
        financial.recheck_historical_proof_custody()?;
        self.captured.recheck_financial_owner(financial)?;
        let selected = &financial.reservation.selected;
        let request = KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(
            self.authorization.request_original(),
        )
        .map_err(|_| Rejected)?;
        let c = &request.authorization.statement.context;
        let issuer_decision = KagemushaSignedOrdinaryMintDebitDecisionV1::decode_canonical_exact(
            &self.finalized.issuer_decision_original,
        )
        .map_err(|_| Rejected)?;
        issuer_decision
            .verify_for_request(
                &request,
                &issuer_decision.subject.selection,
                &selected.issuer,
            )
            .map_err(|_| Rejected)?;
        let control_raw = self.captured.original()?;
        let control: KagemushaSignedOrdinaryCurrentControlV1 =
            norito::decode_canonical_with_limits(
                control_raw,
                norito::canonical_decode_limits(control_raw.len()),
            )
            .map_err(|_| Rejected)?;
        if control.canonical_bytes().map_err(|_| Rejected)? != control_raw
            || c.clock_context.lower_at_ms < control.subject.issued_at_ms
            || c.clock_context.upper_at_ms >= control.subject.expires_at_ms
        {
            return Err(Rejected);
        }
        let lineage = KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: selected.owner.clone(),
            financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(
                financial.enrollment.app_credential().subject(),
            )
            .map_err(|_| Rejected)?,
            financial_authority_commitment: financial
                .historical_financial_authority_commitment()?,
        };
        let release = selected.governed.release();
        if self.finalized.request_original != self.authorization.request_original()
            || self.authorization.request_original_sha256()
                != <[u8; 32]>::from(Sha256::digest(self.authorization.request_original()))
            || self.authorization.authorization() != &request.authorization
            || self.authorization.authorization_original_digest()
                != request
                    .authorization
                    .binding_digest()
                    .map_err(|_| Rejected)?
            || self.authorization.credential_original()
                != financial.enrollment.app_credential().original()
            || c.lineage != lineage
            || c.release_id != release.release_id()
            || c.artifact_manifest_digest != release.manifest_digest()
            || c.financial_control_original_sha256 != self.captured.original_sha256()?
            || self.finalized.canonical_bytes().map_err(|_| Rejected)? != self.finalized_original
            || self.credit_statement
                != request
                    .authorization
                    .finalized_credit_statement(
                        self.finalized
                            .finality
                            .reserve_receipt_witness
                            .receipt
                            .committed_at_ms,
                    )
                    .map_err(|_| Rejected)?
        {
            return Err(Rejected);
        }
        // Same privately retained preparation observations: signatures/original context only,
        // never an offered clock interval or renewed current FI decision.
        let clock_original = financial.retained_cash_clock_originals(&c.clock_context)?;
        financial.recheck_retained_cash_clock_originals(&clock_original)?;
        if clock_original.canonical_original() != self.authorization.preparation_clock_original() {
            return Err(Rejected);
        }
        let clock = match &selected.clock {
            SelectedClock::Native(clock) => clock.lock().map_err(|_| Custody)?,
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => return Err(Rejected),
        };
        if Some(clock.installed_selection_digest().map_err(|_| Custody)?)
            != selected.clock_selection_digest
        {
            return Err(Rejected);
        }
        let verifier = clock
            .retained_finality_verifier_for_original_custody()
            .map_err(|_| Custody)?;
        self.finalized
            .finality
            .validate_retained_with_verifier(&selected.owner.runtime.network_id, &verifier)
            .map_err(|_| Rejected)?;
        drop(clock);
        financial.recheck_retained_cash_clock_originals(&clock_original)?;
        self.captured.recheck_financial_owner(financial)?;
        financial.recheck_historical_proof_custody()
    }
}
