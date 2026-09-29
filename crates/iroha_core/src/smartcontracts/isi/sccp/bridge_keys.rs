//! Bridge keys and `SetSccpBridgeKeyV1` (`specs/sccp.md` §4.2). Owner: ws31; ws20 implemented
//! the pure account/address derivations.
//!
//! The attestor account of a bridge key is the universal single-key account whose controller
//! is the key's compressed secp256k1 public key. An account is a bridge key's account iff its
//! single secp256k1 controller derives (§3.8) an address present in `sccp_bridge_key_owners`.

use super::{Error, store};
use crate::state::{StateReadOnly, StateTransaction, WorldReadOnly};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    isi::sccp::SetSccpBridgeKeyV1,
    sccp::{
        events::{SccpBridgeKeySetV1, SccpEvent},
        keys::SccpBridgeKeyV1,
    },
};
use iroha_sccp::v1::{
    eip712::{BridgeKeyPopFieldsV1, peer_key_hash},
    signature::{address_of, verify_signature},
};

/// Return the authenticated scheduling epoch of `height`; pending successor slots provide
/// no bridge-key authority before their incumbent boundary is certified.
#[must_use]
pub fn current_epoch(world: &(impl WorldReadOnly + ?Sized), height: u64) -> Option<u64> {
    let config = world.consensus_schedule().ready(height).ok()?;
    Some(config.epoch.authorization.epoch)
}

/// Return the epoch of the first height of the Sumeragi core's committed schedule window, the
/// deterministic "current epoch" of fee decisions made without a block height.
#[must_use]
pub fn schedule_epoch(world: &(impl WorldReadOnly + ?Sized)) -> Option<u64> {
    let schedule = world.consensus_schedule();
    let first = schedule.entries().first()?;
    current_epoch(world, first.height())
}

/// Return whether a key registration is fee-exempt (§4.2.3): it comes from the new key's own
/// account and the peer has not used its exempt registration in the current epoch.
#[must_use]
pub fn binding_exempt(
    world: &(impl WorldReadOnly + ?Sized),
    instruction: &SetSccpBridgeKeyV1,
    authority: &AccountId,
) -> bool {
    let Some(public_key) = instruction.public_key else {
        return false;
    };
    if account_of(&public_key).ok().as_ref() != Some(authority) {
        return false;
    }
    let last = store::bridge_keys::get(world, &instruction.peer)
        .and_then(|state| state.last_exempt_binding_epoch);
    last.is_none() || last != schedule_epoch(world)
}

/// Check `instruction` against `world` at `height` (§4.2.2 validation 1–6) and return the
/// validated key, if one is registered (a revocation returns `None`).
///
/// # Errors
///
/// Returns the first violated rule as text.
pub fn check_binding(
    world: &(impl WorldReadOnly + ?Sized),
    network_id: &NetworkId,
    height: u64,
    genesis: bool,
    instruction: &SetSccpBridgeKeyV1,
    authority: &AccountId,
) -> Result<Option<SccpBridgeKeyV1>, String> {
    if store::parameters::get(world).is_none() {
        return Err("SCCP does not exist on this network".into());
    }
    // 1. The peer is registered and not barred.
    if !world.peers().iter().any(|peer| peer == &instruction.peer) {
        return Err("the peer is not registered".into());
    }
    let state = store::bridge_keys::get(world, &instruction.peer)
        .cloned()
        .unwrap_or_default();
    if state.is_barred() {
        return Err("the peer is barred by a recorded fault".into());
    }
    // 2. The binding nonce is the next one.
    if instruction.binding_nonce != state.next_binding_nonce {
        return Err(format!(
            "binding nonce {} differs from the expected {}",
            instruction.binding_nonce, state.next_binding_nonce
        ));
    }
    // 3. The peer's consensus key consents to the exact binding.
    let binding = instruction.binding(*network_id);
    instruction
        .peer_signature
        .verify(instruction.peer.public_key(), &binding)
        .map_err(|_| "the peer signature does not verify over the binding".to_owned())?;
    // 6. Activation: epoch 0 inside genesis, otherwise a future epoch.
    if genesis {
        if instruction.activation_epoch != 0 {
            return Err("genesis bindings activate at epoch 0".into());
        }
    } else {
        let epoch = current_epoch(world, height)
            .ok_or_else(|| "the current epoch is unknown".to_owned())?;
        if instruction.activation_epoch <= epoch {
            return Err(format!(
                "activation epoch {} is not after the current epoch {epoch}",
                instruction.activation_epoch
            ));
        }
    }
    let Some(public_key) = instruction.public_key else {
        // A revocation: any authority, ordinary fee.
        return Ok(None);
    };
    // 4. The key is a valid point whose proof of possession recovers its address.
    let address = address_of(&public_key)
        .map_err(|_| "the bridge key is not a valid compressed secp256k1 point".to_owned())?;
    let pop = instruction
        .key_pop
        .ok_or_else(|| "a key registration needs its proof of possession".to_owned())?;
    let (_, peer_key_bytes) = instruction.peer.public_key().to_bytes();
    let pop_digest = BridgeKeyPopFieldsV1 {
        peer_key_hash: peer_key_hash(&peer_key_bytes),
        bridge_address: address,
        activation_epoch: instruction.activation_epoch,
    }
    .digest(network_id.as_bytes());
    verify_signature(&pop_digest, &pop, &address)
        .map_err(|error| format!("the key proof of possession does not verify: {error}"))?;
    if store::bridge_key_owners::contains(world, &address) {
        return Err("the bridge key address was used before".into());
    }
    // 5. Outside genesis the key's own account submits the registration.
    if !genesis && authority != &account_of(&public_key).map_err(|error| error.to_string())? {
        return Err("a key registration must come from the key's own account".into());
    }
    Ok(Some(SccpBridgeKeyV1 {
        public_key,
        address,
        activation_epoch: instruction.activation_epoch,
        registered_at_height: height,
        faulted: false,
    }))
}

/// Execute `SetSccpBridgeKeyV1` (§4.2.2).
///
/// A registration stages the key as pending (genesis promotes it at once), burns its address
/// in the permanent owner index and registers the key's own account when it does not exist; a
/// revocation stages the retirement of the active key from its activation epoch. Emits
/// `SccpBridgeKeySet`.
///
/// # Errors
///
/// Fails when any §4.2.2 rule is violated, or when the key's account cannot be registered.
pub fn execute_set_bridge_key(
    instruction: SetSccpBridgeKeyV1,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let height = state_transaction._curr_block.height().get();
    let genesis = state_transaction._curr_block.is_genesis();
    let network_id = *state_transaction.network_id();
    let key = check_binding(
        &*state_transaction.world,
        &network_id,
        height,
        genesis,
        &instruction,
        authority,
    )
    .map_err(|reason| Error::InvariantViolation(format!("SCCP bridge key: {reason}").into()))?;
    let exempt = !genesis && binding_exempt(&*state_transaction.world, &instruction, authority);
    let mut state = store::bridge_keys::get(&*state_transaction.world, &instruction.peer)
        .cloned()
        .unwrap_or_default();
    state.next_binding_nonce = state.next_binding_nonce.saturating_add(1);
    if exempt {
        state.last_exempt_binding_epoch = schedule_epoch(&*state_transaction.world);
    }
    let account = match key {
        Some(key) => {
            store::bridge_key_owners::insert(
                state_transaction,
                key.address,
                instruction.peer.clone(),
            )?;
            state.stage_key(key);
            if genesis {
                state.promote_for_epoch(0);
            }
            let account = account_of(&key.public_key)?;
            super::recipients::ensure_registered(state_transaction, &account)?;
            Some(account)
        }
        None => {
            state.stage_revocation(instruction.activation_epoch);
            None
        }
    };
    store::bridge_keys::insert(state_transaction, instruction.peer.clone(), state)?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::BridgeKeySet(SccpBridgeKeySetV1 {
            peer: instruction.peer,
            address: key.map(|key| key.address),
            account,
            activation_epoch: instruction.activation_epoch,
        })));
    Ok(())
}

/// Return the attestor account of the bridge key `public_key` (§4.2.1).
///
/// # Errors
///
/// Fails when `public_key` is not a canonical compressed secp256k1 point.
pub fn account_of(public_key: &[u8; 33]) -> Result<AccountId, Error> {
    PublicKey::from_bytes(Algorithm::Secp256k1, public_key)
        .map(AccountId::new)
        .map_err(|error| {
            Error::InvariantViolation(
                format!("SCCP bridge key is not a compressed secp256k1 point: {error}").into(),
            )
        })
}

/// Return the §3.8 address of `account`'s single secp256k1 controller, if it has one.
///
/// This does not check `sccp_bridge_key_owners`; callers look the address up to recognize a
/// bridge key's account.
#[must_use]
pub fn bridge_key_address_of(account: &AccountId) -> Option<[u8; 20]> {
    let (algorithm, payload) = account.try_signatory()?.to_bytes();
    if algorithm != Algorithm::Secp256k1 {
        return None;
    }
    let compressed: [u8; 33] = payload.try_into().ok()?;
    iroha_sccp::v1::signature::address_of(&compressed).ok()
}

/// Promote the bridge keys pending for `epoch` at the boundary block ending `epoch − 1`
/// (§4.3.2 step 1): each pending key activating at or before `epoch` becomes active and the
/// previous active key is retired; each pending revocation due by `epoch` retires the active key.
///
/// # Errors
///
/// Fails only when a promoted state cannot be stored.
pub fn promote_pending_for_epoch(
    state_transaction: &mut StateTransaction<'_, '_>,
    epoch: u64,
) -> Result<(), Error> {
    let due: Vec<_> = super::store::bridge_keys::iter(&*state_transaction.world)
        .filter(|(_, state)| {
            state
                .pending
                .is_some_and(|key| key.activation_epoch <= epoch)
                || state
                    .pending_revocation_epoch
                    .is_some_and(|revocation| revocation <= epoch)
        })
        .map(|(peer, state)| (peer.clone(), state.clone()))
        .collect();
    for (peer, mut state) in due {
        if state.promote_for_epoch(epoch) {
            super::store::bridge_keys::insert(state_transaction, peer, state)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};
    use iroha_crypto::KeyPair;

    fn secp256k1_key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Secp256k1)
            .expect("deterministic secp256k1 seed")
    }

    fn compressed(key_pair: &KeyPair) -> [u8; 33] {
        let (algorithm, payload) = key_pair.public_key().to_bytes();
        assert_eq!(algorithm, Algorithm::Secp256k1);
        payload
            .try_into()
            .expect("canonical compressed secp256k1 key")
    }

    #[test]
    fn account_of_is_the_single_key_secp256k1_account() {
        let key_pair = secp256k1_key(3);
        let account = account_of(&compressed(&key_pair)).expect("valid point");
        assert_eq!(account, AccountId::new(key_pair.public_key().clone()));
        assert!(account_of(&[0x05; 33]).is_err(), "not a point encoding");
    }

    #[test]
    fn bridge_key_address_matches_the_sccp_address_derivation() {
        let key_pair = secp256k1_key(4);
        let key = compressed(&key_pair);
        let account = account_of(&key).expect("valid point");
        assert_eq!(
            bridge_key_address_of(&account),
            iroha_sccp::v1::signature::address_of(&key).ok()
        );
        let ed25519 = KeyPair::try_from_seed(vec![4; 32], Algorithm::Ed25519).expect("seed");
        assert_eq!(
            bridge_key_address_of(&AccountId::new(ed25519.public_key().clone())),
            None
        );
    }

    #[test]
    fn promotion_activates_due_keys_and_revocations_only() {
        use super::super::store;
        use crate::smartcontracts::isi::sccp::test_support::peer;
        use iroha_data_model::sccp::keys::{SccpBridgeKeyStateV1, SccpBridgeKeyV1};

        let key = |address: u8, epoch: u64| SccpBridgeKeyV1 {
            public_key: [2; 33],
            address: [address; 20],
            activation_epoch: epoch,
            registered_at_height: 1,
            faulted: false,
        };
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut rotating = SccpBridgeKeyStateV1::default();
        rotating.active = Some(key(1, 0));
        rotating.stage_key(key(2, 3));
        store::bridge_keys::insert(&mut stx, peer(1), rotating).expect("state");
        let mut later = SccpBridgeKeyStateV1::default();
        later.stage_key(key(3, 5));
        store::bridge_keys::insert(&mut stx, peer(2), later).expect("state");
        let mut revoking = SccpBridgeKeyStateV1::default();
        revoking.active = Some(key(4, 0));
        revoking.stage_revocation(3);
        store::bridge_keys::insert(&mut stx, peer(3), revoking).expect("state");

        promote_pending_for_epoch(&mut stx, 3).expect("promotion");
        let first = store::bridge_keys::get(&*stx.world, &peer(1)).expect("state");
        assert_eq!(first.active.map(|key| key.address), Some([2; 20]));
        assert_eq!(first.retired.len(), 1);
        let second = store::bridge_keys::get(&*stx.world, &peer(2)).expect("state");
        assert!(second.active.is_none(), "epoch 5 key is not due at epoch 3");
        assert!(second.pending.is_some());
        let third = store::bridge_keys::get(&*stx.world, &peer(3)).expect("state");
        assert!(third.active.is_none(), "revocation retired the key");
        assert_eq!(third.retired.len(), 1);
    }
}
