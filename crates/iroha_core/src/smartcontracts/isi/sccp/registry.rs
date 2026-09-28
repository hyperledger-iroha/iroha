//! Route registry and activation states (`specs/sccp.md` §4.14.1, §4.14.2). Owner: ws33; ws20
//! implemented the read accessors.
//!
//! `Staged` → `Bidirectional` ⇄ `Paused`; `Bidirectional`/`Paused` → `InboundOnly` →
//! `Retired`. Every transition except the automatic frozen-void transition is Parliament
//! enacted. At most one revision per route is `Bidirectional` or `Paused`.

use super::{Error, not_wired, store};
use crate::state::{StateTransaction, WorldReadOnly};
use iroha_data_model::{bridge::SccpNetworkV1, sccp::registry::SccpRouteActivationV1};

/// Return the route revision of `network` that is `Bidirectional`, if any.
#[must_use]
pub fn bidirectional_revision(
    world: &(impl WorldReadOnly + ?Sized),
    network: SccpNetworkV1,
) -> Option<u32> {
    store::routes::get(world, &network)
        .and_then(|route| route.bidirectional_revision())
        .map(|revision| revision.revision)
}

/// Move revision `revision` of `network` to activation state `to`, emitting
/// `RevisionActivationChanged`.
///
/// # Errors
///
/// Fails closed until ws33 implements activation transitions.
pub fn set_activation(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _network: SccpNetworkV1,
    _revision: u32,
    _to: SccpRouteActivationV1,
) -> Result<(), Error> {
    // TODO(ws33): checked transitions and the activation event (§4.14.2).
    Err(not_wired("route activation transition", "ws33"))
}

/// Apply a proven frozen void: set `destination_frozen` and move a live revision to
/// `InboundOnly` (§4.14.2, §4.16).
///
/// # Errors
///
/// Fails closed until ws33 implements the transition.
pub fn apply_frozen_void(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _network: SccpNetworkV1,
    _revision: u32,
) -> Result<(), Error> {
    // TODO(ws33): automatic frozen-void transition (§4.14.2).
    Err(not_wired("frozen-void transition", "ws33"))
}

/// Return whether the latest destination control of `(network, revision)` pauses minting
/// (§4.14.6). An unknown revision is not paused.
#[must_use]
pub fn destination_paused(
    world: &(impl WorldReadOnly + ?Sized),
    network: SccpNetworkV1,
    revision: u32,
) -> bool {
    store::routes::get(world, &network)
        .and_then(|route| route.revisions.get(&revision))
        .is_some_and(|revision| revision.destination_paused)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header, sample_route};

    #[test]
    fn accessors_read_the_stored_route() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let network = SccpNetworkV1::EthereumMainnet;
        assert_eq!(bidirectional_revision(&*stx.world, network), None);
        assert!(!destination_paused(&*stx.world, network, 1));
        let mut route = sample_route(network);
        let revision = route.revisions.get_mut(&1).expect("sample revision");
        revision.activation = SccpRouteActivationV1::Bidirectional;
        revision.destination_paused = true;
        store::routes::insert(&mut stx, network, route).expect("route");
        assert_eq!(bidirectional_revision(&*stx.world, network), Some(1));
        assert!(destination_paused(&*stx.world, network, 1));
        assert!(!destination_paused(&*stx.world, network, 2));
        let error = apply_frozen_void(&mut stx, network, 1).expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws33)"), "{error}");
    }
}
