//! Recipient self-claims (`specs/sccp.md` §4.12.4). Owner: ws41.
//!
//! A recipient that holds no XOR claims alone and fee-exempt; `inbound_self_claim_fee` is
//! deducted from the proceeds at release.

use super::admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1};
use crate::state::WorldReadOnly;
use iroha_data_model::transaction::SignedTransaction;

/// Pre-verify a self-claim transaction against committed state and return its admission keys
/// (one pending self-claim per authority and per message id).
///
/// # Errors
///
/// Rejects until ws41 implements pre-verification.
pub fn preverify(
    world: &(impl WorldReadOnly + ?Sized),
    next_block_height: u64,
    transaction: &SignedTransaction,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let _ = (world, next_block_height, transaction);
    // TODO(ws41): shape, recipient authority, proof or pending record, fee bound (§4.12.4).
    Err(SccpAdmissionRejectV1::not_wired(
        "self-claim pre-verification",
        "ws41",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, sample_signed_transaction};

    #[test]
    fn self_claims_are_rejected_until_implemented() {
        let state = blank_state();
        let reject =
            preverify(&state.world_view(), 2, &sample_signed_transaction()).expect_err("skeleton");
        assert!(reject.reason.contains("TODO(ws41)"), "{reject}");
    }
}
