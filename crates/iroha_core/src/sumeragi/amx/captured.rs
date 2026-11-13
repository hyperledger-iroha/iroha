//! Selected AMX global transitions prepare exact edits before producing a decision witness.
//!
//! The frozen global model API remains unchanged. These selected operations move the original
//! affected vector into a preallocated destination after the writer accepts the witness; they
//! never clone a global tracker graph as an execution candidate.

use std::alloc::Layout;

use iroha_allocation::{AllocationRefusal, AllocatorRefusal};
use iroha_data_model::sumeragi_amx::{
    AmxDecidedV1, AmxDecisionV1, AmxError, AmxGlobalEntryV1, AmxOutcomeV1, AmxRecordProofV1,
    AmxRecordV1, AmxRelayOutcome, AmxTransactionV1, AmxVoteV1, AmxYesV1, MAX_AMX_DEADLINE_WINDOW,
    MAX_AMX_PENDING, amx_max_time_window_ms,
};

use super::{Error, StateTransaction, WorldReadOnly, amx_error, write};
use crate::{
    execution_attempt::{ExecutionAttemptError, ExecutionDeferred},
    runtime::native_amx_capture,
};

/// A physical destination for exactly one insertion, holding no copied payload graph.
struct Destination<T>(Option<Vec<T>>);
impl<T> Destination<T> {
    fn prepare(current: &Vec<T>) -> Result<Self, ExecutionDeferred> {
        if current.capacity() > current.len() {
            return Ok(Self(None));
        }
        let capacity = current
            .len()
            .checked_add(1)
            .ok_or_else(|| ExecutionDeferred::from(AllocationRefusal::DemandOverflow))?;
        let layout = Layout::array::<T>(capacity)
            .map_err(|_| ExecutionDeferred::from(AllocationRefusal::DemandOverflow))?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(capacity)
            .map_err(|original| {
                ExecutionDeferred::from(AllocatorRefusal::from_vec_reserve(original, layout))
            })?;
        Ok(Self(Some(replacement)))
    }
    fn insert(self, current: &mut Vec<T>, slot: usize, value: T) {
        if let Some(mut replacement) = self.0 {
            replacement.append(current);
            *current = replacement;
        }
        current.insert(slot, value);
    }
}

fn deferred(tx: &mut StateTransaction<'_, '_>, original: ExecutionDeferred) -> Error {
    tx.attempt_error_to_instruction_error(ExecutionAttemptError::Deferred(original))
}

pub(super) fn begin(
    tx: &mut StateTransaction<'_, '_>,
    transaction: &AmxTransactionV1,
    height: u64,
    time_ms: u64,
    cadence_ms: u64,
) -> Result<(), Error> {
    begin_with(tx, transaction, height, time_ms, cadence_ms, write)
}

pub(super) fn begin_with(
    tx: &mut StateTransaction<'_, '_>,
    transaction: &AmxTransactionV1,
    height: u64,
    time_ms: u64,
    cadence_ms: u64,
    record: impl FnOnce(&AmxRecordV1) -> Result<(), AmxError>,
) -> Result<(), Error> {
    let capture = native_amx_capture::prepare_begin(tx, transaction, height, time_ms, cadence_ms)?;
    let begin = transaction.begin().map_err(|error| amx_error(&error, tx))?;
    let time_window = amx_max_time_window_ms(cadence_ms).map_err(|error| amx_error(&error, tx))?;
    let invalid = if begin.expires_at_ms <= time_ms || begin.expires_at_ms - time_ms > time_window {
        Some("the expiry is not within the admitted time window")
    } else if begin.deadline <= height || begin.deadline - height > MAX_AMX_DEADLINE_WINDOW {
        Some("the deadline is not within the admitted window")
    } else if begin
        .participants
        .iter()
        .any(|id| tx.world.sumeragi_amx().dataspace(*id).is_none())
    {
        Some("a participant dataspace is not registered")
    } else {
        None
    };
    if let Some(reason) = invalid {
        return Err(amx_error(&AmxError::State(reason), tx));
    }
    let slot = match tx
        .world
        .sumeragi_amx()
        .transactions
        .binary_search_by_key(&begin.tx, |entry| entry.begin.tx)
    {
        Ok(_) => {
            return Err(amx_error(
                &AmxError::State("the transaction was already begun"),
                tx,
            ));
        }
        Err(slot) => slot,
    };
    if tx.world.sumeragi_amx().transactions.len() >= MAX_AMX_PENDING {
        return Err(amx_error(
            &AmxError::State("too many pending transactions"),
            tx,
        ));
    }
    // Make the existing cell overlay writable before the witness, then prepare its exact
    // physical destination. TODO: replace the preexisting global Cell graph with funded custody.
    let destination = Destination::prepare(&tx.world.sumeragi_amx.get_mut().transactions)
        .map_err(|original| deferred(tx, original))?;
    let witness = AmxRecordV1::Begin(begin);
    record(&witness).map_err(|error| amx_error(&error, tx))?;
    let AmxRecordV1::Begin(begin) = witness else {
        unreachable!("constructed Begin witness")
    };
    destination.insert(
        &mut tx.world.sumeragi_amx.get_mut().transactions,
        slot,
        AmxGlobalEntryV1 {
            begin,
            begun_at: height,
            begun_time_ms: time_ms,
            original_cadence_ms: cadence_ms,
            yes: Vec::new(),
            decided: None,
        },
    );
    capture.publish(&mut tx.world);
    Ok(())
}

pub(super) fn expire(
    tx: &mut StateTransaction<'_, '_>,
    transaction: [u8; 32],
    height: u64,
    time_ms: u64,
) -> Result<(), Error> {
    expire_with(tx, transaction, height, time_ms, write)
}

pub(super) fn expire_with(
    tx: &mut StateTransaction<'_, '_>,
    transaction: [u8; 32],
    height: u64,
    time_ms: u64,
    record: impl FnOnce(&AmxRecordV1) -> Result<(), AmxError>,
) -> Result<(), Error> {
    let completion = native_amx_capture::prepare_completion(
        tx,
        iroha_data_model::runtime::native_amx::NativeAmxKindV1::GlobalDecision,
        transaction,
    )?;
    let slot = tx
        .world
        .sumeragi_amx()
        .transactions
        .binary_search_by_key(&transaction, |entry| entry.begin.tx)
        .map_err(|_| amx_error(&AmxError::State("the expiry obligation is unknown"), tx))?;
    let entry = &tx.world.sumeragi_amx().transactions[slot];
    let invalid = if entry.decided.is_some() {
        Some("the expiry obligation is already decided")
    } else if !entry.begin.is_due(height, time_ms) {
        Some("the expiry obligation is not due")
    } else {
        None
    };
    if let Some(reason) = invalid {
        return Err(amx_error(&AmxError::State(reason), tx));
    }
    let decision = AmxDecisionV1 {
        tx: transaction,
        outcome: AmxOutcomeV1::Abort,
    };
    let _ = tx.world.sumeragi_amx.get_mut();
    record(&AmxRecordV1::Decision(decision)).map_err(|error| amx_error(&error, tx))?;
    tx.world.sumeragi_amx.get_mut().transactions[slot].decided = Some(AmxDecidedV1 {
        outcome: decision.outcome,
        height,
        time_ms,
    });
    completion.publish(&mut tx.world);
    Ok(())
}

pub(super) fn relay(
    tx: &mut StateTransaction<'_, '_>,
    proof: &AmxRecordProofV1,
    height: u64,
    time_ms: u64,
) -> Result<(), Error> {
    relay_with(tx, proof, height, time_ms, write)
}

pub(super) fn relay_with(
    tx: &mut StateTransaction<'_, '_>,
    proof: &AmxRecordProofV1,
    height: u64,
    time_ms: u64,
    record: impl FnOnce(&AmxRecordV1) -> Result<(), AmxError>,
) -> Result<(), Error> {
    let AmxRecordV1::Prepared(prepared) = &proof.record else {
        return Err(amx_error(
            &AmxError::Record("a relayed vote must be a Prepared record"),
            tx,
        ));
    };
    let Ok(index) = tx
        .world
        .sumeragi_amx()
        .transactions
        .binary_search_by_key(&prepared.tx, |entry| entry.begin.tx)
    else {
        return Ok(());
    };
    let entry = &tx.world.sumeragi_amx().transactions[index];
    if entry.decided.is_some() {
        return Ok(());
    }
    let completion = native_amx_capture::prepare_completion(
        tx,
        iroha_data_model::runtime::native_amx::NativeAmxKindV1::GlobalDecision,
        prepared.tx,
    )?;
    let entry = &tx.world.sumeragi_amx().transactions[index];
    if !entry.begin.has_participant(prepared.participant) {
        return Err(amx_error(
            &AmxError::State("the voter is not a participant"),
            tx,
        ));
    }
    let Err(slot) = entry
        .yes
        .binary_search_by_key(&prepared.participant, |yes| yes.participant)
    else {
        return Ok(());
    };
    let Some(registration) = tx.world.sumeragi_amx().dataspace(prepared.participant) else {
        return Err(amx_error(
            &AmxError::State("the voter is not registered"),
            tx,
        ));
    };
    registration
        .tracker
        .verify_record(proof)
        .map_err(|error| amx_error(&error, tx))?;
    let entry = &tx.world.sumeragi_amx().transactions[index];
    let (vote, outcome) = match prepared.vote {
        AmxVoteV1::No => (
            None,
            AmxRelayOutcome::Decided(AmxDecisionV1 {
                tx: prepared.tx,
                outcome: AmxOutcomeV1::Abort,
            }),
        ),
        AmxVoteV1::Yes(effects_hash) => {
            let outcome = if entry.yes.len() + 1 < entry.begin.participants.len()
                || entry.begin.is_due(height, time_ms)
            {
                AmxRelayOutcome::Voted
            } else {
                AmxRelayOutcome::Decided(AmxDecisionV1 {
                    tx: prepared.tx,
                    outcome: AmxOutcomeV1::Commit,
                })
            };
            (
                Some(AmxYesV1 {
                    participant: prepared.participant,
                    effects_hash,
                }),
                outcome,
            )
        }
    };
    let writable = tx.world.sumeragi_amx.get_mut();
    let destination = if vote.is_some() {
        Some(
            Destination::prepare(&writable.transactions[index].yes)
                .map_err(|original| deferred(tx, original))?,
        )
    } else {
        None
    };
    if let AmxRelayOutcome::Decided(decision) = outcome {
        record(&AmxRecordV1::Decision(decision)).map_err(|error| amx_error(&error, tx))?;
    }
    let entry = &mut tx.world.sumeragi_amx.get_mut().transactions[index];
    if let Some((destination, vote)) = destination.zip(vote) {
        destination.insert(&mut entry.yes, slot, vote);
    }
    if let AmxRelayOutcome::Decided(decision) = outcome {
        entry.decided = Some(AmxDecidedV1 {
            outcome: decision.outcome,
            height,
            time_ms,
        });
        completion.publish(&mut tx.world);
    }
    Ok(())
}
