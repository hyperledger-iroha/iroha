//! Route escrows and their custody guards (`specs/sccp.md` §4.15). Owner: ws32; ws20
//! implemented the escrow identity and recognition.
//!
//! One core-created escrow account per route, derived from the live `NetworkId` without the
//! revision and created at genesis. Core rejects every non-SCCP debit, credit, registration or
//! unregistration of it, and `balance(escrow(route)) = Σ_r liability(r) + stranded(route)`.

use super::{Error, not_wired, store};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{
    NetworkId, account::AccountId, bridge::SccpNetworkV1,
    sccp::escrow::sccp_xor_route_escrow_account_id_v1,
};

/// Return the escrow account of `network`'s route under `network_id`, or `None` for the Taira
/// profile, which has no route.
#[must_use]
pub fn escrow_account(network_id: &NetworkId, network: SccpNetworkV1) -> Option<AccountId> {
    sccp_xor_route_escrow_account_id_v1(network_id, network)
}

/// Create the four route escrow accounts and empty routes (genesis, §4.1).
///
/// # Errors
///
/// Fails closed until ws32 implements escrow creation.
pub fn create_route_escrows(
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws32): register the escrows as core-reserved accounts and insert empty routes.
    Err(not_wired("route escrow creation", "ws32"))
}

/// Lock `amount` Taira units of XOR from `from` into `network`'s route escrow.
///
/// # Errors
///
/// Fails closed until ws32 implements escrow custody.
pub fn lock(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _network: SccpNetworkV1,
    _from: &AccountId,
    _amount: u128,
) -> Result<(), Error> {
    // TODO(ws32): SCCP-only escrow debit/credit path (§4.15).
    Err(not_wired("escrow lock", "ws32"))
}

/// Release `amount` Taira units of XOR from `network`'s route escrow to `to`.
///
/// # Errors
///
/// Fails closed until ws32 implements escrow custody.
pub fn release(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _network: SccpNetworkV1,
    _to: &AccountId,
    _amount: u128,
) -> Result<(), Error> {
    // TODO(ws32): SCCP-only escrow debit/credit path (§4.15).
    Err(not_wired("escrow release", "ws32"))
}

/// Return whether `account` is the escrow of a registered route.
#[must_use]
pub fn is_escrow(world: &(impl WorldReadOnly + ?Sized), account: &AccountId) -> bool {
    store::routes::iter(world).any(|(_, route)| route.escrow == *account)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        authority, blank_state, header, sample_route,
    };

    #[test]
    fn escrows_are_per_route_and_bound_to_the_network_id() {
        let state = blank_state();
        let network_id = state.network_id;
        let eth = escrow_account(&network_id, SccpNetworkV1::EthereumMainnet).expect("route");
        let ton = escrow_account(&network_id, SccpNetworkV1::TonMainnet).expect("route");
        assert_ne!(eth, ton);
        assert_eq!(escrow_account(&network_id, SccpNetworkV1::SoraTaira), None);
    }

    #[test]
    fn only_registered_route_escrows_are_recognized() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let route = sample_route(SccpNetworkV1::BscMainnet);
        let escrow = route.escrow.clone();
        assert!(!is_escrow(&*stx.world, &escrow));
        store::routes::insert(&mut stx, SccpNetworkV1::BscMainnet, route).expect("route");
        assert!(is_escrow(&*stx.world, &escrow));
        assert!(!is_escrow(&*stx.world, &authority(1)));
        let error =
            lock(&mut stx, SccpNetworkV1::BscMainnet, &authority(1), 1).expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws32)"), "{error}");
    }
}
