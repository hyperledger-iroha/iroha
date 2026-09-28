//! Genesis-only `InitializeSccpV1` (`specs/sccp.md` §4.1, §4.15, §4.18). Owner: ws31.
//!
//! Validates that it executes inside the genesis block under NPoS with `max_validators` in
//! 4..=31, checks every §4.1 parameter rule, stores the parameters and the nonzero reset nonce,
//! creates the four route escrow accounts and an empty registry.

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_data_model::{account::AccountId, isi::sccp::InitializeSccpV1};

/// Execute `InitializeSccpV1`.
///
/// # Errors
///
/// Fails closed until ws31 implements the instruction.
pub fn execute_initialize(
    _instruction: InitializeSccpV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws31): genesis-only initialization (§4.1).
    Err(not_wired("InitializeSccpV1 execution", "ws31"))
}
