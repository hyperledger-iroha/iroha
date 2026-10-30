//! Live obligations that restrictions must not strand (`specs/live_runtime_upgrades.md` §7.2).
//!
//! An asset escrow or lock whose status is still open is a live obligation of the native escrow
//! implementation: its funds sit in protocol custody until one of its terminal instructions runs,
//! and the committed `world.asset_escrows` record is its registration. A restriction never removes
//! that settlement authority (§7.1):
//!
//! - while a definition behind an escrow instruction has admission closed, the instructions that
//!   continue or complete a live escrow still run, because work already admitted continues;
//! - while it is completion-only, the instructions that complete a live escrow still run;
//! - an instruction that opens new custody runs under neither;
//! - a change set whose successor leaves a live escrow without a bound, active and enabled way to
//!   complete lapses at its effective height instead of activating.
//!
//! TODO(§7.2): obligations created by contracts register through a kernel syscall with their
//! captured completion closure, retained in `RootSelectionV1::completion_closures`, and complete
//! under it after a rebinding; obligations transfer under their preserved invariants; pending
//! consent (multisig, §8.2), scheduled triggers, migration cursors and AMX participations become
//! further kinds.

use std::collections::BTreeMap;

use iroha_data_model::{
    escrow::{AssetEscrowStatus, EscrowId},
    isi::{
        InstructionBox,
        escrow::{
            AcceptAssetEscrow, AttestEscrowCondition, CancelAssetEscrow, CancelAssetLock,
            DrawdownAssetLock, ExpireAssetLock, ExpireConditionalEscrow, MarkEscrowPaymentSent,
            OpenEscrowDispute, ReleaseAssetEscrow, ResolveEscrowDispute,
        },
        escrow_control::{
            CancelControlledAssetLockV1, DrawdownControlledAssetLockV1, ExpireControlledAssetLockV1,
        },
        wire_id_of,
    },
    runtime::{
        definition::{DefinitionId, RestrictionV1, RootSelectionV1},
        lifecycle::ObligationKindV1,
    },
};
use mv::storage::StorageReadOnly as _;

/// How an instruction acts on a live obligation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// It advances the obligation and adds a party's exposure, such as a buyer accepting an
    /// offer: work already admitted continues, but completion-only does not run it.
    Continue,
    /// It completes the obligation, or protects a party on the way to completing it, and adds no
    /// new exposure.
    Complete,
}

impl Step {
    /// Whether this step runs on a definition under `restriction` (§7.1). Deprecation never stops
    /// execution, and disabling stops it entirely.
    pub(crate) const fn runs_under(self, restriction: RestrictionV1) -> bool {
        match restriction {
            RestrictionV1::Deprecated(_) | RestrictionV1::AdmissionClosed(_) => true,
            RestrictionV1::CompletionOnly(_) => matches!(self, Self::Complete),
            RestrictionV1::Disabled(_) => false,
        }
    }
}

/// Whether an escrow in `status` is still a live obligation: its custody has not been paid out,
/// refunded or resolved.
pub(crate) const fn is_live(status: AssetEscrowStatus) -> bool {
    match status {
        AssetEscrowStatus::Open
        | AssetEscrowStatus::Accepted
        | AssetEscrowStatus::PaymentSent
        | AssetEscrowStatus::Disputed
        | AssetEscrowStatus::Locked => true,
        AssetEscrowStatus::Released
        | AssetEscrowStatus::Cancelled
        | AssetEscrowStatus::Resolved
        | AssetEscrowStatus::DrawnDown
        | AssetEscrowStatus::Expired => false,
    }
}

/// The escrow `instruction` acts on and how. Instructions that open new custody, and every
/// instruction that is not an escrow instruction, act on no live escrow.
pub(crate) fn escrow_step(instruction: &InstructionBox) -> Option<(EscrowId, Step)> {
    let instruction = instruction.as_any();
    let step = |escrow_id: &EscrowId, step| Some((*escrow_id, step));
    if let Some(accept) = instruction.downcast_ref::<AcceptAssetEscrow>() {
        return step(&accept.escrow_id, Step::Continue);
    }
    if let Some(mark) = instruction.downcast_ref::<MarkEscrowPaymentSent>() {
        return step(&mark.escrow_id, Step::Complete);
    }
    if let Some(release) = instruction.downcast_ref::<ReleaseAssetEscrow>() {
        return step(&release.escrow_id, Step::Complete);
    }
    if let Some(cancel) = instruction.downcast_ref::<CancelAssetEscrow>() {
        return step(&cancel.escrow_id, Step::Complete);
    }
    if let Some(dispute) = instruction.downcast_ref::<OpenEscrowDispute>() {
        return step(&dispute.escrow_id, Step::Complete);
    }
    if let Some(resolve) = instruction.downcast_ref::<ResolveEscrowDispute>() {
        return step(&resolve.escrow_id, Step::Complete);
    }
    if let Some(attest) = instruction.downcast_ref::<AttestEscrowCondition>() {
        return step(&attest.escrow_id, Step::Complete);
    }
    if let Some(expire) = instruction.downcast_ref::<ExpireConditionalEscrow>() {
        return step(&expire.escrow_id, Step::Complete);
    }
    if let Some(drawdown) = instruction.downcast_ref::<DrawdownAssetLock>() {
        return step(&drawdown.escrow_id, Step::Complete);
    }
    if let Some(cancel) = instruction.downcast_ref::<CancelAssetLock>() {
        return step(&cancel.escrow_id, Step::Complete);
    }
    if let Some(expire) = instruction.downcast_ref::<ExpireAssetLock>() {
        return step(&expire.escrow_id, Step::Complete);
    }
    if let Some(drawdown) = instruction.downcast_ref::<DrawdownControlledAssetLockV1>() {
        return step(&drawdown.drawdown.escrow_id, Step::Complete);
    }
    if let Some(cancel) = instruction.downcast_ref::<CancelControlledAssetLockV1>() {
        return step(&cancel.cancel.escrow_id, Step::Complete);
    }
    if let Some(expire) = instruction.downcast_ref::<ExpireControlledAssetLockV1>() {
        return step(&expire.expiry.escrow_id, Step::Complete);
    }
    None
}

/// How `instruction` acts on a live obligation of `world`, if it does.
pub(crate) fn step(
    instruction: &InstructionBox,
    world: &crate::state::WorldTransaction<'_, '_>,
) -> Option<Step> {
    let (escrow_id, step) = escrow_step(instruction)?;
    world
        .asset_escrows
        .get(&escrow_id)
        .is_some_and(|record| is_live(record.status))
        .then_some(step)
}

/// Wire IDs of the instructions that complete a live escrow. The successor of a change set must
/// keep each one bound to an active, enabled interface and implementation while escrows are live.
fn escrow_completions() -> Vec<&'static str> {
    [
        wire_id_of::<MarkEscrowPaymentSent>(),
        wire_id_of::<ReleaseAssetEscrow>(),
        wire_id_of::<CancelAssetEscrow>(),
        wire_id_of::<OpenEscrowDispute>(),
        wire_id_of::<ResolveEscrowDispute>(),
        wire_id_of::<AttestEscrowCondition>(),
        wire_id_of::<ExpireConditionalEscrow>(),
        wire_id_of::<DrawdownAssetLock>(),
        wire_id_of::<CancelAssetLock>(),
        wire_id_of::<ExpireAssetLock>(),
        wire_id_of::<DrawdownControlledAssetLockV1>(),
        wire_id_of::<CancelControlledAssetLockV1>(),
        wire_id_of::<ExpireControlledAssetLockV1>(),
    ]
    .into_iter()
    .map(|wire_id| wire_id.expect("escrow completions are registered built-in instructions"))
    .collect()
}

/// Live obligations of committed State, counted by kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LiveObligations(BTreeMap<ObligationKindV1, usize>);

impl LiveObligations {
    /// Live obligations of `world`.
    pub(crate) fn of(world: &crate::state::WorldTransaction<'_, '_>) -> Self {
        let escrows = world
            .asset_escrows
            .iter()
            .filter(|(_, record)| is_live(record.status))
            .count();
        Self::with_escrows(escrows)
    }

    /// `escrows` live escrow obligations and nothing else.
    pub(crate) fn with_escrows(escrows: usize) -> Self {
        let mut kinds = BTreeMap::new();
        if escrows > 0 {
            kinds.insert(ObligationKindV1::Escrow, escrows);
        }
        Self(kinds)
    }
}

/// Whether `selection` leaves `definition` active and not disabled.
fn executes(selection: &RootSelectionV1, definition: DefinitionId) -> bool {
    let disabled = selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(&definition))
        .is_ok_and(|index| {
            matches!(
                selection.restrictions[index].restriction,
                RestrictionV1::Disabled(_)
            )
        });
    selection.active.binary_search(&definition).is_ok() && !disabled
}

/// Why `selection` would strand the `live` obligations, if it would: a completion of a live
/// obligation kind that it leaves unbound, or binds to an inactive or disabled definition (§7.2).
pub(crate) fn stranded(selection: &RootSelectionV1, live: &LiveObligations) -> Option<String> {
    for (kind, count) in &live.0 {
        let completions = match kind {
            ObligationKindV1::Escrow => escrow_completions(),
            // TODO(§7.2): these kinds do not register yet; `LiveObligations::of` counts none.
            ObligationKindV1::PendingConsent
            | ObligationKindV1::ScheduledTrigger
            | ObligationKindV1::MigrationCursor
            | ObligationKindV1::AmxParticipation => continue,
        };
        for wire_id in completions {
            let binding = selection
                .bindings
                .binary_search_by(|binding| binding.wire_id.as_str().cmp(wire_id))
                .ok()
                .map(|index| &selection.bindings[index]);
            let gap = match binding {
                None => "is unbound".to_owned(),
                Some(binding) => match [binding.interface, binding.implementation]
                    .into_iter()
                    .find(|definition| !executes(selection, *definition))
                {
                    Some(definition) => {
                        format!("needs {definition}, which is inactive or disabled")
                    }
                    None => continue,
                },
            };
            return Some(format!(
                "{count} live {kind:?} obligations complete through {wire_id}, which {gap}"
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests;
