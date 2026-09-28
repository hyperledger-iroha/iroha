//! Attestation subjects and statements (`specs/sccp.md` §3.6, §4.6). Owner: ws30.
//!
//! The statement of height `h` is its stored subject plus `block_hashes[h]`; its digest is the
//! §3.6 EIP-712 attestation digest under the live `NetworkId`.

use super::store;
use crate::{
    block::BlockValidationError,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};
use iroha_data_model::sccp::{
    attestation::{SccpAttestationStatementV1, SccpAttestationSubjectV1},
    events::{SccpEvent, SccpHandoffStalledV1, SccpSubjectCreatedV1},
};
use iroha_sccp::v1::eip712::AttestationFieldsV1;

/// Write the attestation subject of `subject.height` and emit `SccpSubjectCreated` (§4.6).
///
/// # Errors
///
/// Fails the block when the subject breaks its stored invariant.
pub fn write_subject(
    state_transaction: &mut StateTransaction<'_, '_>,
    subject: SccpAttestationSubjectV1,
) -> Result<(), BlockValidationError> {
    let event = SccpSubjectCreatedV1 {
        height: subject.height,
        generation: subject.generation,
        message_count: subject.message_count,
        rotation: subject.is_rotation(),
    };
    store::attestation_subjects::insert(state_transaction, subject.height, subject).map_err(
        |error| BlockValidationError::ExecutionContextInvalid(format!("SCCP subject: {error}")),
    )?;
    state_transaction
        .world
        .emit_events(Some(SccpEvent::SubjectCreated(event)));
    Ok(())
}

/// Return the committed hash of Taira block `height`, if it is committed.
fn block_hash(view: &impl StateReadOnly, height: u64) -> Option<[u8; 32]> {
    let index = usize::try_from(height.checked_sub(1)?).ok()?;
    view.block_hashes()
        .hash_at(index)
        .map(|hash| *AsRef::<[u8; 32]>::as_ref(hash))
}

/// Return the canonical statement of `height`, or `None` when no subject exists there or the
/// block is not yet committed.
#[must_use]
pub fn statement(view: &impl StateReadOnly, height: u64) -> Option<SccpAttestationStatementV1> {
    let subject = store::attestation_subjects::get(view.world(), &height)?;
    Some(subject.statement(block_hash(view, height)?))
}

/// Return the §3.6.1 attestation fields of `statement`.
#[must_use]
pub fn fields(statement: &SccpAttestationStatementV1) -> AttestationFieldsV1 {
    AttestationFieldsV1 {
        height: statement.height,
        epoch: statement.epoch,
        timestamp_ms: statement.timestamp_ms,
        block_hash: statement.block_hash,
        sccp_root: statement.sccp_root,
        message_count: statement.message_count,
        history_root: statement.history_root,
        history_size: statement.history_size,
        roster_digest: statement.roster_digest,
        next_roster_digest: statement.next_roster_digest,
    }
}

/// Return the §3.6 attestation digest of `height`'s statement under the live `NetworkId`.
#[must_use]
pub fn statement_digest(view: &impl StateReadOnly, height: u64) -> Option<[u8; 32]> {
    let statement = statement(view, height)?;
    Some(fields(&statement).digest(view.network_id().as_bytes()))
}

/// Record every rotation subject that stayed unattested for `attestation_stall_ms` before
/// `now_ms` and emit `SccpHandoffStalled` once per subject (§4.3.3).
///
/// Only rotation heights (the `handoff_height` of each generation) can stall, so the scan is
/// bounded by the number of generations.
///
/// # Errors
///
/// Fails the block only when the stall record cannot be stored.
pub fn record_stalled_handoffs(
    state_transaction: &mut StateTransaction<'_, '_>,
    stall_ms: u64,
    now_ms: u64,
) -> Result<(), BlockValidationError> {
    let world = &*state_transaction.world;
    let stalled: Vec<(u64, u64)> = store::rosters::iter(world)
        .filter_map(|(generation, roster)| Some((*generation, roster.handoff_height?)))
        .filter(|(_, height)| !store::handoff_stalled::contains(world, height))
        .filter(|(_, height)| {
            store::attestation_status::get(world, height)
                .is_none_or(|status| status.attested_at_height.is_none())
        })
        .filter(|(_, height)| {
            store::attestation_subjects::get(world, height)
                .is_some_and(|subject| subject.timestamp_ms.saturating_add(stall_ms) <= now_ms)
        })
        .collect();
    for (generation, height) in stalled {
        store::handoff_stalled::insert(state_transaction, height, generation).map_err(|error| {
            BlockValidationError::ExecutionContextInvalid(format!("SCCP stalled handoff: {error}"))
        })?;
        state_transaction
            .world
            .emit_events(Some(SccpEvent::HandoffStalled(SccpHandoffStalledV1 {
                height,
                generation,
            })));
    }
    Ok(())
}

/// Return whether `world` holds a subject at `height`.
#[must_use]
pub fn exists(world: &(impl WorldReadOnly + ?Sized), height: u64) -> bool {
    store::attestation_subjects::contains(world, &height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header, sample_roster};

    fn subject(height: u64, next: [u8; 32]) -> SccpAttestationSubjectV1 {
        SccpAttestationSubjectV1 {
            height,
            epoch: 0,
            timestamp_ms: height * 4_000,
            sccp_root: [0; 32],
            message_count: 0,
            history_root: [0; 32],
            history_size: 0,
            generation: 1,
            roster_digest: [5; 32],
            next_roster_digest: next,
        }
    }

    #[test]
    fn no_statement_exists_without_a_subject() {
        let state = blank_state();
        let view = state.view();
        assert_eq!(statement(&view, 1), None);
        assert_eq!(statement_digest(&view, 1), None);
    }

    #[test]
    fn subjects_are_written_once_per_height_with_an_event() {
        let state = blank_state();
        let mut block = state.block(header(3));
        let mut stx = block.transaction();
        write_subject(&mut stx, subject(3, [0; 32])).expect("subject");
        assert!(exists(&*stx.world, 3));
        assert!(!exists(&*stx.world, 4));
    }

    #[test]
    fn statement_fields_map_one_to_one() {
        let statement = subject(9, [8; 32]).statement([3; 32]);
        let fields = fields(&statement);
        assert_eq!(fields.height, 9);
        assert_eq!(fields.block_hash, [3; 32]);
        assert_eq!(fields.next_roster_digest, [8; 32]);
        assert!(fields.is_rotation());
    }

    #[test]
    fn unattested_rotation_subjects_stall_once() {
        let state = blank_state();
        let mut block = state.block(header(40));
        let mut stx = block.transaction();
        let mut roster = sample_roster(1, 1);
        roster.handoff_height = Some(10);
        store::rosters::insert(&mut stx, 1, roster).expect("roster");
        write_subject(&mut stx, subject(10, [8; 32])).expect("rotation subject");
        record_stalled_handoffs(&mut stx, 1_000_000, 40_000).expect("not yet stalled");
        assert!(!store::handoff_stalled::contains(&*stx.world, &10));
        record_stalled_handoffs(&mut stx, 1_000, 41_000).expect("stalled");
        assert_eq!(store::handoff_stalled::get(&*stx.world, &10), Some(&1));
        record_stalled_handoffs(&mut stx, 1_000, 99_000).expect("idempotent");
        assert_eq!(store::handoff_stalled::len(&*stx.world), 1);
    }
}
