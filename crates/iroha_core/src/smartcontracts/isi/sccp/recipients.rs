//! Recipient classification and implicit registration (`specs/sccp.md` §4.12.3, §4.19).
//! Owner: ws32.
//!
//! A codec-3 recipient decodes as an `AccountAddress`. It can never be credited when it is an
//! SCCP escrow, when its controller uses an algorithm account admission refuses, or when
//! transfer control deterministically refuses an XOR credit; such messages bounce. An absent
//! creditable recipient is registered with the effect and fee classification of
//! `Register<Account>`.

use super::{Error, not_wired};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::account::AccountId;

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
/// # Errors
///
/// Fails closed until ws32 implements implicit registration.
pub fn ensure_registered(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _account: &AccountId,
) -> Result<bool, Error> {
    // TODO(ws32): `Account::new(account)` with `Register<Account>` effects and fee class.
    Err(not_wired("implicit recipient registration", "ws32"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};

    #[test]
    fn skeleton_classification_bounces_and_registration_fails_closed() {
        let state = blank_state();
        assert_eq!(
            classify_recipient(&state.world_view(), &[1, 2, 3]),
            SccpRecipientClassV1::Undecodable
        );
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let error = ensure_registered(&mut stx, &authority(1)).expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws32)"), "{error}");
    }
}
