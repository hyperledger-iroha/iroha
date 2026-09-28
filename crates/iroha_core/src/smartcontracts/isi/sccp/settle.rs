//! Settlement retries: `SettleSccpV1` (`specs/sccp.md` §4.12.1, §4.12.3, §4.16). Owner: ws41.
//!
//! Retries a `Pending` inbound settlement or outbound refund without a proof; it fails when it
//! changes nothing.

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_data_model::{account::AccountId, isi::sccp::SettleSccpV1};

/// Execute `SettleSccpV1`.
///
/// # Errors
///
/// Fails closed until ws41 implements the instruction.
pub fn execute_settle(
    _instruction: SettleSccpV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws41): release, bounce or keep `Pending` (§4.12.3, §4.12.5, §4.16).
    Err(not_wired("SettleSccpV1 execution", "ws41"))
}
