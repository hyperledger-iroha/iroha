//! Exact same-WAL outgoing retry originals; no decoded result can construct a receipt.
use super::*;
impl KagemushaOrdinaryLineageCasOwnerV1 {
    fn acknowledged_outgoing_operation(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        operation: &KagemushaOrdinaryLineageRequestOperationV1,
    ) -> Result<Option<(Vec<u8>, [u8; 64])>> {
        self.recheck_live(financial, current)?;
        if !matches!(
            operation,
            KagemushaOrdinaryLineageRequestOperationV1::Reserve(_)
                | KagemushaOrdinaryLineageRequestOperationV1::Commit(_)
        ) {
            return Err(Rejected);
        }
        let mut keys = self
            .acknowledged
            .iter()
            .filter_map(|(key, entry)| (&entry.request.operation == operation).then_some(*key));
        let key = sole_outgoing_key(&mut keys)?;
        let found = key
            .map(|key| -> Result<_> {
                let actual = self.acknowledged(key, financial)?;
                Ok((
                    actual.request.canonical_bytes().map_err(|_| Rejected)?,
                    actual.account_signature,
                ))
            })
            .transpose()?;
        if let (Some((request, _)), Some(pending)) = (&found, &self.pending) {
            if pending.request.canonical_bytes().map_err(|_| Rejected)? != *request {
                return Err(Rejected);
            }
        }
        self.recheck_live(financial, current)?;
        Ok(found)
    }
    pub(crate) fn acknowledged_outgoing_reservation_request(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryLineageReservationProofV1,
    ) -> Result<Option<(Vec<u8>, [u8; 64])>> {
        if proof.state_proof().normalized_statement().release_id
            != self.selected.governed.release().release_id()
        {
            return Err(Rejected);
        }
        self.acknowledged_outgoing_operation(
            financial,
            current,
            &KagemushaOrdinaryLineageRequestOperationV1::Reserve(Box::new(
                proof.reservation().clone(),
            )),
        )
    }
    pub(crate) fn acknowledged_outgoing_commit_request(
        &self,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        current: &KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1<'_>,
        proof: &KagemushaVerifiedOrdinaryLineageCommitProofV1,
    ) -> Result<Option<(Vec<u8>, [u8; 64])>> {
        // The closed Commit has no projected release getter. Its exact lineage must still
        // be this held CAS owner, and only a matching actual same-WAL acknowledgment is read.
        if proof.commit().reservation.selection.lineage != self.initialize.lineage {
            return Err(Rejected);
        }
        self.acknowledged_outgoing_operation(
            financial,
            current,
            &KagemushaOrdinaryLineageRequestOperationV1::Commit(Box::new(proof.commit().clone())),
        )
    }
}
fn sole_outgoing_key(keys: &mut impl Iterator<Item = [u8; 32]>) -> Result<Option<[u8; 32]>> {
    let key = keys.next();
    if keys.next().is_some() {
        return Err(Rejected);
    }
    Ok(key)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outgoing_retry_refuses_every_ambiguous_acknowledgement() {
        assert_eq!(sole_outgoing_key(&mut [].into_iter()).unwrap(), None);
        assert_eq!(
            sole_outgoing_key(&mut [[1; 32]].into_iter()).unwrap(),
            Some([1; 32])
        );
        assert!(sole_outgoing_key(&mut [[1; 32], [2; 32]].into_iter()).is_err());
        assert!(sole_outgoing_key(&mut [[1; 32], [1; 32]].into_iter()).is_err());
    }
}
