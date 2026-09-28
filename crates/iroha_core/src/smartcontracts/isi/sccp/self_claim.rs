//! Recipient self-claims (`specs/sccp.md` §4.12.4). Owner: ws41.
//!
//! A recipient that holds no XOR claims alone and fee-exempt; `inbound_self_claim_fee` is
//! deducted from the proceeds at release. Admission pre-verifies the claim against committed
//! state: the authority is the payload's recipient, the amount exceeds the fee, and either the
//! proof verifies and the message is not yet proven, or the record is `Pending`.
//!
//! TODO(ws41): admit exempt self-claims that carry `AdvanceSccpLightClientV1` instructions by
//! verifying the proof against the advanced light-client state; such claims currently pay the
//! ordinary fee.

use super::{
    admission::{SccpAdmissionKeysV1, SccpAdmissionRejectV1, SccpExemptClassV1},
    inbound::{bind_transfer_event, decode_inbound_payload, recipient_account},
    light_clients::WorldLightClientView,
    store,
    subjects::SccpStatementDigests,
};
use crate::state::WorldReadOnly;
use iroha_data_model::{
    account::AccountId,
    isi::{
        InstructionBox,
        sccp::{SccpSettleTargetV1, SettleSccpV1, SubmitSccpInboundMessageV1},
    },
    sccp::registry::SccpRouteActivationV1,
    transaction::{Executable, SignedTransaction},
};
use iroha_sccp::v1::payload::SccpTransferPayloadV1;

fn reject(reason: impl Into<String>) -> SccpAdmissionRejectV1 {
    SccpAdmissionRejectV1::new(reason)
}

fn keys(authority: &AccountId, message_id: &[u8; 32]) -> SccpAdmissionKeysV1 {
    SccpAdmissionKeysV1::new(SccpExemptClassV1::SelfClaim)
        .with_exclusive(authority)
        .with_exclusive(message_id)
}

fn check_claimant(
    world: &(impl WorldReadOnly + ?Sized),
    payload: &SccpTransferPayloadV1,
    authority: &AccountId,
) -> Result<(), SccpAdmissionRejectV1> {
    let fee = store::parameters::get(world)
        .as_ref()
        .map(|params| params.inbound_self_claim_fee)
        .ok_or_else(|| reject("SCCP does not exist on this network"))?;
    if recipient_account(payload).as_ref() != Some(authority) {
        return Err(reject("the authority is not the payload's recipient"));
    }
    if payload.amount <= fee {
        return Err(reject("the amount does not exceed the self-claim fee"));
    }
    Ok(())
}

/// Pre-verify a self-claim transaction against committed state and return its admission keys
/// (one pending self-claim per authority and per message id).
///
/// # Errors
///
/// Rejects a transaction that is not a valid self-claim.
pub fn preverify(
    world: &(impl WorldReadOnly + ?Sized),
    digests: &(impl SccpStatementDigests + ?Sized),
    authority: &AccountId,
    transaction: &SignedTransaction,
) -> Result<SccpAdmissionKeysV1, SccpAdmissionRejectV1> {
    let Executable::Instructions(instructions) = &transaction.payload().instructions else {
        return Err(reject("a self-claim carries instructions"));
    };
    let instructions: &[InstructionBox] = instructions;
    let last = instructions
        .last()
        .ok_or_else(|| reject("a self-claim carries instructions"))?;
    let network_id = digests.taira_network_id();
    if let Some(settle) = last.as_any().downcast_ref::<SettleSccpV1>() {
        let SccpSettleTargetV1::Inbound(target) = &settle.target else {
            return Err(reject("only inbound settlements are self-claims"));
        };
        let record = store::inbound_messages::get(world, &target.message_id)
            .ok_or_else(|| reject("unknown inbound message"))?;
        if !record.status.is_pending() {
            return Err(reject("the inbound message is not pending"));
        }
        let payload = SccpTransferPayloadV1::decode(&record.payload)
            .map_err(|error| reject(format!("stored payload: {error}")))?;
        check_claimant(world, &payload, authority)?;
        return Ok(keys(authority, &target.message_id));
    }
    let submit = last
        .as_any()
        .downcast_ref::<SubmitSccpInboundMessageV1>()
        .ok_or_else(|| reject("a self-claim ends with an inbound proof or settlement"))?;
    if instructions.len() > 2 {
        return Err(reject(
            "self-claims with light-client advances pay the ordinary fee (TODO(ws41))",
        ));
    }
    let payload = decode_inbound_payload(submit.network, submit.revision, &submit.payload)
        .map_err(|error| reject(error.to_string()))?;
    check_claimant(world, &payload, authority)?;
    let message_id = payload
        .message_id(network_id.as_bytes())
        .map_err(|error| reject(format!("payload: {error}")))?;
    if store::inbound_messages::contains(world, &message_id) {
        return Err(reject("the message is already proven"));
    }
    let deployment = store::routes::get(world, &submit.network)
        .and_then(|route| route.revisions.get(&submit.revision))
        .filter(|record| record.activation != SccpRouteActivationV1::Staged)
        .map(|record| record.deployment.clone())
        .ok_or_else(|| reject("the revision does not accept proofs"))?;
    let verified = iroha_sccp::light_client::verify_proof(
        &WorldLightClientView(world),
        submit.network,
        &submit.proof,
        digests.committed_time_ms(),
    )
    .map_err(|error| reject(format!("proof: {error}")))?;
    bind_transfer_event(submit.network, &verified, &deployment, &payload, &message_id)
        .map_err(|error| reject(error.to_string()))?;
    Ok(keys(authority, &message_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        authority, blank_state, sample_signed_transaction,
    };

    #[test]
    fn ordinary_transactions_are_not_self_claims() {
        let state = blank_state();
        let view = state.view();
        let reject = preverify(
            &*view.world(),
            &view,
            &authority(1),
            &sample_signed_transaction(),
        )
        .expect_err("not a self-claim");
        assert!(!reject.reason.is_empty());
    }
}
