//! Destination control messages (`specs/sccp.md` §4.14.6). Owner: ws33.
//!
//! An enacted `SetDestinationPaused` consumes `next_control_nonce(r)`, sets
//! `destination_paused(r)`, allocates a control leaf in the enacting block (see
//! [`super::leaves`]) and records `sccp_control_messages[(network, r, control_nonce)]`.
//! Controls are recorded whether or not SCCP is `enabled`, for any revision that is not
//! `Retired`, and are never pruned. Certificates execute at block start, so a block's control
//! leaves precede its transfer leaves.

use super::{Error, leaves, store};
use crate::state::{StateReadOnly, StateTransaction};
use iroha_data_model::{
    bridge::SccpNetworkV1,
    sccp::{
        control::{SccpControlLeafRefV1, SccpControlRecordV1, SccpLeafRefV1},
        events::{SccpControlRecordedV1, SccpEvent},
        registry::SccpRouteActivationV1,
    },
};
use iroha_sccp::v1::hashes::control_leaf;

fn refuse(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("SCCP: destination control refused: {reason}").into())
}

/// Record a destination control of `(network, revision)` enacted by `proposal_id` and return
/// its control nonce.
///
/// # Errors
///
/// Fails when the revision does not exist or is `Retired`, or the block already holds the
/// maximum number of leaves.
pub fn record_control(
    state_transaction: &mut StateTransaction<'_, '_>,
    network: SccpNetworkV1,
    revision: u32,
    paused: bool,
    proposal_id: [u8; 32],
) -> Result<u64, Error> {
    let mut route = store::routes::get(&*state_transaction.world, &network)
        .cloned()
        .ok_or_else(|| refuse(format_args!("no route to {}", network.profile_key())))?;
    let record = route
        .revisions
        .get_mut(&revision)
        .filter(|record| record.activation != SccpRouteActivationV1::Retired)
        .ok_or_else(|| refuse(format_args!("revision {revision} is absent or retired")))?;
    let control_nonce = record.next_control_nonce;
    let leaf = control_leaf(
        state_transaction.network_id().as_bytes(),
        network,
        &record.destination_word,
        revision,
        control_nonce,
        paused,
    )
    .map_err(|error| refuse(format_args!("control leaf: {error}")))?;
    record.next_control_nonce = control_nonce
        .checked_add(1)
        .ok_or_else(|| refuse("the control nonce is exhausted"))?;
    record.destination_paused = paused;
    store::routes::insert(state_transaction, network, route)?;
    let height = state_transaction._curr_block.height().get();
    let commitment_index = leaves::allocate_leaf(
        state_transaction,
        SccpLeafRefV1::Control(SccpControlLeafRefV1 {
            network,
            revision,
            control_nonce,
        }),
    )?;
    store::control_messages::insert(
        state_transaction,
        (network, revision, control_nonce),
        SccpControlRecordV1 {
            paused,
            height,
            commitment_index,
            leaf,
            proposal_id,
        },
    )?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::ControlRecorded(SccpControlRecordedV1 {
            network,
            revision,
            control_nonce,
            paused,
            height,
            commitment_index,
        })));
    Ok(control_nonce)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header, sample_route};

    #[test]
    fn controls_consume_nonces_and_leaves_and_skip_retired_revisions() {
        let state = blank_state();
        let mut block = state.block(header(7));
        let mut stx = block.transaction();
        let network = SccpNetworkV1::TonMainnet;
        record_control(&mut stx, network, 1, true, [1; 32]).expect_err("no route");
        store::routes::insert(&mut stx, network, sample_route(network)).expect("route");
        let first = record_control(&mut stx, network, 1, true, [1; 32]).expect("pause");
        let second = record_control(&mut stx, network, 1, false, [2; 32]).expect("resume");
        assert_eq!(second, first + 1);
        let revision = store::routes::get(&*stx.world, &network)
            .and_then(|route| route.revisions.get(&1))
            .expect("revision");
        assert!(!revision.destination_paused);
        assert_eq!(revision.next_control_nonce, second + 1);
        let record =
            store::control_messages::get(&*stx.world, &(network, 1, first)).expect("record");
        assert!(record.paused);
        assert_eq!((record.height, record.commitment_index), (7, 0));
        assert_eq!(record.proposal_id, [1; 32]);
        assert_eq!(
            record.leaf,
            control_leaf(
                stx.network_id().as_bytes(),
                network,
                &revision.destination_word,
                1,
                first,
                true
            )
            .expect("leaf")
        );
        assert_eq!(leaves::leaf_count_at(&*stx.world, 7), 2);
        record_control(&mut stx, network, 2, true, [3; 32]).expect_err("unknown revision");

        let mut route = store::routes::get(&*stx.world, &network)
            .cloned()
            .expect("route");
        route.revisions.get_mut(&1).expect("revision").activation = SccpRouteActivationV1::Retired;
        store::routes::insert(&mut stx, network, route).expect("route");
        let error = record_control(&mut stx, network, 1, true, [4; 32]).expect_err("retired");
        assert!(error.to_string().contains("retired"), "{error}");
    }
}
