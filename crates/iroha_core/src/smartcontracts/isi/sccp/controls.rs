//! Destination control messages (`specs/sccp.md` §4.14.6). Owner: ws33.
//!
//! An enacted `SetDestinationPaused` consumes `next_control_nonce(r)`, sets
//! `destination_paused(r)`, allocates a control leaf in the enacting block (see
//! [`super::leaves`]) and records `sccp_control_messages[(network, r, control_nonce)]`.

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_data_model::bridge::SccpNetworkV1;

/// Record a destination control of `(network, revision)` enacted by `proposal_id` and return
/// its control nonce.
///
/// # Errors
///
/// Fails closed until ws33 implements control recording.
pub fn record_control(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _network: SccpNetworkV1,
    _revision: u32,
    _paused: bool,
    _proposal_id: [u8; 32],
) -> Result<u64, Error> {
    // TODO(ws33): nonce, control leaf, record and `SccpControlRecorded` (§4.14.6).
    Err(not_wired("destination control recording", "ws33"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header};

    #[test]
    fn control_recording_fails_closed_until_implemented() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let error = record_control(&mut stx, SccpNetworkV1::TonMainnet, 1, true, [1; 32])
            .expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws33)"), "{error}");
    }
}
