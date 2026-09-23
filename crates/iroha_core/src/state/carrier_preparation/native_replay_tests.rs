//! Historical replay of genuinely published independent and atomic Native carriers.

use super::phase_queue;
use crate::{
    block::VerifiedV2FinalityArtifact,
    kura::CommitManifest,
    snapshot::canonical_state_snapshot_hash,
    state::{
        RetainedCarrier, State, StateReadOnlyWithTransactions, TransactionsReadOnly, WorldReadOnly,
        tests::native_publication_replay_fixture,
    },
    sumeragi::v2_apply::V2ApplyService,
};
use iroha_crypto::Hash;
use iroha_data_model::{asset::AssetId, block::SignedBlock};
use mv::storage::StorageReadOnly;
use std::{
    collections::BTreeMap,
    convert::Infallible,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    task::Waker,
};

struct PublishedNativeReplayFixture {
    state: Arc<State>,
    replay: Box<State>,
    block: SignedBlock,
    finality: VerifiedV2FinalityArtifact,
    checkpoint: Hash,
    source: AssetId,
    destination: AssetId,
}

/// End genesis construction and release every preparation writer before replay.
#[inline(never)]
fn published_native_replay_fixture(atomic: bool) -> Box<PublishedNativeReplayFixture> {
    let fixture = native_publication_replay_fixture(atomic);
    let source = fixture.assets().0.clone();
    let destination = fixture.assets().1.clone();
    let replay = crate::state::isolated_state_for_replay_prevalidation(
        fixture.state(),
        &fixture.state().kura,
    )
    .expect("copy the exact applying pre-State before Native execution");
    assert_eq!(
        canonical_state_snapshot_hash(&replay).unwrap(),
        canonical_state_snapshot_hash(fixture.state()).unwrap(),
    );
    let prepared = fixture.prepare();
    let block = prepared.block().clone();
    let batch = block
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_ref()
        .unwrap();
    assert_eq!(batch.groups.len(), 1);
    assert_eq!(batch.groups[0].decisions.len(), if atomic { 2 } else { 1 });
    let finality = fixture.finality(&block, prepared.execution_prefix_commitment());
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, Infallible>(()))
        .expect("capture the actual executed Native owner");
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
    let published = RetainedCarrier::Validated(journals)
        .try_publish(
            &state,
            &service.carrier_queue_source(),
            finality.clone(),
            Waker::noop().clone(),
        )
        .unwrap_or_else(|(_, refusal)| panic!("publish original Native carrier: {refusal:?}"));
    assert!(published.native_apply().is_some());
    assert_eq!(
        published.block().encode_wire().unwrap(),
        block.encode_wire().unwrap()
    );
    assert_eq!(canonical_state_snapshot_hash(&state).unwrap(), checkpoint);
    drop(published);
    drop(service);
    Box::new(PublishedNativeReplayFixture {
        state,
        replay,
        block,
        finality,
        checkpoint,
        source,
        destination,
    })
}

/// Include every durable byte and directory, so read-only rejection cannot hide I/O.
fn replay_tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(root: &Path, path: &Path, output: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                assert!(output.insert(relative, None).is_none());
                visit(root, &path, output);
            } else {
                assert!(kind.is_file(), "fixture contains no links or special files");
                assert!(
                    output
                        .insert(relative, Some(std::fs::read(path).unwrap()))
                        .is_none()
                );
            }
        }
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn assert_native_replay_economics(fixture: &PublishedNativeReplayFixture) {
    let actual = fixture.replay.view();
    let live = fixture.state.view();
    for asset in [&fixture.source, &fixture.destination] {
        assert_eq!(
            actual.world().assets().get(asset),
            live.world().assets().get(asset)
        );
    }
    let height = usize::try_from(fixture.block.header().height().get()).unwrap();
    for input in fixture.block.network_input_hashes() {
        assert_eq!(
            actual.transactions().get(&input).map(NonZeroUsize::get),
            Some(height)
        );
    }
    assert_eq!(fixture.replay.committed_height(), height);
    assert_eq!(
        canonical_state_snapshot_hash(&fixture.replay).unwrap(),
        fixture.checkpoint,
        "replay includes the same Native effects, admission controls and beacon as publication",
    );
}

fn exercise_native_replay(atomic: bool, mismatch_checkpoint: bool) {
    let mut fixture = published_native_replay_fixture(atomic);
    let kura = Arc::clone(&fixture.state.kura);
    let height = usize::try_from(fixture.block.header().height().get()).unwrap();
    let pre_state = canonical_state_snapshot_hash(&fixture.replay).unwrap();
    let pre_height = fixture.replay.committed_height();
    let pre_generation = fixture.replay.state_view_generation();
    assert_eq!(pre_height + 1, height);
    assert_ne!(pre_state, fixture.checkpoint);
    assert!(fixture.replay.pending_replay_publication.is_none());
    if mismatch_checkpoint {
        // Keep finality and all executed bytes genuine. Correlating the manifest
        // and checkpoint makes this reach the final post-execution State check.
        let forged = Hash::new(b"Native replay late checkpoint mismatch");
        assert_ne!(forged, fixture.checkpoint);
        let manifest = CommitManifest::new(
            u64::try_from(height).unwrap(),
            fixture.block.hash(),
            None,
            None,
            forged,
            None,
        )
        .with_authenticated_v2_commit_authority(fixture.finality.artifact());
        kura.overwrite_commit_manifest_without_binding_for_tests(&manifest)
            .expect("install the correlated late-checkpoint fixture");
        kura.overwrite_wsv_checkpoint_without_validation_for_tests(
            u64::try_from(height).unwrap(),
            forged,
            Some(&manifest),
        )
        .expect("bind the checkpoint fixture without changing valid Native execution");
        assert!(kura.commit_manifest_has_wsv_binding(&manifest).unwrap());
    }
    let files_before = replay_tree(&kura.store_root());
    let result =
        crate::state::replay_blocks_from_kura_range(&kura, fixture.replay.as_mut(), height, height);
    if mismatch_checkpoint {
        let error = result.expect_err("the late checkpoint must reject the whole Native replay");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains(&format!("block #{height} WSV checkpoint mismatch")),
            "must execute and authenticate Native outputs before the failing checkpoint: {diagnostic}",
        );
        assert_eq!(
            canonical_state_snapshot_hash(&fixture.replay).unwrap(),
            pre_state
        );
        assert_eq!(fixture.replay.committed_height(), pre_height);
        assert_eq!(fixture.replay.state_view_generation(), pre_generation);
        assert!(fixture.replay.pending_replay_publication.is_none());
        assert_eq!(replay_tree(&kura.store_root()), files_before);
    } else {
        result.unwrap_or_else(|error| panic!("replay genuine Native publication: {error:?}"));
        assert_native_replay_economics(&fixture);
        assert_eq!(
            kura.get_block(NonZeroUsize::new(height).unwrap())
                .unwrap()
                .encode_wire()
                .unwrap(),
            fixture.block.encode_wire().unwrap(),
        );
        assert_eq!(
            kura.v2_finality_artifact(u64::try_from(height).unwrap())
                .unwrap()
                .as_ref(),
            Some(fixture.finality.artifact()),
        );
        assert!(fixture.replay.pending_replay_publication.is_none());
    }
    assert_eq!(
        canonical_state_snapshot_hash(&fixture.state).unwrap(),
        fixture.checkpoint
    );
}

fn run_native_replay(name: &'static str, atomic: bool, mismatch_checkpoint: bool) {
    let worker = crate::sumeragi::sumeragi_thread_builder(name)
        .spawn(move || exercise_native_replay(atomic, mismatch_checkpoint))
        .expect("spawn Native replay on the consensus stack");
    if let Err(error) = worker.join() {
        std::panic::resume_unwind(error);
    }
}

#[test]
fn retained_native_independent_publication_replays_exact_pre_state() {
    run_native_replay("native-independent-replay", false, false);
}

#[test]
fn retained_native_atomic_publication_replays_exact_pre_state() {
    run_native_replay("native-atomic-replay", true, false);
}

#[test]
fn retained_native_independent_late_checkpoint_rejection_is_read_only() {
    run_native_replay("native-independent-checkpoint-rejection", false, true);
}

#[test]
fn retained_native_atomic_late_checkpoint_rejection_is_read_only() {
    run_native_replay("native-atomic-checkpoint-rejection", true, true);
}
