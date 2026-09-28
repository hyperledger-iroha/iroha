//! Bridge keys and `SetSccpBridgeKeyV1` (`specs/sccp.md` §4.2). Owner: ws31; ws20 implemented
//! the pure account/address derivations.
//!
//! The attestor account of a bridge key is the universal single-key account whose controller
//! is the key's compressed secp256k1 public key. An account is a bridge key's account iff its
//! single secp256k1 controller derives (§3.8) an address present in `sccp_bridge_key_owners`.

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{account::AccountId, isi::sccp::SetSccpBridgeKeyV1};

/// Execute `SetSccpBridgeKeyV1` (§4.2.2).
///
/// # Errors
///
/// Fails closed until ws31 implements the instruction.
pub fn execute_set_bridge_key(
    _instruction: SetSccpBridgeKeyV1,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws31): consent, PoP, binding nonce, pending key and implicit account (§4.2.2).
    Err(not_wired("SetSccpBridgeKeyV1 execution", "ws31"))
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
