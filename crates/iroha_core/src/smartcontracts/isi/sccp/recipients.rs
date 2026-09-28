//! Recipient classification and implicit registration (`specs/sccp.md` §4.12.3, §4.19).
//! Owner: ws32.
//!
//! A codec-3 recipient decodes as an `AccountAddress`. It can never be credited when it is an
//! SCCP escrow, when its controller uses an algorithm account admission refuses, or when
//! transfer control deterministically refuses an XOR credit; such messages bounce. An absent
//! creditable recipient is registered with the effect and fee classification of
//! `Register<Account>`.

use super::{Error, escrow};
use crate::{
    smartcontracts::{Execute, isi::domain::isi::ensure_controller_capabilities},
    state::{StateTransaction, WorldReadOnly},
};
use iroha_data_model::{
    account::{Account, AccountAddress, AccountId},
    asset::AssetId,
    bridge::SccpNetworkV1,
    isi::Register,
    sccp::{
        escrow::sccp_taira_xor_asset_definition_id,
        events::{SccpBounceReasonV1, SccpEvent, SccpRecipientRegisteredV1},
    },
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
        /// Why it can never be credited.
        reason: SccpBounceReasonV1,
    },
    /// Does not decode as an `AccountAddress`; the message bounces.
    Undecodable,
}

/// Classify the codec-3 recipient `bytes` of an inbound credit of `amount` Taira units
/// (§4.12.3 steps 1–2).
#[must_use]
pub fn classify_recipient(
    state_transaction: &StateTransaction<'_, '_>,
    bytes: &[u8],
    amount: u128,
) -> SccpRecipientClassV1 {
    AccountAddress::from_canonical_bytes(bytes)
        .and_then(|address| address.to_account_id())
        .map_or(SccpRecipientClassV1::Undecodable, |account| {
            classify_account(state_transaction, account, amount)
        })
}

/// Classify `account` as the recipient of an XOR credit of `amount` Taira units (§4.12.3
/// step 2).
///
/// It is uncreditable when it is an SCCP escrow, when its controller uses a signing algorithm
/// or curve that account admission refuses, or, when it exists, when its incoming transfer
/// control, holding limit or custody refuses the credit.
#[must_use]
pub fn classify_account(
    state_transaction: &StateTransaction<'_, '_>,
    account: AccountId,
    amount: u128,
) -> SccpRecipientClassV1 {
    let world = &*state_transaction.world;
    let reason = if escrow::is_escrow(world, &account) {
        Some(SccpBounceReasonV1::EscrowRecipient)
    } else if ensure_controller_capabilities(
        account.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )
    .is_err()
    {
        Some(SccpBounceReasonV1::InadmissibleController)
    } else {
        None
    };
    if let Some(reason) = reason {
        return SccpRecipientClassV1::Uncreditable { account, reason };
    }
    let registered = world.account(&account).is_ok();
    if registered {
        let receivable = escrow::xor_quantity(amount).is_ok_and(|quantity| {
            state_transaction
                .world
                .precheck_numeric_asset_receivable(
                    &AssetId::of(sccp_taira_xor_asset_definition_id(), account.clone()),
                    &quantity,
                )
                .is_ok()
        });
        if !receivable {
            return SccpRecipientClassV1::Uncreditable {
                account,
                reason: SccpBounceReasonV1::CreditRefused,
            };
        }
    }
    SccpRecipientClassV1::Creditable {
        account,
        registered,
    }
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

/// Register the absent recipient `account` of a credit on `network`'s route and emit
/// `SccpRecipientRegistered` (§4.12.3 step 3, §4.16); return whether it was registered now.
///
/// `message_id` names the inbound or outbound message being settled, `None` for a stranded
/// release.
///
/// # Errors
///
/// Propagates the rejection of the `Register<Account>` execution.
pub fn register_recipient(
    state_transaction: &mut StateTransaction<'_, '_>,
    account: &AccountId,
    network: SccpNetworkV1,
    message_id: Option<[u8; 32]>,
) -> Result<bool, Error> {
    let registered = ensure_registered(state_transaction, account)?;
    if registered {
        state_transaction
            .world
            .emit_events(Some(SccpEvent::RecipientRegistered(
                SccpRecipientRegisteredV1 {
                    account: account.clone(),
                    network,
                    message_id,
                },
            )));
    }
    Ok(registered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};

    #[test]
    fn registration_is_idempotent() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(ensure_registered(&mut stx, &authority(1)), Ok(true));
        assert!(stx.world.account(&authority(1)).is_ok());
        assert_eq!(ensure_registered(&mut stx, &authority(1)), Ok(false));
        let network = SccpNetworkV1::TonMainnet;
        assert_eq!(
            register_recipient(&mut stx, &authority(2), network, Some([1; 32])),
            Ok(true)
        );
        assert_eq!(
            register_recipient(&mut stx, &authority(2), network, Some([1; 32])),
            Ok(false)
        );
        assert!(stx.world.account(&authority(2)).is_ok());
    }

    fn address_bytes(account: &AccountId) -> Vec<u8> {
        AccountAddress::from_account_id(account)
            .and_then(|address| address.canonical_bytes())
            .expect("address")
    }

    #[test]
    fn recipients_are_decoded_and_classified() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(
            classify_recipient(&stx, &[], 1),
            SccpRecipientClassV1::Undecodable
        );
        assert_eq!(
            classify_recipient(&stx, &[0xff, 1, 2, 3], 1),
            SccpRecipientClassV1::Undecodable
        );
        let fresh = authority(1);
        assert_eq!(
            classify_recipient(&stx, &address_bytes(&fresh), 1),
            SccpRecipientClassV1::Creditable {
                account: fresh.clone(),
                registered: false
            }
        );
        escrow::create_route_escrows(&mut stx).expect("escrows");
        let escrow = escrow::escrow_account(
            &stx.network_id,
            iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
        )
        .expect("route");
        assert_eq!(
            classify_recipient(&stx, &address_bytes(&escrow), 1),
            SccpRecipientClassV1::Uncreditable {
                account: escrow,
                reason: SccpBounceReasonV1::EscrowRecipient
            }
        );
    }
}
