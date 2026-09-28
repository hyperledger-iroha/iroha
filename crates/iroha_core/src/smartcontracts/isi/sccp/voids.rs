//! Outbound voids: `SubmitSccpOutboundVoidV1` (`specs/sccp.md` §4.16). Owner: ws41.
//!
//! A recorded outbound message that is never minted is recovered by voiding its nonce on the
//! destination and proving the void to Taira. Each voided `Recorded` nonce becomes `Voided`
//! and is refunded (or stranded) through [`super::settle::attempt_refund`]; a frozen void also
//! drains the revision to `InboundOnly`. No refund depends on Taira observing an absence.

use super::{Error, light_clients, registry, settle, store};
use crate::state::StateTransaction;
use iroha_data_model::{
    account::AccountId,
    isi::sccp::SubmitSccpOutboundVoidV1,
    sccp::{
        events::{SccpEvent, SccpOutboundVoidedV1},
        outbound::{SccpOutboundStatusV1, SccpVoidKindV1, SccpVoidStatusV1},
        registry::SccpRouteActivationV1,
    },
};
use iroha_sccp::light_client::proof::SccpNormalizedEventV1;

/// Most nonces one frozen void may name (the destination's range bound).
pub const MAX_VOID_RANGE: u64 = 256;

fn refuse(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(format!("SCCP void: {reason}").into())
}

/// Execute `SubmitSccpOutboundVoidV1` (§4.16).
///
/// # Errors
///
/// Fails when SCCP is absent, the revision is unknown or `Staged`, the proof does not verify
/// or does not prove a void of this deployment, a voided message id disagrees with the
/// record, or the void changes nothing.
pub fn execute_submit_void(
    instruction: SubmitSccpOutboundVoidV1,
    _authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let (network, revision) = (instruction.network, instruction.revision);
    let world = &*state_transaction.world;
    if store::parameters::get(world).is_none() {
        return Err(refuse("SCCP does not exist on this network"));
    }
    let deployment = store::routes::get(world, &network)
        .and_then(|route| route.revisions.get(&revision))
        .filter(|record| record.activation != SccpRouteActivationV1::Staged)
        .map(|record| record.deployment.clone())
        .ok_or_else(|| refuse(format_args!("revision {revision} does not accept proofs")))?;
    let verified =
        light_clients::verify_source_proof(state_transaction, network, &instruction.proof)?;
    let SccpNormalizedEventV1::Void {
        emitter,
        kind,
        first_nonce,
        count,
        message_id_or_zero,
        ..
    } = verified.event
    else {
        return Err(refuse("the proof does not prove a void"));
    };
    if !emitter.matches(&deployment) {
        return Err(refuse("the void was not emitted by this deployment"));
    }
    if count == 0 || count > MAX_VOID_RANGE {
        return Err(refuse(format_args!("void range {count} is out of bounds")));
    }
    let height = state_transaction._curr_block.height().get();
    let mut changed = false;
    for nonce in first_nonce..first_nonce.saturating_add(count) {
        let Some(message_id) =
            store::outbound_by_nonce::get(&*state_transaction.world, &(network, revision, nonce))
                .copied()
        else {
            continue;
        };
        let mut record = store::outbound_messages::get(&*state_transaction.world, &message_id)
            .cloned()
            .ok_or_else(|| refuse("an outbound index entry has no record"))?;
        if !record.status.is_recorded() {
            continue;
        }
        if message_id_or_zero != [0; 32] && message_id_or_zero != message_id {
            return Err(refuse("the voided message id differs from the record"));
        }
        record.status = SccpOutboundStatusV1::Voided(SccpVoidStatusV1 {
            kind,
            proven_at_height: height,
            refund_pending: true,
        });
        store::outbound_messages::insert(state_transaction, message_id, record.clone())?;
        settle::adjust_pending(state_transaction, (network, revision), 0, 1)?;
        state_transaction
            .world
            .emit_events(Some(SccpEvent::OutboundVoided(SccpOutboundVoidedV1 {
                message_id,
                network,
                revision,
                nonce,
                kind,
                refund_pending: true,
            })));
        settle::attempt_refund(state_transaction, message_id)?;
        changed = true;
    }
    if kind == SccpVoidKindV1::Frozen {
        registry::apply_frozen_void(state_transaction, network, revision)?;
        changed = true;
    }
    if changed {
        Ok(())
    } else {
        Err(refuse("the void changes nothing"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{
        SampleInstructions, authority, blank_state, header,
    };

    #[test]
    fn voids_need_sccp() {
        let state = blank_state();
        let mut block = state.block(header(3));
        let mut stx = block.transaction();
        let error = execute_submit_void(SampleInstructions::void(), &authority(1), &mut stx)
            .expect_err("no SCCP");
        assert!(error.to_string().contains("does not exist"), "{error}");
    }
}
