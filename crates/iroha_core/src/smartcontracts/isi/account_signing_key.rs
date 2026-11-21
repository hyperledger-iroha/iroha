//! Off-chain signing keys of accounts: the `iroha.account.signing_key.v1` addition
//! (`specs/account_signing_keys.md`).
//!
//! An account's controller signs only transactions. Its signing key signs everything the
//! account signs off chain. [`SetAccountSigningKeyV1`](iroha_data_model::isi::account_signing_key::SetAccountSigningKeyV1) installs, rotates and revokes that key,
//! and its rules keep the two roles apart:
//!
//! - only the account itself changes its signing key;
//! - a signing key is never a member of its account's controller;
//! - a signing key is never the controller of a registered account;
//! - a key is a signing key once: it is reserved for its account for good, and stays reserved
//!   after a rotation or revocation.
//!
//! A controller change (rekey or recovery) gives the account a new `AccountId`, which has no
//! signing key until it installs one; the old key stays reserved.
//!
//! TODO(`specs/account_signing_keys.md`, "Follow-up"): registering an account, explicitly or
//! implicitly, whose controller is a reserved signing key is not refused yet. That needs a
//! rebind of the baseline registration handlers, which this release cannot dispatch. Such an
//! account gains nothing: no off-chain signature is a valid transaction signature.

use iroha_data_model::account::{
    AccountId,
    signing_key::{AccountSigningKeyRecordV1, AccountSigningKeyV1},
};
use mv::storage::StorageReadOnly;

use crate::state::WorldReadOnly;

mod execute;

/// The active off-chain signing key of `account`, read from committed state.
///
/// `None` when the account has no signing key, or the addition is not active and the table is
/// empty.
pub fn account_signing_key(
    world: &impl WorldReadOnly,
    account: &AccountId,
) -> Option<AccountSigningKeyV1> {
    world
        .account_signing_keys()
        .get(account)
        .and_then(AccountSigningKeyRecordV1::active)
}
