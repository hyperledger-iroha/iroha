//! One fenced account invocation borrowing the actual Main-owned lineage CAS journal.
use super::*;

/// A live borrow of the exact pending lineage request after its durable signing fence.
/// No decoder, signature, clock or public request can construct this Native signing original.
/// It authorizes account consent only; it creates no global result or financial State effect.
pub struct KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'a> {
    owner: &'a KagemushaOrdinaryLineageCasOwnerV1,
    financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
    current: KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'a>,
    prefix: KagemushaRecoveryJournalPrefixV1,
    request_sha256: [u8; 32],
}
impl KagemushaOrdinaryLineageCasOwnerV1 {
    pub(crate) fn pending_request_original(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
    ) -> Result<&KagemushaOrdinaryLineageRequestV1> {
        self.recheck_live(financial, current)?;
        let request = &self.pending.as_ref().ok_or(Rejected)?.request;
        self.require_request(request)?;
        Ok(request)
    }
    pub(crate) fn retained_account_signature(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
    ) -> Result<Option<[u8; 64]>> {
        self.pending_request_original(financial, current)?;
        Ok(self.pending.as_ref().ok_or(Rejected)?.signature)
    }
    pub(crate) fn account_signing_original<'a>(
        &'a self,
        financial: &'a KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'a>,
    ) -> Result<KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'a>> {
        let request_sha256 = request_digest(self.pending_request_original(financial, &current)?)?;
        let value = KagemushaAuthenticatedOrdinaryLineageAccountSigningV1 {
            owner: self,
            financial,
            current,
            prefix: self.prefix.ok_or(Custody)?,
            request_sha256,
        };
        value.recheck()?;
        Ok(value)
    }
    /// Find only an actual retained acknowledgment of the immutable genuine zero anchor.
    /// This closes a crash between CAS acknowledgment and Main's separate durable journal row.
    pub(crate) fn acknowledged_anchor_request(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        expected: &KagemushaOrdinaryLineageAnchorV1,
    ) -> Result<Option<[u8; 32]>> {
        self.recheck_historical(financial)?;
        let operation = KagemushaOrdinaryLineageRequestOperationV1::Anchor(Box::new(expected.clone()));
        let mut found = None;
        for (key, value) in &self.acknowledged {
            if value.request.operation == operation {
                self.acknowledged(*key, financial)?;
                if found.replace(*key).is_some() {
                    return Err(Rejected);
                }
            }
        }
        self.recheck_historical(financial)?;
        Ok(found)
    }
}
impl KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_> {
    /// Check the same exclusive journal prefix, exact request, pending one-call fence and FI.
    /// # Errors
    /// Refuses a changed prefix/request, absent fence, prior signature, stale FI or foreign owner.
    pub fn recheck(&self) -> Result<()> {
        self.owner.recheck_live(self.financial, &self.current)?;
        let pending = self.owner.pending.as_ref().ok_or(Rejected)?;
        if self.owner.prefix != Some(self.prefix)
            || !pending.invoked
            || pending.signature.is_some()
            || request_digest(&pending.request)? != self.request_sha256
        {
            return Err(Rejected);
        }
        self.owner.recheck_journal()
    }
    /// Exact pending public request selected by Native, with no request/key replacement.
    /// # Errors
    /// Refuses unavailable or changed actual signing custody.
    pub fn request(&self) -> Result<&KagemushaOrdinaryLineageRequestV1> {
        self.recheck()?;
        Ok(&self.owner.pending.as_ref().ok_or(Rejected)?.request)
    }
    /// Complete installed purpose original; the real Native account signer compares its inventory.
    /// # Errors
    /// Refuses changed signing custody.
    pub fn installed_policy_original(&self) -> Result<&[u8]> {
        self.recheck()?;
        Ok(&self.owner.initialize.policy_original)
    }
    /// Exact sole Model signing message, distinct from current FI or enrollment consent.
    /// # Errors
    /// Refuses custody, canonical encoding or a changed invocation.
    pub fn account_signing_message(&self) -> Result<Vec<u8>> {
        let message = self.request()?.account_signing_message().map_err(|_| Rejected)?;
        self.recheck()?;
        Ok(message)
    }
}
