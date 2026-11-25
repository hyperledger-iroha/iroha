//! Native installs, rotations and revocations; retained readers live in the parent module.

use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::{
        AccountController, AccountId,
        signing_key::{AccountSigningKeyRecordV1, AccountSigningKeyReservationV1},
    },
    isi::account_signing_key::SetAccountSigningKeyV1,
};
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::{Execute, isi::Error},
    state::{StateTransaction, WorldReadOnly},
};

#[cfg(test)]
use super::account_signing_key;
#[cfg(test)]
use iroha_data_model::account::signing_key::AccountSigningKeyV1;

fn refused(reason: &str) -> Error {
    Error::InvariantViolation(reason.to_owned().into())
}

/// Whether `key` is a member of `controller`.
fn controller_contains(controller: &AccountController, key: &PublicKey) -> bool {
    match controller {
        AccountController::Single(signatory) => signatory == key,
        AccountController::Multisig(policy) => policy
            .members()
            .iter()
            .any(|member| member.public_key() == key),
    }
}

/// The key of `key`'s reservation: the single-key `AccountId` the key would control.
fn reservation_key(key: &PublicKey) -> AccountId {
    AccountId::new(key.clone())
}

/// Mark the reservation of `key` inactive; it stays reserved.
fn retire(state: &mut StateTransaction<'_, '_>, key: &PublicKey) -> Result<(), Error> {
    let reserved = reservation_key(key);
    let mut reservation = state
        .world
        .account_signing_key_reservations
        .get(&reserved)
        .cloned()
        .ok_or_else(|| refused("the active signing key has no reservation"))?;
    reservation.active = false;
    state
        .world
        .account_signing_key_reservations
        .insert(reserved, reservation);
    Ok(())
}

impl Execute for SetAccountSigningKeyV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if *authority != self.account {
            return Err(refused(
                "only the account itself can change its off-chain signing key",
            ));
        }
        if state.world.account(&self.account).is_err() {
            return Err(refused(
                "an off-chain signing key needs a registered account",
            ));
        }
        let previous = state.world.account_signing_keys.get(&self.account).cloned();
        let generation = previous
            .as_ref()
            .map_or(0, |record| record.generation)
            .checked_add(1)
            .ok_or_else(|| refused("the account's signing key generation is exhausted"))?;
        let previous_key = previous.and_then(|record| record.public_key);
        match self.signing_key {
            Some(signing_key) => {
                if controller_contains(self.account.controller(), &signing_key) {
                    return Err(refused(
                        "an off-chain signing key must not be a member of the account's controller",
                    ));
                }
                let reserved = reservation_key(&signing_key);
                if state
                    .world
                    .account_signing_key_reservations
                    .get(&reserved)
                    .is_some()
                {
                    return Err(refused(
                        "this key is or was an off-chain signing key and cannot be installed again",
                    ));
                }
                if state.world.account(&reserved).is_ok() {
                    return Err(refused(
                        "this key controls an account and cannot be an off-chain signing key",
                    ));
                }
                if let Some(old) = previous_key.as_ref() {
                    retire(state, old)?;
                }
                state.world.account_signing_key_reservations.insert(
                    reserved,
                    AccountSigningKeyReservationV1 {
                        account: self.account.clone(),
                        generation,
                        active: true,
                    },
                );
                state.world.account_signing_keys.insert(
                    self.account,
                    AccountSigningKeyRecordV1 {
                        generation,
                        public_key: Some(signing_key),
                    },
                );
            }
            None => {
                let old = previous_key
                    .ok_or_else(|| refused("the account has no off-chain signing key to revoke"))?;
                retire(state, &old)?;
                state.world.account_signing_keys.insert(
                    self.account,
                    AccountSigningKeyRecordV1 {
                        generation,
                        public_key: None,
                    },
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
