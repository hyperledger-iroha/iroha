//! Dedicated protected incoming DATA receipts; decoded selectors grant no capability.
//! The pre-debit IncomingSelection owner precedes this finalized source admission.
use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaVerifiedOrdinaryIncomingCommitProofV1,
    KagemushaVerifiedOrdinaryIncomingReservationProofV1,
};
impl KagemushaOrdinaryLineageCasOwnerV1 {
    /// Consume only a genuine finalized-source/State admission under this same installed release.
    /// Actual Main pre-debit intent and Core exclusive head hold are separate earlier owners.
    pub(crate) fn reserve_incoming_transition(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    ) -> Result<Vec<u8>> {
        if proof.release_id() != self.selected.governed.release().release_id() {
            return Err(Rejected);
        }
        let operation = KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(
            proof.reservation().clone(),
        ));
        if let Some((original, _)) =
            self.acknowledged_incoming_operation(financial, current, &operation)?
        {
            return Ok(original);
        }
        self.reserve(financial, current, operation)
    }
    /// Require genuine incoming successor and exact previously acknowledged ReserveIncoming
    /// from this private WAL. Fresh FI remains mandatory before account signing/capture/effects.
    pub(crate) fn reserve_incoming_commit(
        &mut self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryIncomingCommitProofV1,
        reserve_request_sha256_from_native_intent: [u8; 32],
    ) -> Result<Vec<u8>> {
        if proof.release_id() != self.selected.governed.release().release_id() {
            return Err(Rejected);
        }
        let previous = self.acknowledged(reserve_request_sha256_from_native_intent, financial)?;
        match &previous.request.operation {
            KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(reservation)
                if reservation.as_ref() == &proof.commit().reservation => {}
            _ => return Err(Rejected),
        }
        let operation = KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(Box::new(
            proof.commit().clone(),
        ));
        if let Some((original, _)) =
            self.acknowledged_incoming_operation(financial, current, &operation)?
        {
            return Ok(original);
        }
        self.reserve(financial, current, operation)
    }
    /// Lend only exact acknowledged Native incoming reservation; offered equivalent bytes
    /// cannot manufacture a receipt. The expected projection comes from actual Main Intent.
    pub(crate) fn incoming_reservation_receipt(
        &self,
        key: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        expected: &KagemushaOrdinaryIncomingReservationV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1<'_>> {
        if self.acknowledged(key, financial)?.request.operation
            != KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(
                expected.clone(),
            ))
        {
            return Err(Rejected);
        }
        Ok(KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1 {
            owner: self,
            request_sha256: key,
        })
    }
    /// Lend only exact globally committed incoming original after genuine original fsync,
    /// post-fsync clock capture and acknowledgement. Raw or partial replies cannot lend it.
    pub(crate) fn incoming_commit_receipt(
        &self,
        key: [u8; 32],
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        expected: &KagemushaOrdinaryIncomingCommitV1,
    ) -> Result<KagemushaAuthenticatedOrdinaryIncomingCommitReceiptV1<'_>> {
        if self.acknowledged(key, financial)?.request.operation
            != KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(Box::new(
                expected.clone(),
            ))
        {
            return Err(Rejected);
        }
        Ok(KagemushaAuthenticatedOrdinaryIncomingCommitReceiptV1 {
            owner: self,
            request_sha256: key,
        })
    }
}

impl KagemushaOrdinaryLineageCasOwnerV1 {
    // Only actual same-WAL acknowledged originals are reusable. An offered operation is data;
    // callers construct it solely from their actual verified incoming proof/captured W1.
    fn acknowledged_incoming_operation(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        operation: &KagemushaOrdinaryLineageRequestOperationV1,
    ) -> Result<Option<(Vec<u8>, [u8; 64])>> {
        self.recheck_live(financial, current)?;
        if !matches!(
            operation,
            KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(_)
                | KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(_)
        ) {
            return Err(Rejected);
        }
        let key =
            sole_acknowledged_key(self.acknowledged.iter().filter_map(|(key, entry)| {
                (&entry.request.operation == operation).then_some(*key)
            }))?;
        let found = key
            .map(|key| -> Result<_> {
                let actual = self.acknowledged(key, financial)?;
                Ok((
                    actual.request.canonical_bytes().map_err(|_| Rejected)?,
                    actual.account_signature,
                ))
            })
            .transpose()?;
        if let Some((request, _)) = &found {
            if let Some(pending) = &self.pending {
                if pending.request.canonical_bytes().map_err(|_| Rejected)? != *request {
                    return Err(Rejected);
                }
            }
        }
        self.recheck_live(financial, current)?;
        Ok(found)
    }
    pub(crate) fn acknowledged_incoming_reservation_request(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryIncomingReservationProofV1,
    ) -> Result<Option<(Vec<u8>, [u8; 64])>> {
        if proof.release_id() != self.selected.governed.release().release_id() {
            return Err(Rejected);
        }
        self.acknowledged_incoming_operation(
            financial,
            current,
            &KagemushaOrdinaryLineageRequestOperationV1::ReserveIncoming(Box::new(
                proof.reservation().clone(),
            )),
        )
    }
    pub(crate) fn acknowledged_incoming_commit_request(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryIncomingCommitProofV1,
    ) -> Result<Option<(Vec<u8>, [u8; 64])>> {
        if proof.release_id() != self.selected.governed.release().release_id() {
            return Err(Rejected);
        }
        self.acknowledged_incoming_operation(
            financial,
            current,
            &KagemushaOrdinaryLineageRequestOperationV1::CommitIncoming(Box::new(
                proof.commit().clone(),
            )),
        )
    }
}

// Pure cardinality selector, never a receipt/owner constructor. Admission remains in the held WAL.
fn sole_acknowledged_key(mut keys: impl Iterator<Item = [u8; 32]>) -> Result<Option<[u8; 32]>> {
    let key = keys.next();
    if keys.next().is_some() {
        return Err(Rejected);
    }
    Ok(key)
}
#[cfg(test)]
mod retry_tests {
    use super::*;
    #[test]
    fn incoming_exact_retry_refuses_ambiguous_acknowledgements() {
        assert_eq!(sole_acknowledged_key([].into_iter()).unwrap(), None);
        assert_eq!(
            sole_acknowledged_key([[1; 32]].into_iter()).unwrap(),
            Some([1; 32])
        );
        assert!(sole_acknowledged_key([[1; 32], [2; 32]].into_iter()).is_err());
        assert!(sole_acknowledged_key([[1; 32], [1; 32]].into_iter()).is_err());
    }
}
