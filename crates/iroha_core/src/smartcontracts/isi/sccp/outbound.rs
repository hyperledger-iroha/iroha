//! Outbound value path: `RecordSccpMessage` (`specs/sccp.md` §4.4). Owner: ws32.
//!
//! Recording checks activation, attestation liveness, amount, sender and recipient rules and
//! the supply cap, locks XOR into the route escrow and then records the message per §4.4 steps
//! 9–14 through [`record_outbound_message`], which inbound bounces (§4.12.5) reuse.

use super::{Error, not_wired};
use crate::state::StateTransaction;
use iroha_data_model::{account::AccountId, bridge::SccpNetworkV1, isi::sccp::RecordSccpMessage};

/// Inputs of §4.4 steps 9–14 for one outbound message on `(network, revision)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundRecordArgsV1 {
    /// External target network.
    pub network: SccpNetworkV1,
    /// Route revision whose next outbound nonce the message consumes.
    pub revision: u32,
    /// Amount in Taira units.
    pub amount: u128,
    /// Taira sender; its `AccountAddress` bytes are the payload's codec-3 sender.
    pub sender: AccountId,
    /// Recipient bytes in the target's codec (§3.1).
    pub recipient: Vec<u8>,
}

/// Execute `RecordSccpMessage`.
///
/// # Errors
///
/// Fails closed until ws32 implements the instruction.
pub fn execute_record(
    _instruction: RecordSccpMessage,
    _authority: &AccountId,
    _state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    // TODO(ws32): §4.4 steps 1–14.
    Err(not_wired("RecordSccpMessage execution", "ws32"))
}

/// Record one outbound message per §4.4 steps 9–14 and return its message id.
///
/// # Errors
///
/// Fails closed until ws32 implements recording.
pub fn record_outbound_message(
    _state_transaction: &mut StateTransaction<'_, '_>,
    _args: OutboundRecordArgsV1,
) -> Result<[u8; 32], Error> {
    // TODO(ws32): nonce, deadline, payload, leaf allocation, records and event (§4.4 9–14).
    Err(not_wired("outbound message recording", "ws32"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{authority, blank_state, header};

    #[test]
    fn recording_fails_closed_until_implemented() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let args = OutboundRecordArgsV1 {
            network: SccpNetworkV1::EthereumMainnet,
            revision: 1,
            amount: 1_000_000_000,
            sender: authority(1),
            recipient: vec![0x22; 20],
        };
        let error = record_outbound_message(&mut stx, args).expect_err("skeleton");
        assert!(error.to_string().contains("TODO(ws32)"), "{error}");
    }
}
