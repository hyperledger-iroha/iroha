//! `SubmitSccpAttestationFaultV1`: equivocation evidence (`specs/sccp.md` §4.11). Owner: ws31.
//!
//! Any valid bridge signature over a non-canonical statement records a fault, evicts and bars
//! the key's peer and forces a new roster generation at the executing block.

use super::{
    Error,
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1},
    not_wired,
};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{account::AccountId, isi::sccp::SubmitSccpAttestationFaultV1};

/// Execute `SubmitSccpAttestationFaultV1`.
///
/// # Errors
///
/// Fails closed until ws31 implements the instruction.
pub fn execute_submit_fault(
    _instruction: SubmitSccpAttestationFaultV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws31): verify, record, bar the peer and force a rotation (§4.11).
    Err(not_wired("SubmitSccpAttestationFaultV1 execution", "ws31"))
}

/// Pre-verify fault evidence against committed state and return its admission keys
/// (deduplicated by `(address, height)`).
///
/// # Errors
///
/// Rejects until ws31 implements pre-verification.
pub fn preverify(
    world: &(impl WorldReadOnly + ?Sized),
    next_block_height: u64,
    instruction: &SubmitSccpAttestationFaultV1,
    authority: &AccountId,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let _ = (world, next_block_height, instruction, authority);
    // TODO(ws31): §4.11 validation against committed state.
    Err(SccpAdmissionRejectV1::not_wired(
        "SubmitSccpAttestationFaultV1 pre-verification",
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
        let reject =
            preverify(&view, 5, &SampleInstructions::fault(), &authority(1)).expect_err("skeleton");
        assert!(reject.reason.contains("TODO(ws31)"), "{reject}");
    }
}
