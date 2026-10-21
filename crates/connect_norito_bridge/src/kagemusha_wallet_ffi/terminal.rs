//! Separate explicit destructive review; ordinary retirement/collection never enter this path.

use super::*;

const REVIEW_BYTES: usize = 254;
const MAGIC: &[u8; 8] = b"KWCDV1\0\0";

pub(super) fn is_terminal_input(input: &setup::Setup) -> bool {
    matches!(
        input,
        setup::Setup::ReviewCustodyDeletion
            | setup::Setup::ConfirmCustodyDeletion { .. }
            | setup::Setup::ResumeCustodyDeletion
            | setup::Setup::DiscardCustodyDeletion { .. }
    )
}

pub(super) fn dispatch(
    invocation: &exchange::Invocation,
    input: setup::Setup,
    delivery: &mut review::CapabilityDelivery,
) -> Result<Response> {
    invocation.validate()?;
    let owner = invocation.owner();
    // Cancel/join proof work before locking custody. This remains callable after an uncertain
    // destructive attempt solely so the same actual provider can reconcile its outcome.
    let _priority = owner
        .background
        .payment(&owner.scheduler, state::FoldCallClassV1::Payment);
    let mut guard = owner.wallet.lock().map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    // Deleting custody discards the open call's staged records with the slot.
    let wallet = guard.as_deref_mut().ok_or(Failure::code(CLOSED))?;
    archive_call(wallet, |wallet| {
        let response = wallet.setup(invocation.origin(), input)?;
        delivery.issued(invocation, &response);
        Ok(response)
    })
}

fn projection(value: &state::CustodyDeletionReviewV1) -> Result<Vec<u8>> {
    if [
        value.slot,
        value.marker_file_digest,
        value.scheme_id,
        value.asset_digest,
        value.wallet_id,
        value.head,
    ]
    .contains(&[0; 32])
        || value.core_burned_total > value.gross_balance
    {
        return Err(Failure::code(INTERNAL));
    }
    let mut bytes = Vec::with_capacity(REVIEW_BYTES);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&[
        if value.pending { 1 } else { 2 },
        value.lifecycle.tag(),
        value.operation_kind.tag(),
        u8::from(value.pending_outgoing),
        u8::from(value.fee_claims),
        u8::from(value.load_redeem),
    ]);
    for digest in [
        value.slot,
        value.marker_file_digest,
        value.scheme_id,
        value.asset_digest,
        value.wallet_id,
        value.head,
    ] {
        bytes.extend_from_slice(&digest);
    }
    for number in [value.sequence, value.gross_balance, value.core_burned_total] {
        bytes.extend_from_slice(&number.to_le_bytes());
    }
    Ok(bytes)
}
fn progress(value: state::CustodyDeletionProgressV1) -> Response {
    match value {
        state::CustodyDeletionProgressV1::Deleted { marker_file_digest } => Response {
            kind: 54,
            bytes: marker_file_digest.to_vec(),
            ..Response::default()
        },
        state::CustodyDeletionProgressV1::NotDeleted => Response {
            kind: 56,
            ..Response::default()
        },
    }
}
impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send> NativeWallet<P, S> {
    pub(super) fn review_deletion(&mut self, exchange_id: u64) -> Result<Response> {
        self.deletion_reviews.capacity()?;
        let review = self.wallet.review_custody_deletion()?;
        let bytes = projection(review.projection())?;
        let token = self
            .deletion_reviews
            .put(review::OriginBound::new(exchange_id, review))?;
        Ok(Response {
            kind: 53,
            sequence: token.into(),
            bytes,
            ..Response::default()
        })
    }
    pub(super) fn confirm_deletion(&mut self, exchange_id: u64, token: u64) -> Result<Response> {
        // Wrong owner/purpose tokens are rejected without consuming that owner's other review.
        // The genuine token is consumed even if the subsequent current-state check refuses it.
        let review = self.deletion_reviews.take_bound(token, exchange_id)?;
        self.times.clear();
        self.reviews.clear();
        self.deletion_reviews.clear();
        Ok(progress(self.wallet.confirm_custody_deletion(review)?))
    }
    pub(super) fn resume_deletion(&mut self) -> Result<Response> {
        Ok(progress(self.wallet.resume_custody_deletion()?))
    }
    pub(super) fn discard_deletion(&mut self, exchange_id: u64, token: u64) -> Result<Response> {
        self.deletion_reviews.take_bound(token, exchange_id)?;
        Ok(Response {
            kind: 55,
            ..Response::default()
        })
    }
}

#[cfg(test)]
mod tests;
