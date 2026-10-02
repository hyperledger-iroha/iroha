//! AMX two-phase commit on the global chain (`specs/sumeragi.md` §11).
//!
//! The global chain `G` keeps its AMX state in the World cell `sumeragi_amx`
//! ([`iroha_data_model::sumeragi_amx::SumeragiAmxState`]) and changes it only through the pure, deterministic transitions of that
//! state. The executor requires the authenticated global root for all four coordinator
//! instructions, including genesis; a private dataspace bootstrap cannot install this role:
//!
//! - `RegisterAmxDataspaceV1` registers a participant dataspace with a foreign-committee tracker
//!   anchored at an authenticated epoch context of its consensus instance (in genesis, or by an
//!   authority holding `CanSetParameters`);
//! - `BeginAmxV1` records `Begin{x, participants, d}` (a second `Begin` for `x` is rejected);
//! - `RelayAmxPreparedV1` verifies a participant's `Prepared` record proof with its tracker; the
//!   first `No` decides `Abort`, the `Yes` that completes the votes at a height `≤ d` decides
//!   `Commit`, and later proofs are ignored;
//! - `RelayAmxHandoffV1` advances a participant's tracker by one epoch;
//! - after every block's transactions, [`StateBlock::advance_sumeragi_amx`] (the output seal's
//!   finalizer) aborts every transaction whose deadline passed undecided — in the first block
//!   with a height `> d` — and drops the transactions whose deadline passed.
//!
//! Every record (`Begin`, `Decision`) is also written into the block's execution witness under
//! its reserved key ([`crate::exec_witness::record_write_amx_record`]), so the ordinary-write root
//! of the block's certified result `R` commits it and a participant verifies it with a record
//! proof, without any change to `R`'s layout. Instruction writes land in the transaction's
//! witness overlay and roll back with a rejected transaction.
//! The mandatory native context archive persists each original complete write set under its
//! exact carrier. [`amx_record_proof`] authenticates that carrier and its complete write root
//! before constructing a historical record proof, including after deadline pruning and replay.
//!
//! **Participants.** The participant side ([`iroha_data_model::sumeragi_amx::AmxParticipantStateV1`]
//! over an [`iroha_data_model::sumeragi_amx::AmxEscrow`]) is tested over an in-memory
//! dataspace. Escrow reports host failures separately from protocol rejection: a refused
//! prepare records no vote, and a refused apply/release keeps the original unsettled entry
//! and global-height cursor for retry. Native participant graph funding remains open.
//! Inherited decoder limits remain typed local refusals through anchors, records and certified
//! proofs. An executing instruction retains that refusal outside its canonical result, including
//! when contract code catches the inner error; a refused attempt cannot publish World effects.
//! The node driver can run independent signed dataspace roots, but the daemon does not yet
//! supervise their participant State owners (lane instances still share `G`'s State,
//! `specs/sumeragi_lanes.md` §0). TODO(S6): hosting a native AMX participant needs
//! (1) a per-dataspace World with an `AmxParticipantStateV1` cell anchored at `G`'s genesis
//! context, (2) native `PrepareAmx`/`SettleAmx`/`RelayGlobalHandoff` instructions of that
//! instance that call `prepare`/`settle`/`handoff` and record the returned `Prepared` record into
//! the instance's execution witness exactly as `G` does here, (3) an `AmxEscrow` implementation
//! that locks, applies and releases the leg's World effects, (4) the same original archive
//! ownership for each dataspace executor, and (5) relayers — the payload builders of each
//! instance's validators — that include pending proofs (§11.4). The existing historical reader
//! still shares the native receipt proof-graph and tree-scratch resource qualification gap.

pub use crate::query::native_receipts::amx_record_proof;

use iroha_data_model::{
    account::AccountId,
    isi::{
        error::InstructionExecutionError as Error,
        sumeragi_amx::{BeginAmxV1, RegisterAmxDataspaceV1, RelayAmxHandoffV1, RelayAmxPreparedV1},
    },
    permission::Permission,
    sumeragi::epoch::ValidatorEpochContextV1,
    sumeragi_amx::{AmxDecisionV1, AmxError, AmxForeignInstanceV1, AmxRecordV1, AmxRelayOutcome},
    sumeragi_finality::MAX_RESULT_PREIMAGE_BYTES,
};
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::Execute,
    state::{StateBlock, StateTransaction, WorldReadOnly},
};

/// Retain decoder refusal outside every deterministic instruction/result carrier.
fn amx_error(error: AmxError, transaction: &mut StateTransaction<'_, '_>) -> Error {
    if let AmxError::Resource(resource) = &error
        && !cfg!(all(test, sumeragi_core_mutation = "HC19"))
    {
        transaction.arm_local_storage_refusal(crate::state::StateStorageAdmissionError::AmxDecode(
            *resource,
        ));
    }
    Error::InvariantViolation(format!("AMX: {error}").into())
}

/// Record `record` as a World write of the executing block.
fn write(record: &AmxRecordV1) -> Result<(), AmxError> {
    crate::exec_witness::record_write_amx_record(record)
}

/// Borrow the exact canonical `CanSetParameters` grant, directly or through a role.
/// Constructing a token would allocate JSON under the caller's inherited decoder limit.
fn has_permission(world: &impl WorldReadOnly, authority: &AccountId) -> bool {
    let matches = |permission: &Permission| {
        permission.name() == "CanSetParameters" && permission.payload().get().as_str() == "null"
    };
    world
        .account_permissions_iter(authority)
        .is_ok_and(|permissions| permissions.into_iter().any(matches))
        || world.account_roles_iter(authority).any(|role| {
            world
                .roles()
                .get(role)
                .is_some_and(|role| role.permissions().any(matches))
        })
}

/// Decode a trust anchor: one canonical `ValidatorEpochContextV1` frame within the bound of a
/// result preimage, under the same cumulative limits as the certified results that carry it.
fn decode_anchor(bytes: &[u8]) -> Result<ValidatorEpochContextV1, AmxError> {
    if bytes.is_empty() || bytes.len() > MAX_RESULT_PREIMAGE_BYTES {
        return Err(AmxError::Anchor("the anchor exceeds its bound".into()));
    }
    let outer_scope = norito::core::decode_limits_active();
    let canonical = norito::canonical_decode_limits(bytes.len());
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(
            96,
            MAX_RESULT_PREIMAGE_BYTES,
            canonical.max_total_elements(),
            canonical.max_total_allocated_bytes(),
            32,
        ),
    )
    .map_err(|error| {
        if (outer_scope || matches!(error, norito::Error::AllocationFailed { .. }))
            && let Some(resource) = error.decode_resource_error()
        {
            return AmxError::Resource(resource);
        }
        AmxError::Anchor(error.to_string())
    })
}

impl Execute for RegisterAmxDataspaceV1 {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if !state_transaction._curr_block.is_genesis()
            && !has_permission(&state_transaction.world, authority)
        {
            return Err(amx_error(
                AmxError::State("registering an AMX dataspace needs genesis or CanSetParameters"),
                state_transaction,
            ));
        }
        let anchor =
            decode_anchor(&self.anchor).map_err(|error| amx_error(error, state_transaction))?;
        let tracker = AmxForeignInstanceV1::new(self.instance, anchor)
            .map_err(|error| amx_error(error, state_transaction))?;
        state_transaction
            .world
            .sumeragi_amx
            .get_mut()
            .register_dataspace(self.dataspace, tracker)
            .map_err(|error| amx_error(error, state_transaction))
    }
}

impl Execute for BeginAmxV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let height = state_transaction._curr_block.height().get();
        let record = state_transaction
            .world
            .sumeragi_amx
            .get_mut()
            .begin(height, &self.transaction)
            .map_err(|error| amx_error(error, state_transaction))?;
        write(&record).map_err(|error| amx_error(error, state_transaction))
    }
}

impl Execute for RelayAmxPreparedV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let height = state_transaction._curr_block.height().get();
        // Proofs of unknown or decided transactions are ignored without touching the cell.
        let known = state_transaction
            .world
            .sumeragi_amx()
            .transaction(&self.proof.record.tx())
            .is_some_and(|entry| entry.decided.is_none());
        if !known {
            return Ok(());
        }
        let outcome = state_transaction
            .world
            .sumeragi_amx
            .get_mut()
            .relay_prepared(height, &self.proof)
            .map_err(|error| amx_error(error, state_transaction))?;
        if let AmxRelayOutcome::Decided(decision) = outcome {
            write(&AmxRecordV1::Decision(decision))
                .map_err(|error| amx_error(error, state_transaction))?;
        }
        Ok(())
    }
}

impl Execute for RelayAmxHandoffV1 {
    fn execute(
        self,
        _authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state_transaction
            .world
            .sumeragi_amx
            .get_mut()
            .relay_handoff(self.dataspace, &self.proof)
            .map(|_| ())
            .map_err(|error| amx_error(error, state_transaction))
    }
}

impl StateBlock<'_> {
    /// The AMX deadline step of the block this overlay executes (§11.5), run by the output
    /// seal's finalizer after the block's transactions: every transaction whose deadline passed
    /// without a decision is aborted and its `Decision` recorded, and every transaction whose
    /// deadline passed is dropped from the state. A block that has nothing to expire leaves the
    /// cell untouched.
    ///
    /// # Errors
    /// A decision record does not encode (a local bug).
    pub(crate) fn advance_sumeragi_amx(&mut self) -> Result<Vec<AmxDecisionV1>, String> {
        let height = self._curr_block.height().get();
        if !self
            .world
            .sumeragi_amx
            .get()
            .transactions
            .iter()
            .any(|entry| entry.begin.deadline < height)
        {
            return Ok(Vec::new());
        }
        let decisions = self.world.sumeragi_amx.get_mut().expire(height);
        for decision in &decisions {
            write(&AmxRecordV1::Decision(*decision)).map_err(|error| error.to_string())?;
        }
        Ok(decisions)
    }
}

#[cfg(test)]
mod proof_tests;
#[cfg(test)]
mod tests;
