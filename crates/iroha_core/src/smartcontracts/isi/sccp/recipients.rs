//! Recipient classification and implicit registration (`specs/sccp.md` §4.12.3, §4.19).
//! Owner: ws32.
//!
//! A codec-3 recipient decodes as an `AccountAddress`. It can never be credited when it is an
//! SCCP escrow, when its controller uses an algorithm account admission refuses, or when
//! transfer control deterministically refuses an XOR credit; such messages bounce. An absent
//! creditable recipient is registered with the effect and fee classification of
//! `Register<Account>`.

use super::Error;
use crate::{
    smartcontracts::Execute,
    state::{StateTransaction, WorldReadOnly},
};
use iroha_data_model::{
    account::{Account, AccountId},
    isi::Register,
};

/// Classification of an inbound recipient (§4.12.3 steps 1–3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SccpRecipientClassV1 {
    /// Decodes to an account that can be credited.
    Creditable {
        /// Recipient account.
        account: AccountId,
        /// Whether the account already exists.
        registered: bool,
    },
    /// Decodes, but the account can never be credited; the message bounces.
    Uncreditable {
        /// Recipient account.
        account: AccountId,
    },
    /// Does not decode as an `AccountAddress`; the message bounces.
    Undecodable,
}

/// Classify the codec-3 recipient `bytes` against `world`.
///
/// The skeleton returns [`SccpRecipientClassV1::Undecodable`] (bounce) until ws32 implements
/// the rules.
#[must_use]
pub fn classify_recipient(
    world: &(impl WorldReadOnly + ?Sized),
    bytes: &[u8],
) -> SccpRecipientClassV1 {
    let _ = (world, bytes);
    // TODO(ws32): decode, escrow/algorithm/transfer-control rules (§4.12.3).
    SccpRecipientClassV1::Undecodable
}

/// Register `account` if it does not exist, returning whether it was registered now.
///
/// The registration executes `Register<Account>(Account::new(account))` as the account itself,
/// so it has exactly the effects, controller checks and events of an ordinary registration
/// (§4.12.3, §4.19); core applies it without an executor permission check because the SCCP
/// rule, not a user, authorizes it.
///
/// # Errors
///
/// Propagates the rejection of the `Register<Account>` execution (for example a controller
/// algorithm that account admission refuses).
pub fn ensure_registered(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
) -> Result<bool, Error> {
    if state_transaction.world.account(account).is_ok() {
        return Ok(false);
    }
    Register::account(Account::new(account.clone())).execute(account, state_transaction)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};

    #[test]
    fn skeleton_classification_bounces_and_registration_is_idempotent() {
        let state = blank_state();
        assert_eq!(
            classify_recipient(&state.world_view(), &[1, 2, 3]),
            SccpRecipientClassV1::Undecodable
        );
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(ensure_registered(&mut stx, &authority(1)), Ok(true));
        assert!(stx.world.account(&authority(1)).is_ok());
        assert_eq!(ensure_registered(&mut stx, &authority(1)), Ok(false));
    }
}
