//! Inbound prove step: `SubmitSccpInboundMessageV1` (`specs/sccp.md` §4.12.1). Owner: ws41.
//!
//! A proof is recorded whenever the revision is not `Staged`; the record is then settled
//! immediately if possible (see [`super::settle`]) and otherwise stays `Pending`.

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_data_model::{account::AccountId, isi::sccp::SubmitSccpInboundMessageV1};

/// Execute `SubmitSccpInboundMessageV1`.
///
/// # Errors
///
/// Fails closed until ws41 implements the instruction.
pub fn execute_submit_inbound(
    _instruction: SubmitSccpInboundMessageV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws41): decode, verify, record, then attempt settlement (§4.12.1).
    Err(not_wired("SubmitSccpInboundMessageV1 execution", "ws41"))
}
