//! Bounded post-execution pruning (`specs/sccp.md` §4.10). Owner: ws30.
//!
//! Attestation signatures are pruned by block time after `attestation_retention_ms`, rotation
//! subjects are kept until the outgoing roster expires plus one day, and light-client
//! checkpoints follow §4.13.1. At most [`MAX_PRUNE_DELETIONS_PER_BLOCK`] records are deleted
//! per block; `sccp_prune_cursor` resumes the scan. Subjects, statuses, commitments and history
//! are permanent; only signatures (and light-client checkpoints) are pruned.

use super::{light_clients, store};
use crate::state::StateTransaction;

/// Per-block work bound of the pruning step (§4.10).
pub const MAX_PRUNE_DELETIONS_PER_BLOCK: usize = 1_024;

/// Rotation-subject signatures outlive the outgoing generation by one day (§4.10).
pub const ROTATION_RETENTION_GRACE_MS: u64 = 86_400_000;

/// Delete the stored signatures of subject `height`, at most `budget`; return
/// `(deleted, complete)`.
fn delete_signatures(
    state_transaction: &mut StateTransaction<'_, '_>,
    height: u64,
    budget: usize,
) -> (usize, bool) {
    let keys: Vec<(u64, u8)> = store::attestation_signatures::range(
        &*state_transaction.world,
        (height, 0)..=(height, u8::MAX),
    )
    .map(|(key, _)| *key)
    .take(budget.saturating_add(1))
    .collect();
    let complete = keys.len() <= budget;
    let mut deleted = 0;
    for key in keys.into_iter().take(budget) {
        store::attestation_signatures::remove(state_transaction, key);
        deleted += 1;
    }
    (deleted, complete)
}

/// Prune expired SCCP records at block time `now_ms`, deleting at most `budget` records, and
/// return the number deleted.
///
/// Ordinary subjects are visited in height order from the cursor and stop at the first one
/// still inside `attestation_retention_ms`; rotation subjects are skipped there and pruned in
/// generation order once the outgoing generation's `valid_until_ms` plus one day has passed.
/// The remaining budget goes to light-client checkpoints.
pub fn prune(
    state_transaction: &mut StateTransaction<'_, '_>,
    now_ms: u64,
    budget: usize,
) -> usize {
    let Some(retention_ms) = store::parameters::get(&*state_transaction.world)
        .as_ref()
        .map(|params| params.attestation_retention_ms)
    else {
        return 0;
    };
    let mut cursor = *store::prune_cursor::get(&*state_transaction.world);
    let mut remaining = budget;

    // Ordinary subjects, in height order.
    let ordinary: Vec<(u64, bool, u64)> =
        store::attestation_subjects::range(&*state_transaction.world, cursor.signatures_height..)
            .map(|(height, subject)| (*height, subject.is_rotation(), subject.timestamp_ms))
            .collect();
    let mut next_signatures_height = cursor.signatures_height;
    for (height, rotation, timestamp_ms) in ordinary {
        if timestamp_ms.saturating_add(retention_ms) >= now_ms {
            break;
        }
        if !rotation {
            let (deleted, complete) = delete_signatures(state_transaction, height, remaining);
            remaining -= deleted;
            if !complete {
                break;
            }
        }
        next_signatures_height = height.saturating_add(1);
        if remaining == 0 {
            break;
        }
    }

    // Rotation subjects, by generation handoff.
    let rotations: Vec<(u64, u64)> = store::rosters::iter(&*state_transaction.world)
        .filter_map(|(_, roster)| Some((roster.handoff_height?, roster.valid_until_ms)))
        .filter(|(height, _)| *height >= cursor.rotation_height)
        .collect();
    let mut next_rotation_height = cursor.rotation_height;
    for (height, valid_until_ms) in rotations {
        if remaining == 0 || valid_until_ms.saturating_add(ROTATION_RETENTION_GRACE_MS) >= now_ms {
            break;
        }
        let (deleted, complete) = delete_signatures(state_transaction, height, remaining);
        remaining -= deleted;
        if !complete {
            break;
        }
        next_rotation_height = height.saturating_add(1);
    }

    cursor = cursor.advanced_to(next_signatures_height, next_rotation_height);
    store::prune_cursor::set(state_transaction, cursor);
    let light_client = light_clients::prune(state_transaction, remaining);
    budget - remaining + light_client
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header, sample_roster};
    use iroha_data_model::sccp::{attestation::SccpAttestationSubjectV1, params::SccpParametersV1};

    fn subject(height: u64, timestamp_ms: u64, rotation: bool) -> SccpAttestationSubjectV1 {
        SccpAttestationSubjectV1 {
            height,
            epoch: 0,
            timestamp_ms,
            sccp_root: [0; 32],
            message_count: 0,
            history_root: [0; 32],
            history_size: 0,
            generation: 1,
            roster_digest: [5; 32],
            next_roster_digest: if rotation { [6; 32] } else { [0; 32] },
        }
    }

    fn seed(stx: &mut StateTransaction<'_, '_>, height: u64, timestamp_ms: u64, rotation: bool) {
        store::attestation_subjects::insert(stx, height, subject(height, timestamp_ms, rotation))
            .expect("subject");
        for index in 0..3 {
            store::attestation_signatures::insert(stx, (height, index), [1; 65]).expect("sig");
        }
    }

    #[test]
    fn nothing_is_pruned_without_sccp() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(prune(&mut stx, u64::MAX, MAX_PRUNE_DELETIONS_PER_BLOCK), 0);
    }

    #[test]
    fn expired_ordinary_signatures_are_pruned_within_budget_and_resume() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut params = SccpParametersV1::taira_default();
        params.attestation_retention_ms = 1_000;
        store::parameters::set(&mut stx, Some(params));
        seed(&mut stx, 1, 100, false);
        seed(&mut stx, 2, 200, false);
        seed(&mut stx, 3, 10_000, false);
        // Budget 4: all of height 1 and one signature of height 2.
        assert_eq!(prune(&mut stx, 5_000, 4), 4);
        assert_eq!(store::attestation_signatures::len(&*stx.world), 5);
        assert_eq!(store::prune_cursor::get(&*stx.world).signatures_height, 2);
        assert_eq!(
            prune(&mut stx, 5_000, 100),
            2,
            "height 2 finishes; height 3 is recent"
        );
        assert_eq!(store::attestation_signatures::len(&*stx.world), 3);
        assert_eq!(store::prune_cursor::get(&*stx.world).signatures_height, 3);
        assert!(
            store::attestation_subjects::contains(&*stx.world, &1),
            "subjects are permanent"
        );
    }

    #[test]
    fn rotation_signatures_outlive_the_outgoing_generation_by_a_day() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        let mut params = SccpParametersV1::taira_default();
        params.attestation_retention_ms = 1_000;
        store::parameters::set(&mut stx, Some(params));
        let mut roster = sample_roster(1, 1);
        roster.handoff_height = Some(4);
        let valid_until = roster.valid_until_ms;
        store::rosters::insert(&mut stx, 1, roster).expect("roster");
        seed(&mut stx, 4, 100, true);
        assert_eq!(
            prune(&mut stx, 50_000, 100),
            0,
            "rotation kept past ordinary retention"
        );
        assert_eq!(store::attestation_signatures::len(&*stx.world), 3);
        let after = valid_until + ROTATION_RETENTION_GRACE_MS + 1;
        assert_eq!(prune(&mut stx, after, 100), 3);
        assert_eq!(store::prune_cursor::get(&*stx.world).rotation_height, 5);
    }
}
