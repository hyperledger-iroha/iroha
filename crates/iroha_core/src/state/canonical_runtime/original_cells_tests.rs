//! Actual Core source joins preserve undo and reject direct MV source drift.

use super::*;
use crate::{kura::Kura, query::store::LiveQueryStore};

fn state() -> State {
    State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    )
}

#[test]
fn original_runtime_pair_join_keeps_all_four_actual_sources() {
    let state = state();
    let committed = state
        .capture_committed_runtime_cells_once()
        .unwrap()
        .unwrap();
    let acquired = state.acquire_canonical_runtime_block(false).unwrap();
    let joined = committed.bind(acquired).unwrap();
    assert!(joined.commit_topology().0.is_empty());
    assert!(joined.commit_topology().1.is_none());
    assert!(joined.prev_commit_topology().0.is_empty());
    assert!(joined.prev_commit_topology().1.is_none());
    assert_eq!(
        joined.canonical_runtime().0.version,
        SnapshotNexusRuntime::VERSION
    );
    assert!(joined.canonical_runtime().1.is_none());
    assert_eq!(joined.native_execution_tip(), (&None, &None));
    assert_eq!(
        joined.acquired().canonical_runtime.get().version,
        SnapshotNexusRuntime::VERSION
    );
    drop(joined);
    assert!(
        state
            .capture_committed_runtime_cells_once()
            .unwrap()
            .is_some()
    );
}

#[test]
fn committed_committee_undo_survives_ordinary_overlay_initialization() {
    let state = state();
    let peer = PeerId::new(
        iroha_crypto::KeyPair::from_seed(
            b"original-committee".to_vec(),
            iroha_crypto::Algorithm::Ed25519,
        )
        .public_key()
        .clone(),
    );
    let mut original = state.commit_topology.block();
    original.get_mut().push(peer.clone());
    original.commit();
    let committed = state
        .capture_committed_runtime_cells_once()
        .unwrap()
        .unwrap();
    let acquired = state.acquire_canonical_runtime_block(false).unwrap();
    assert_eq!(acquired.fields().commit_topology.original_undo(), &None);
    let joined = committed.bind(acquired).unwrap();
    assert_eq!(joined.commit_topology(), (&vec![peer], &Some(vec![])));
    assert_eq!(joined.acquired().commit_topology.original_undo(), &None);
    drop(joined);
    assert_eq!(
        state.commit_topology.predecessor_view().get(),
        &Some(vec![])
    );
}

#[test]
fn equal_value_direct_mv_republication_refuses_without_generation_change() {
    let state = state();
    let generation = state.state_view_generation();
    let committed = state
        .capture_committed_runtime_cells_once()
        .unwrap()
        .unwrap();
    state
        .commit_topology
        .replace_current_preserving_predecessor(vec![]);
    assert_eq!(state.state_view_generation(), generation);
    let acquired = state.acquire_canonical_runtime_block(false).unwrap();
    assert_eq!(
        committed.bind(acquired).err(),
        Some(OriginalRuntimeJoinError::SourceChanged)
    );
    let fresh = state
        .capture_committed_runtime_cells_once()
        .unwrap()
        .unwrap();
    assert!(
        fresh
            .bind(state.acquire_canonical_runtime_block(false).unwrap())
            .is_ok()
    );
}

#[test]
fn foreign_state_and_active_publication_cannot_supply_original_runtime_cells() {
    let first = state();
    let second = state();
    let committed = first
        .capture_committed_runtime_cells_once()
        .unwrap()
        .unwrap();
    assert_eq!(
        committed
            .bind(second.acquire_canonical_runtime_block(false).unwrap())
            .err(),
        Some(OriginalRuntimeJoinError::ForeignState)
    );
    let mut publication = first.state_view_publication();
    let guard = publication.begin();
    assert!(
        first
            .capture_committed_runtime_cells_once()
            .unwrap()
            .is_none()
    );
    drop(guard);
    assert!(
        first
            .capture_committed_runtime_cells_once()
            .unwrap()
            .is_some()
    );
}
