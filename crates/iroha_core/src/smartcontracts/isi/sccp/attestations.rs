//! `SubmitSccpAttestationsV1` and its admission pre-verification (`specs/sccp.md` §4.8).
//! Owner: ws31.
//!
//! Entries are sorted by `(height, signer_index)` and carry only signatures of the submitting
//! bridge key. Cheap checks run before any cryptography; each stored signature recovers its
//! member's address from `statement_digest(height)`.

use super::{
    Error,
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1},
    not_wired,
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{account::AccountId, isi::sccp::SubmitSccpAttestationsV1};

/// Execute `SubmitSccpAttestationsV1`.
///
/// # Errors
///
/// Fails closed until ws31 implements the instruction.
pub fn execute_submit_attestations(
    _instruction: SubmitSccpAttestationsV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws31): checks 1–3, bitmap, threshold and `SccpAttestationsAllRecorded` (§4.8).
    Err(not_wired("SubmitSccpAttestationsV1 execution", "ws31"))
}

/// Pre-verify one attestation batch against committed state at `next_block_height` and return
/// its admission keys (one pending batch per authority; one content key per entry).
///
/// # Errors
///
/// Rejects until ws31 implements pre-verification.
pub fn preverify(
    world: &(impl WorldReadOnly + ?Sized),
    next_block_height: u64,
    instruction: &SubmitSccpAttestationsV1,
    authority: &AccountId,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let _ = (world, next_block_height, instruction, authority);
    // TODO(ws31): §4.8 checks 1–3 against committed state.
    Err(SccpAdmissionRejectV1::not_wired(
        "SubmitSccpAttestationsV1 pre-verification",
        "ws31",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        SampleInstructions, authority, blank_state,
    };

    #[test]
    fn preverification_rejects_until_implemented() {
        let state = blank_state();
        let view = state.world_view();
        let reject = preverify(&view, 5, &SampleInstructions::attestations(), &authority(1))
            .expect_err("skeleton");
        assert!(reject.reason.contains("TODO(ws31)"), "{reject}");
    }
}
