//! Outbound voids and refunds: `SubmitSccpOutboundVoidV1` (`specs/sccp.md` §4.16). Owner: ws41.
//!
//! A proven destination void refunds the voided outbound messages; a proven frozen void also
//! freezes the revision through [`super::registry::apply_frozen_void`].

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_data_model::{account::AccountId, isi::sccp::SubmitSccpOutboundVoidV1};

/// Execute `SubmitSccpOutboundVoidV1`.
///
/// # Errors
///
/// Fails closed until ws41 implements the instruction.
pub fn execute_submit_void(
    _instruction: SubmitSccpOutboundVoidV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws41): verify the void event, mark records voided and refund (§4.16).
    Err(not_wired("SubmitSccpOutboundVoidV1 execution", "ws41"))
}
