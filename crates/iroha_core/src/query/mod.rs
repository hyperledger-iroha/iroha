//! This module contains [`store::LiveQueryStore`] and helpers.

#![allow(clippy::disallowed_types)]

pub mod cursor;
pub mod pagination;
pub mod snapshot;
pub mod store;

use std::{
    collections::HashMap,
    convert::TryFrom,
    num::{NonZeroU64, NonZeroUsize},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use iroha_data_model::block::consensus::EvidenceRecord;
use mv::storage::StorageReadOnly;

use crate::state::{WorldReadOnly, WorldStateSnapshot};

/// Return the number of persisted evidence entries currently stored in WSV.
pub fn evidence_count(state: &impl WorldStateSnapshot) -> usize {
    evidence_count_from_world(state.world())
}

/// Return the number of persisted evidence entries currently stored in WSV.
pub fn evidence_count_from_world(world: &impl WorldReadOnly) -> usize {
    world.consensus_evidence().iter().count()
}

/// Snapshot persisted evidence records ordered latest-first.
pub fn evidence_list_snapshot(state: &impl WorldStateSnapshot) -> Vec<EvidenceRecord> {
    evidence_list_snapshot_from_world(state.world())
}

/// Snapshot persisted evidence records ordered latest-first.
pub fn evidence_list_snapshot_from_world(world: &impl WorldReadOnly) -> Vec<EvidenceRecord> {
    let mut records: Vec<_> = world
        .consensus_evidence()
        .iter()
        .map(|(_, record)| record.clone())
        .collect();
    records.sort_by(|a, b| {
        (a.recorded_at_height, a.recorded_at_view, a.recorded_at_ms)
            .cmp(&(b.recorded_at_height, b.recorded_at_view, b.recorded_at_ms))
            .reverse()
    });
    records
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[test]
    fn evidence_world_helpers_match_state_snapshot_helpers_on_empty_state() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), Arc::clone(&kura), query);
        let world = state.world_view();
        let view = state.view();

        assert_eq!(evidence_count_from_world(&world), evidence_count(&view));
        assert_eq!(
            evidence_list_snapshot_from_world(&world),
            evidence_list_snapshot(&view),
        );
    }
}

fn next_test_block_height() -> NonZeroUsize {
    static NEXT_HEIGHT: OnceLock<AtomicUsize> = OnceLock::new();
    let next = NEXT_HEIGHT.get_or_init(|| AtomicUsize::new(0));
    let height = next.fetch_add(1, Ordering::SeqCst).saturating_add(1);
    NonZeroUsize::new(height).expect("height non-zero")
}

fn next_height_for_state(state: &mut crate::state::State) -> (NonZeroUsize, NonZeroU64) {
    static STATE_HEIGHTS: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
    let map = STATE_HEIGHTS.get_or_init(|| Mutex::new(HashMap::new()));
    let state_ptr: *mut crate::state::State = state;
    let key = state_ptr as usize;
    let mut guard = map.lock().expect("state height mutex");
    let entry = guard.entry(key).or_insert_with(|| state.committed_height());
    *entry = entry.saturating_add(1);
    let next = *entry;
    let nz_usize = NonZeroUsize::new(next).expect("height non-zero");
    let nz_u64 =
        NonZeroU64::new(u64::try_from(next).expect("height fits u64")).expect("height non-zero");
    (nz_usize, nz_u64)
}

// --- Test utilities (non-consensus; for integration tests) ---
/// Compute double-vote evidence from two votes (helper for tests).
pub fn evidence_check_double_vote(
    v1: &crate::sumeragi::consensus::Vote,
    v2: &crate::sumeragi::consensus::Vote,
) -> Option<crate::sumeragi::consensus::Evidence> {
    crate::sumeragi::evidence::check_double_vote(v1, v2)
}

/// Insert evidence into the WSV-backed store (helper for tests).
pub fn evidence_insert(
    state: &crate::state::State,
    ev: &crate::sumeragi::consensus::Evidence,
    context: &crate::sumeragi::EvidenceValidationContext<'_>,
) -> bool {
    crate::sumeragi::evidence::persist_record(state, ev, context)
}

/// Insert a verifying key record directly into WSV for tests.
pub fn insert_verifying_key_record_for_test(
    state: &mut crate::state::State,
    id: iroha_data_model::proof::VerifyingKeyId,
    rec: iroha_data_model::proof::VerifyingKeyRecord,
) {
    let circuit_key = (rec.circuit_id.clone(), rec.version);
    let height = next_test_block_height();
    let height_u64 = u64::try_from(usize::from(height)).expect("height fits in u64");
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(height_u64).expect("height non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.verifying_keys.insert(id.clone(), rec);
    if !circuit_key.0.trim().is_empty() {
        stx.world.verifying_keys_by_circuit.insert(circuit_key, id);
    }
    stx.mark_confidential_registry_dirty();
    stx.apply();
    block
        .transactions
        .insert_block(std::collections::HashSet::new(), height);
    let _ = block.commit();
}

/// Insert a consensus evidence record directly into WSV for tests.
pub fn insert_evidence_record_for_test(state: &mut crate::state::State, record: EvidenceRecord) {
    let (height, height_u64) = next_height_for_state(state);
    let header = iroha_data_model::block::BlockHeader::new(height_u64, None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    let key = crate::sumeragi::evidence::evidence_key(&record.evidence);
    stx.world.consensus_evidence.insert(key, record);
    stx.apply();
    block
        .transactions
        .insert_block(std::collections::HashSet::new(), height);
    let _ = block.commit();
}

/// Insert a contract instance record directly into WSV for tests.
pub fn insert_contract_instance_for_test(
    state: &mut crate::state::State,
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    code_hash: iroha_crypto::Hash,
) {
    let height = next_test_block_height();
    let height_u64 = u64::try_from(usize::from(height)).expect("height fits in u64");
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(height_u64).expect("height non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world
        .contract_instances
        .insert(contract_address.clone(), code_hash);
    stx.apply();
    block
        .transactions
        .insert_block(std::collections::HashSet::new(), height);
    let _ = block.commit();
}

/// Insert a proof record directly into WSV for tests.
pub fn insert_proof_record_for_test(
    state: &mut crate::state::State,
    id: iroha_data_model::proof::ProofId,
    rec: iroha_data_model::proof::ProofRecord,
) {
    let height = next_test_block_height();
    let height_u64 = u64::try_from(usize::from(height)).expect("height fits in u64");
    let header = iroha_data_model::block::BlockHeader::new(
        NonZeroU64::new(height_u64).expect("height non-zero"),
        None,
        None,
        None,
        0,
        0,
    );
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.proofs.insert(id, rec);
    stx.apply();
    block
        .transactions
        .insert_block(std::collections::HashSet::new(), height);
    let _ = block.commit();
}

/// Insert a governance proposal record for tests.
pub fn insert_gov_proposal_for_test(
    state: &mut crate::state::State,
    id: [u8; 32],
    rec: crate::state::GovernanceProposalRecord,
) {
    use std::collections::HashSet;

    let (next_height, next_height_u64) = next_height_for_state(state);
    let header = iroha_data_model::block::BlockHeader::new(next_height_u64, None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.governance_proposals.insert(id, rec);
    stx.apply();
    block.transactions.insert_block(HashSet::new(), next_height);
    block.commit().expect("commit test block");
}

/// Insert a governance referendum record for tests.
pub fn insert_gov_referendum_for_test(
    state: &mut crate::state::State,
    id: String,
    rec: crate::state::GovernanceReferendumRecord,
) {
    use std::collections::HashSet;

    let (next_height, next_height_u64) = next_height_for_state(state);
    let header = iroha_data_model::block::BlockHeader::new(next_height_u64, None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.governance_referenda.insert(id, rec);
    stx.apply();
    block.transactions.insert_block(HashSet::new(), next_height);
    block.commit().expect("commit test block");
}

/// Insert governance locks for a referendum for tests.
pub fn insert_gov_locks_for_test(
    state: &mut crate::state::State,
    id: String,
    locks: crate::state::GovernanceLocksForReferendum,
) {
    use std::collections::HashSet;

    let (next_height, next_height_u64) = next_height_for_state(state);
    let header = iroha_data_model::block::BlockHeader::new(next_height_u64, None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.world.governance_locks.insert(id, locks);
    stx.apply();
    block.transactions.insert_block(HashSet::new(), next_height);
    block.commit().expect("commit test block");
}
