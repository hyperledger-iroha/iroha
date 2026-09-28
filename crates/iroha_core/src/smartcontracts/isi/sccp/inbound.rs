//! Inbound value path: `SubmitSccpInboundMessageV1` (`specs/sccp.md` §4.12.1). Owner: ws41.
//!
//! A burn on the source chain is proven once and settled separately, so a proven burn never
//! becomes unprovable: the proof verifies against the network's light client, binds the
//! payload to the normalized source event, records the message `Pending` and then attempts
//! settlement (§4.12.3). Proving works while SCCP is disabled or the revision is paused; only
//! settlement waits.

use super::{Error, light_clients, settle, store};
use crate::state::{StateReadOnly, StateTransaction};
use iroha_data_model::{
    account::{AccountAddress, AccountId},
    isi::sccp::SubmitSccpInboundMessageV1,
    sccp::{
        events::{SccpEvent, SccpInboundProvenV1},
        inbound::{SccpInboundRecordV1, SccpInboundStatusV1, SccpPendingReasonV1},
        registry::SccpRouteActivationV1,
    },
};
use iroha_sccp::{
    light_client::proof::{SccpNormalizedEventV1, SccpVerifiedProofV1},
    v1::{network::domain, payload::SccpTransferPayloadV1},
};

fn refuse(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("SCCP inbound: {reason}").into())
}

/// Return the Taira recipient `AccountId` of an inbound `payload` (codec 3), if it decodes.
#[must_use]
pub fn recipient_account(payload: &SccpTransferPayloadV1) -> Option<AccountId> {
    AccountAddress::from_canonical_bytes(&payload.recipient.bytes)
        .and_then(|address| address.to_account_id())
        .ok()
}

/// Decode `bytes` as an inbound payload of `(network, revision)` (§4.12.1 step 2).
///
/// # Errors
///
/// Fails when the payload does not decode or names another lane, revision or a deadline.
pub fn decode_inbound_payload(
    network: iroha_data_model::bridge::SccpNetworkV1,
    revision: u32,
    bytes: &[u8],
) -> Result<SccpTransferPayloadV1, Error> {
    let payload = SccpTransferPayloadV1::decode(bytes)
        .map_err(|error| refuse(format_args!("payload: {error}")))?;
    if payload.source_domain != domain(network)
        || payload.dest_domain != 0
        || payload.route_revision != revision
        || payload.deadline_ms != 0
    {
        return Err(refuse(
            "the payload is not an inbound payload of this route revision",
        ));
    }
    Ok(payload)
}

/// Check that the verified proof's event is the burn of `payload` by `deployment` on
/// `network` (§4.12.1 step 4, §4.12.2): an event carrying the payload (ETH, BSC), or a
/// successful `transferToTaira` call from which Taira rebuilds exactly this payload (TRON).
///
/// # Errors
///
/// Fails when the event is not a transfer to Taira of this payload from this deployment.
pub fn bind_transfer_event(
    network: iroha_data_model::bridge::SccpNetworkV1,
    verified: &SccpVerifiedProofV1,
    deployment: &iroha_data_model::sccp::deployment::SccpDeploymentV1,
    payload: &SccpTransferPayloadV1,
    message_id: &[u8; 32],
) -> Result<iroha_data_model::sccp::inbound::SccpSourceLocatorV1, Error> {
    let mismatch = || refuse("the proven event does not match the payload");
    match &verified.event {
        SccpNormalizedEventV1::TransferToTaira {
            emitter,
            message_id: event_message_id,
            sender,
            nonce,
            payload_hash,
            locator,
        } => {
            let expected_hash = payload
                .payload_hash()
                .map_err(|error| refuse(format_args!("payload: {error}")))?;
            if !emitter.matches(deployment)
                || event_message_id != message_id
                || *sender != payload.sender
                || *nonce != payload.nonce
                || *payload_hash != expected_hash
            {
                return Err(mismatch());
            }
            Ok(*locator)
        }
        SccpNormalizedEventV1::TransferCall {
            emitter,
            caller,
            call,
            locator,
        } => {
            let rebuilt = call
                .inbound_payload(network, payload.route_revision, caller)
                .map_err(|_| mismatch())?;
            if !emitter.matches(deployment) || rebuilt != *payload {
                return Err(mismatch());
            }
            Ok(*locator)
        }
        SccpNormalizedEventV1::Void { .. } => {
            Err(refuse("the proof does not prove a transfer to Taira"))
        }
    }
}

/// Charge the self-claim fee on the `Pending` inbound message `message_id` when `authority`
/// is its recipient (§4.12.4): the fee is due once, at release.
///
/// # Errors
///
/// Fails when the stored record cannot be rewritten.
pub fn charge_self_claim_fee(
    state_transaction: &mut StateTransaction<'_, '_>,
    message_id: [u8; 32],
    authority: &AccountId,
) -> Result<(), Error> {
    let world = &*state_transaction.world;
    let Some(mut record) = store::inbound_messages::get(world, &message_id).cloned() else {
        return Ok(());
    };
    let fee = store::parameters::get(world)
        .as_ref()
        .map_or(0, |params| params.inbound_self_claim_fee);
    let is_recipient = SccpTransferPayloadV1::decode(&record.payload)
        .ok()
        .and_then(|payload| recipient_account(&payload))
        .is_some_and(|recipient| recipient == *authority);
    if is_recipient && record.status.is_pending() && record.fee_due < fee {
        record.fee_due = fee;
        store::inbound_messages::insert(state_transaction, message_id, record)?;
    }
    Ok(())
}

/// Execute `SubmitSccpInboundMessageV1` (§4.12.1).
///
/// # Errors
///
/// Fails when SCCP is absent, the revision is unknown or `Staged`, the payload does not bind
/// to the route, the message is already proven, or the proof does not verify.
pub fn execute_submit_inbound(
    instruction: SubmitSccpInboundMessageV1,
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let (network, revision) = (instruction.network, instruction.revision);
    let world = &*state_transaction.world;
    let params = store::parameters::get(world)
        .clone()
        .ok_or_else(|| refuse("SCCP does not exist on this network"))?;
    let deployment = store::routes::get(world, &network)
        .and_then(|route| route.revisions.get(&revision))
        .filter(|record| record.activation != SccpRouteActivationV1::Staged)
        .map(|record| record.deployment.clone())
        .ok_or_else(|| refuse(format_args!("revision {revision} does not accept proofs")))?;
    let payload = decode_inbound_payload(network, revision, &instruction.payload)?;
    let message_id = payload
        .message_id(state_transaction.network_id().as_bytes())
        .map_err(|error| refuse(format_args!("payload: {error}")))?;
    if store::inbound_messages::contains(world, &message_id) {
        return Err(refuse("the message is already proven"));
    }
    let verified =
        light_clients::verify_source_proof(state_transaction, network, &instruction.proof)?;
    let source_locator =
        bind_transfer_event(network, &verified, &deployment, &payload, &message_id)?;
    let fee_due = if recipient_account(&payload).is_some_and(|recipient| recipient == *authority) {
        params.inbound_self_claim_fee.min(payload.amount)
    } else {
        0
    };
    let height = state_transaction._curr_block.height().get();
    store::inbound_messages::insert(
        state_transaction,
        message_id,
        SccpInboundRecordV1 {
            network,
            revision,
            payload: instruction.payload,
            source_locator,
            proven_at_height: height,
            fee_due,
            status: SccpInboundStatusV1::pending(SccpPendingReasonV1::Disabled),
        },
    )?;
    settle::adjust_pending(state_transaction, (network, revision), 1, 0)?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::InboundProven(SccpInboundProvenV1 {
            message_id,
            network,
            revision,
            amount: payload.amount,
            source_locator,
            fee_due,
        })));
    // A hold (disabled, paused or a shortfall) keeps the record `Pending`; proving succeeds.
    settle::settle_inbound(state_transaction, message_id).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        SampleInstructions, authority, blank_state, header,
    };
    use iroha_data_model::bridge::SccpNetworkV1;

    fn inbound_payload(revision: u32, recipient: Vec<u8>) -> SccpTransferPayloadV1 {
        SccpTransferPayloadV1::inbound(
            SccpNetworkV1::EthereumMainnet,
            3,
            revision,
            2_000_000_000,
            vec![0x44; 20],
            recipient,
        )
        .expect("payload")
    }

    fn address_bytes(account: &AccountId) -> Vec<u8> {
        AccountAddress::from_account_id(account)
            .and_then(|address| address.canonical_bytes())
            .expect("address")
    }

    #[test]
    fn inbound_payloads_bind_to_their_lane_and_revision() {
        let payload = inbound_payload(1, address_bytes(&authority(1)));
        let bytes = payload.encode().expect("encode");
        assert_eq!(
            decode_inbound_payload(SccpNetworkV1::EthereumMainnet, 1, &bytes).expect("decode"),
            payload
        );
        decode_inbound_payload(SccpNetworkV1::EthereumMainnet, 2, &bytes).expect_err("revision");
        decode_inbound_payload(SccpNetworkV1::BscMainnet, 1, &bytes).expect_err("network");
        assert_eq!(recipient_account(&payload), Some(authority(1)));
    }

    #[test]
    fn proofs_need_sccp_and_an_unstaged_revision() {
        let state = blank_state();
        let mut block = state.block(header(3));
        let mut stx = block.transaction();
        let error = execute_submit_inbound(SampleInstructions::inbound(), &authority(1), &mut stx)
            .expect_err("no SCCP");
        assert!(error.to_string().contains("does not exist"), "{error}");
    }
}
