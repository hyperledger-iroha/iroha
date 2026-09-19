//! Actual decision/checkpoint-to-State publication without a second execution.

use super::super::tests::{acquire, fixture_decision};
use super::*;
use iroha_data_model::events::pipeline::BlockStatus;

#[test]
fn consumes_original_journals_once_with_one_visibility_interval() {
    let (state, decision) = fixture_decision();
    let header = decision.block().header();
    let finality = decision.finality().clone();
    let checkpoint = decision.journals.checkpoint;
    let source = decision.journals.source_prefix.sources().entries().as_ptr();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let physical = acquire(decision, &state);
    let published = physical
        .publish()
        .unwrap_or_else(|(_, error)| panic!("actual publication: {error:?}"));
    assert_eq!(state.state_view_generation(), generation + 2);
    assert_eq!(published.block().header(), header);
    assert_eq!(published.source.sources().entries().as_ptr(), source);
    assert_eq!(published.committed_event.header, header);
    assert_eq!(published.committed_event.status, BlockStatus::Committed);
    assert_eq!(
        published
            .events()
            .iter()
            .filter(|event| matches!(event,
        EventBox::Pipeline(iroha_data_model::events::pipeline::PipelineEventBox::Block(event))
            if event.header == header && event.status == BlockStatus::Applied))
            .count(),
        1
    );
    let after = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    assert_ne!(before, after);
    assert_eq!(after, checkpoint);
    assert_eq!(state.latest_block_hash_fast(), Some(header.hash()));
    assert_eq!(
        state.committed_height(),
        usize::try_from(header.height().get()).unwrap()
    );
    let lease = state.kura.try_publication_lease().unwrap();
    lease
        .reauthenticate_checkpoint(&published.checkpoint, &finality, checkpoint)
        .unwrap();
    drop(lease);
    assert!(state.state_commit_lock.try_lock_or_wait().is_ok());
    assert!(state.state_write_lock.try_lock_or_wait().is_ok());
    drop(state.block(header));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
}

#[test]
fn wrong_retained_header_returns_original_decision_and_releases_every_writer() {
    let (state, mut decision) = fixture_decision();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    // Change only the retained execution header; the original source seal and
    // signed decision remain intact and must not authorize this different header.
    let original = decision.journals.effects.header;
    decision
        .journals
        .effects
        .header
        .set_height(core::num::NonZeroU64::new(original.height().get() + 1).unwrap());
    let physical = acquire(decision, &state);
    let (mut original_decision, error) = physical
        .publish()
        .err()
        .expect("foreign header cannot authorize State");
    assert!(matches!(error, CarrierPublicationError::Source));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    original_decision.journals.effects.header = original;
    let checkpoint = original_decision.journals.checkpoint;
    drop(
        acquire(original_decision, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("same original retry: {error:?}")),
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
}

#[test]
fn prevalidation_returns_owner_without_visibility_then_real_owner_publishes() {
    let (state, mut decision) = fixture_decision();
    let generation = state.state_view_generation();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let checkpoint = decision.journals.checkpoint;
    decision.journals.effects.replay_prevalidation = true;
    let (mut decision, error) = acquire(decision, &state)
        .publish()
        .err()
        .expect("scratch cannot publish");
    assert!(matches!(error, CarrierPublicationError::Prevalidation));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    decision.journals.effects.replay_prevalidation = false;
    drop(
        acquire(decision, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("original owner: {error:?}")),
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
}

#[test]
fn foreign_geometry_returns_original_owner_before_any_visibility_change() {
    let (state, mut decision) = fixture_decision();
    let mut foreign_header = decision.block().header();
    foreign_header.set_view_change_index(foreign_header.view_change_index() + 1);
    let foreign = state
        .merge_preexecution_block(foreign_header)
        .prepare_carrier_geometry()
        .unwrap();
    let original = std::mem::replace(&mut decision.journals.geometry, foreign);
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let checkpoint = decision.journals.checkpoint;
    let (mut decision, error) = acquire(decision, &state)
        .publish()
        .err()
        .expect("foreign geometry cannot authorize State");
    assert!(matches!(error, CarrierPublicationError::Geometry));
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    decision.journals.geometry = original;
    drop(
        acquire(decision, &state)
            .publish()
            .unwrap_or_else(|(_, error)| panic!("same original retry: {error:?}")),
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        checkpoint
    );
}
