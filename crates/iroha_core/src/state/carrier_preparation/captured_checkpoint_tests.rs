//! Exact checkpoint identity and retries through the current retained Native publisher.

use super::*;
use crate::{
    block::VerifiedV2FinalityArtifact,
    snapshot::{CapturedStateSnapshot, SnapshotCaptureError},
    state::{RetainedCarrier, tests::native_publication_fixture},
    sumeragi::{
        v2_apply::V2ApplyService,
        v2_body_store::{BodyValidationError, LocalValidationRefusal},
    },
};
use iroha_crypto::Hash;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

struct NativeCheckpointFixture {
    state: Arc<State>,
    service: V2ApplyService,
    owner: Option<RetainedCarrier<()>>,
    finality: VerifiedV2FinalityArtifact,
    checkpoint: Hash,
}

#[inline(never)]
fn native_checkpoint_fixture() -> Box<NativeCheckpointFixture> {
    let fixture = native_publication_fixture(false);
    let prepared = fixture.prepare();
    let finality = fixture.finality(prepared.block(), prepared.execution_prefix_commitment());
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .unwrap();
    let checkpoint = journals.checkpoint;
    let state = fixture.into_shared_state();
    let (events, _) = tokio::sync::broadcast::channel(8);
    let service = V2ApplyService::new(
        Arc::clone(&state),
        phase_queue(),
        Arc::clone(&state.kura),
        None,
        None,
        state.sumeragi_block_cadence(),
        iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        events,
        Vec::new(),
    );
    Box::new(NativeCheckpointFixture {
        state,
        service,
        owner: Some(RetainedCarrier::Validated(journals)),
        finality,
        checkpoint,
    })
}

impl NativeCheckpointFixture {
    fn publish(&mut self) -> Result<crate::state::PublishedCarrier<()>, LocalValidationRefusal> {
        let owner = self
            .owner
            .take()
            .expect("original retained publication owner");
        match owner.try_publish(
            &self.state,
            &self.service.carrier_queue_source(),
            self.finality.clone(),
            Waker::noop().clone(),
        ) {
            Ok(published) => Ok(published),
            Err((owner, refusal)) => {
                self.owner = Some(owner);
                Err(refusal)
            }
        }
    }
}

fn checkpoint_tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if kind.is_dir() {
                assert!(entries.insert(relative, None).is_none());
                visit(root, &path, entries);
            } else {
                assert!(kind.is_file(), "fixture contains no links or special files");
                assert!(
                    entries
                        .insert(relative, Some(std::fs::read(path).unwrap()))
                        .is_none()
                );
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn assert_capture_boundary_refusal(error: SnapshotCaptureError, component: &'static str) {
    assert!(
        matches!(&error, SnapshotCaptureError::CommitBoundary { component: actual } if *actual == component)
    );
    let error = crate::sumeragi::v2_apply::V2ApplyError::SnapshotCapture(error);
    assert!(error.rejection_identity().is_none());
    assert!(matches!(
        error.local_refusal(),
        Some(LocalValidationRefusal::RecoveryRequired(_))
    ));
    let retained = crate::sumeragi::v2_apply::V2ApplyError::LocalValidation(
        error
            .local_refusal()
            .expect("captured State identity is a local refusal"),
    );
    assert!(retained.requires_restart_recovery());
    assert!(retained.rejection_identity().is_none());
}

#[test]
fn retained_publication_checkpoint_rejects_unpublished_cut_without_storage_mutation() {
    let mut fixture = native_checkpoint_fixture();
    let state = Arc::clone(&fixture.state);
    let height = fixture.finality.artifact().height;
    let hash = fixture.finality.artifact().block_hash;
    let network = *state.network_id_ref();
    let original_height = state.committed_height();
    let original = phase_allocations(fixture.owner.as_ref().unwrap());
    state.kura.fail_next_wsv_checkpoint_write_for_tests();
    assert!(matches!(
        fixture.publish(),
        Err(LocalValidationRefusal::RecoveryRequired(_))
    ));
    assert!(matches!(
        fixture.owner.as_ref().unwrap(),
        RetainedCarrier::Decided(_)
    ));
    assert_eq!(phase_allocations(fixture.owner.as_ref().unwrap()), original);
    assert_eq!(state.committed_height(), original_height);
    assert_eq!(
        state.kura.exact_durable_blocks_count().unwrap(),
        height as usize
    );
    assert!(state.kura.wsv_checkpoint(height).unwrap().is_none());
    assert!(state.kura.commit_manifest(height).unwrap().is_none());
    assert!(state.kura.v2_finality_artifact(height).unwrap().is_none());
    let files = checkpoint_tree(&state.kura.store_root());
    let captured = CapturedStateSnapshot::capture(&state).unwrap();
    let before = captured.canonical_hash().unwrap();
    assert_capture_boundary_refusal(
        captured
            .canonical_hash_for_block(network, height, hash)
            .unwrap_err(),
        "height",
    );
    assert_eq!(checkpoint_tree(&state.kura.store_root()), files);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        before
    );
    let published = fixture
        .publish()
        .unwrap_or_else(|e| panic!("retry original Native execution: {e:?}"));
    assert_eq!(state.committed_height(), height as usize);
    assert_eq!(
        state
            .kura
            .wsv_checkpoint(height)
            .unwrap()
            .unwrap()
            .state_hash(),
        fixture.checkpoint
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        fixture.checkpoint
    );
    drop(published);
}

#[test]
fn retained_publication_checkpoint_keeps_historical_capture_after_later_state() {
    let mut fixture = native_checkpoint_fixture();
    let state = Arc::clone(&fixture.state);
    let network = *state.network_id_ref();
    let parent_height = state.committed_height() as u64;
    let parent_hash = state.latest_block_hash_fast().unwrap();
    let captured = CapturedStateSnapshot::capture(&state).unwrap();
    let parent_checkpoint = captured
        .canonical_hash_for_block(network, parent_height, parent_hash)
        .unwrap();
    let published = fixture
        .publish()
        .unwrap_or_else(|e| panic!("publish actual Native successor: {e:?}"));
    assert_eq!(state.committed_height() as u64, parent_height + 1);
    assert_eq!(
        state.latest_block_hash_fast(),
        Some(fixture.finality.artifact().block_hash)
    );
    let files = checkpoint_tree(&state.kura.store_root());
    let height = fixture.finality.artifact().height;
    let checkpoint = state.kura.wsv_checkpoint(height).unwrap().unwrap();
    let manifest = state.kura.commit_manifest(height).unwrap().unwrap();
    let later = CapturedStateSnapshot::capture(&state).unwrap();
    assert_capture_boundary_refusal(
        later
            .canonical_hash_for_block(network, parent_height, parent_hash)
            .unwrap_err(),
        "height",
    );
    assert_eq!(checkpoint_tree(&state.kura.store_root()), files);
    assert_eq!(
        state.kura.wsv_checkpoint(height).unwrap().unwrap(),
        checkpoint
    );
    assert_eq!(
        state.kura.commit_manifest(height).unwrap().unwrap(),
        manifest
    );
    assert_eq!(
        captured
            .canonical_hash_for_block(network, parent_height, parent_hash)
            .unwrap(),
        parent_checkpoint
    );
    assert_eq!(checkpoint.state_hash(), fixture.checkpoint);
    drop(published);
}

#[test]
fn retained_publication_checkpoint_rejects_wrong_tip_and_network_without_storage_mutation() {
    let mut fixture = native_checkpoint_fixture();
    let state = Arc::clone(&fixture.state);
    let published = fixture
        .publish()
        .unwrap_or_else(|e| panic!("publish actual Native carrier: {e:?}"));
    let artifact = fixture.finality.artifact();
    let files = checkpoint_tree(&state.kura.store_root());
    let checkpoint = state.kura.wsv_checkpoint(artifact.height).unwrap().unwrap();
    let manifest = state
        .kura
        .commit_manifest(artifact.height)
        .unwrap()
        .unwrap();
    let captured = CapturedStateSnapshot::capture(&state).unwrap();
    let before = captured.canonical_hash().unwrap();
    for component in ["block hash", "network"] {
        let mut network = artifact.height_context.network_id;
        let mut hash = artifact.block_hash;
        if component == "block hash" {
            hash = iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"another State tip"));
            assert_ne!(hash, artifact.block_hash);
        } else {
            network = iroha_data_model::NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(
                    b"another snapshot network",
                )),
            );
            assert_ne!(network, artifact.height_context.network_id);
        }
        assert_capture_boundary_refusal(
            captured
                .canonical_hash_for_block(network, artifact.height, hash)
                .unwrap_err(),
            component,
        );
        assert_eq!(checkpoint_tree(&state.kura.store_root()), files);
        assert_eq!(
            state.kura.wsv_checkpoint(artifact.height).unwrap().unwrap(),
            checkpoint
        );
        assert_eq!(
            state
                .kura
                .commit_manifest(artifact.height)
                .unwrap()
                .unwrap(),
            manifest
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
    }
    drop(published);
}

#[test]
fn retained_publication_checkpoint_exact_owner_retry_is_idempotent() {
    let mut fixture = native_checkpoint_fixture();
    let state = Arc::clone(&fixture.state);
    let height = fixture.finality.artifact().height;
    let original_height = state.committed_height();
    let allocations = phase_allocations(fixture.owner.as_ref().unwrap());
    let held = state.state_commit_lock.lock();
    assert!(matches!(
        fixture.publish(),
        Err(LocalValidationRefusal::PhysicalBusy(_))
    ));
    assert!(matches!(
        fixture.owner.as_ref().unwrap(),
        RetainedCarrier::Checkpointed(_)
    ));
    let files = checkpoint_tree(&state.kura.store_root());
    let checkpoint = state.kura.wsv_checkpoint(height).unwrap().unwrap();
    let manifest = state.kura.commit_manifest(height).unwrap().unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    for _ in 0..3 {
        assert!(matches!(
            fixture.publish(),
            Err(LocalValidationRefusal::PhysicalBusy(_))
        ));
        assert_eq!(
            phase_allocations(fixture.owner.as_ref().unwrap()),
            allocations
        );
        assert_eq!(checkpoint_tree(&state.kura.store_root()), files);
        assert_eq!(
            state.kura.wsv_checkpoint(height).unwrap().unwrap(),
            checkpoint
        );
        assert_eq!(
            state.kura.commit_manifest(height).unwrap().unwrap(),
            manifest
        );
        assert_eq!(checkpoint.state_hash(), fixture.checkpoint);
        assert!(
            state
                .kura
                .commit_manifest_has_wsv_binding(&manifest)
                .unwrap()
        );
        assert_eq!(state.committed_height(), original_height);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
            before
        );
    }
    drop(held);
    let published = fixture
        .publish()
        .unwrap_or_else(|e| panic!("publish original retained owner: {e:?}"));
    assert_eq!(state.committed_height(), height as usize);
    assert_eq!(
        state.latest_block_hash_fast(),
        Some(fixture.finality.artifact().block_hash)
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state).unwrap(),
        fixture.checkpoint
    );
    drop(published);
}
