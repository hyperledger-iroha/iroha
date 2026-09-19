//! Real finalized archive continuations retain partial success and original custody.

use super::*;
use crate::{
    query::{
        provider_ingest_finalized::{
            ProviderIngestFinalizedArchiveBoundsV1, ProviderIngestFinalizedArchiveV1,
        },
        reputation_finalized::{
            ReputationFinalizedArchive, ReputationFinalizedArchiveBounds,
            ReputationFinalizedArchiveKeyV1,
        },
    },
    state::{State, carrier_preparation::tests::prepare},
};
use iroha_crypto::Hash;
use std::{
    collections::BTreeMap,
    convert::Infallible,
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc, Weak,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::SystemTime,
};

#[test]
fn archive_publication_diagnostics_identify_failed_archive_and_cause() {
    for (error, archive) in [
        (
            CarrierArchivePublicationError::Provider(
                ProviderIngestFinalizedArchiveErrorV1::InvalidKey {
                    reason: "invalid carrier height",
                },
            ),
            "Provider",
        ),
        (
            CarrierArchivePublicationError::Reputation(
                ReputationFinalizedArchiveError::InvalidKey {
                    reason: "invalid carrier height",
                },
            ),
            "Reputation",
        ),
    ] {
        let diagnostic = format!("{error:?}");
        assert!(diagnostic.contains(archive));
        assert!(diagnostic.contains("invalid carrier height"));
    }
}

struct Reservation {
    released: Arc<AtomicUsize>,
    provider: Weak<ProviderIngestFinalizedArchiveV1>,
    reputation: Weak<ReputationFinalizedArchive>,
}

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        // The fixture retains one external Arc each. Both original capture
        // owners must already have dropped before either resource guard does.
        assert_eq!(self.provider.strong_count(), 1);
        assert_eq!(self.reputation.strong_count(), 1);
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

type Decision = DecisionBoundCarrierJournals<
    Reservation,
    Reservation,
    DetachedCarrierComponents,
    KuraWsvCheckpointReceipt,
>;

struct Fixture {
    // Drop captured values before their external archive handles and directory.
    decision: Decision,
    state: Box<State>,
    provider: Arc<ProviderIngestFinalizedArchiveV1>,
    reputation: Arc<ReputationFinalizedArchive>,
    directory: tempfile::TempDir,
    state_hash: Hash,
    state_generation: u64,
    capture_released: Arc<AtomicUsize>,
    binding_released: Arc<AtomicUsize>,
}

fn fixture() -> Box<Fixture> {
    let (state, proposal, topology, context) = super::super::super::tests::archive_fixture();
    let state_hash = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let state_generation = state.state_view_generation();
    let directory = tempfile::tempdir().unwrap();
    let directory_path = directory.path().canonicalize().unwrap();
    let provider = Arc::new(
        ProviderIngestFinalizedArchiveV1::try_open(
            directory_path.join("provider"),
            ProviderIngestFinalizedArchiveBoundsV1::try_new(1 << 20, 16, 16 << 20, 16, 16, 256, 16)
                .unwrap(),
        )
        .unwrap(),
    );
    let reputation = Arc::new(
        ReputationFinalizedArchive::try_open(
            directory_path.join("reputation"),
            ReputationFinalizedArchiveBounds::try_new(1 << 20, 16, 16 << 20).unwrap(),
        )
        .unwrap(),
    );
    let capture_released = Arc::new(AtomicUsize::new(0));
    let binding_released = Arc::new(AtomicUsize::new(0));
    let reservation = |released: &Arc<AtomicUsize>| Reservation {
        released: Arc::clone(released),
        provider: Arc::downgrade(&provider),
        reputation: Arc::downgrade(&reputation),
    };
    let journals = prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("real signed policy execution: {error}"))
        .prepare_journals(Some(&provider), Some(&reputation), |_| {
            Ok::<_, Infallible>(reservation(&capture_released))
        })
        .unwrap();
    assert!(journals.provider_capture.is_some());
    assert!(journals.reputation_capture.is_some());
    let finality = super::super::tests::signed_finality(
        context,
        super::super::tests::subject(journals.valid.as_ref()),
        journals.execution_prefix,
        0,
    );
    let decision = journals
        .bind_decision(finality, |_| {
            Ok::<_, Infallible>(reservation(&binding_released))
        })
        .unwrap_or_else(|refusal| panic!("exact four-validator decision: {:?}", refusal.error));
    state.kura.store_block(decision.block().clone()).unwrap();
    let finality = state
        .kura
        .store_v2_finality_artifact(decision.finality())
        .unwrap();
    let checkpoint = state
        .kura
        .persist_wsv_checkpoint_for_v2_commit(&finality, decision.journals.checkpoint)
        .unwrap();
    Box::new(Fixture {
        decision: decision.attach_checkpoint(checkpoint),
        state,
        provider,
        reputation,
        directory,
        state_hash,
        state_generation,
        capture_released,
        binding_released,
    })
}

impl Fixture {
    fn assert_unpublished(&self) {
        assert_eq!(self.state.committed_height(), 0);
        assert_eq!(self.state.state_view_generation(), self.state_generation);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&self.state).unwrap(),
            self.state_hash
        );
        assert_eq!(self.state.kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(self.capture_released.load(Ordering::SeqCst), 0);
        assert_eq!(self.binding_released.load(Ordering::SeqCst), 0);
        drop(self.state.kura.try_publication_lease().unwrap());
    }

    fn assert_exact_archives(&self) {
        let finality = self.decision.finality();
        let provider_key = self
            .provider
            .resolve_exact_key(
                &finality.height_context.network_id,
                finality.height,
                *finality.block_hash.as_ref(),
            )
            .unwrap();
        assert_eq!(
            provider_key.finalized_at_unix_ms,
            self.decision.block().header().creation_time_ms
        );
        assert!(self.provider.record_path(&provider_key).unwrap().is_file());
        let reputation_key = ReputationFinalizedArchiveKeyV1::try_new(
            finality.height_context.network_id,
            finality.height,
            *finality.block_hash.as_ref(),
        )
        .unwrap();
        assert!(
            self.reputation
                .get_exact(&reputation_key)
                .unwrap()
                .is_some()
        );
        assert!(
            self.reputation
                .record_path(&reputation_key)
                .unwrap()
                .is_file()
        );
        assert!(!self.provider.is_empty().unwrap());
        assert!(!self.reputation.is_empty().unwrap());
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FileImage {
    bytes: Vec<u8>,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64),
}

fn tree_image(root: &Path) -> BTreeMap<PathBuf, FileImage> {
    fn visit(root: &Path, directory: &Path, image: &mut BTreeMap<PathBuf, FileImage>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            if metadata.is_dir() {
                visit(root, &path, image);
            } else {
                assert!(metadata.is_file());
                #[cfg(unix)]
                use std::os::unix::fs::MetadataExt;
                image.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    FileImage {
                        bytes: fs::read(&path).unwrap(),
                        modified: metadata.modified().unwrap(),
                        #[cfg(unix)]
                        identity: (metadata.dev(), metadata.ino()),
                    },
                );
            }
        }
    }
    let mut image = BTreeMap::new();
    visit(root, root, &mut image);
    image
}

struct OriginalCustody {
    wire: Vec<u8>,
    provider: *const (),
    reputation: *const (),
    hashes: *const (),
    membership: *const (),
}

impl OriginalCustody {
    fn capture(decision: &Decision) -> Self {
        Self {
            wire: decision.block().encode_wire().unwrap(),
            provider: std::ptr::from_ref(decision.journals.provider_capture.as_ref().unwrap())
                .cast(),
            reputation: std::ptr::from_ref(decision.journals.reputation_capture.as_ref().unwrap())
                .cast(),
            hashes: decision
                .journals
                .components
                .block_hashes
                .as_slice()
                .as_ptr()
                .cast(),
            membership: std::ptr::from_ref(
                decision
                    .journals
                    .components
                    .transactions
                    .staged_membership()
                    .1,
            )
            .cast(),
        }
    }

    fn assert_retained(&self, decision: &Decision) {
        let actual = Self::capture(decision);
        assert_eq!(self.wire, actual.wire);
        assert_eq!(self.provider, actual.provider);
        assert_eq!(self.reputation, actual.reputation);
        assert_eq!(self.hashes, actual.hashes);
        assert_eq!(self.membership, actual.membership);
    }
}

#[test]
fn original_archives_publish_and_exact_retry_preserves_files_without_state_publication() {
    let mut fixture = fixture();
    let original = OriginalCustody::capture(&fixture.decision);
    let kura = tree_image(&fixture.state.kura.store_root());
    assert!(fixture.provider.is_empty().unwrap());
    assert!(fixture.reputation.is_empty().unwrap());
    fixture.decision.publish_archives().unwrap();
    fixture.assert_exact_archives();
    let archives = tree_image(fixture.directory.path());
    let generations = (
        fixture.provider.health_generation().unwrap(),
        fixture.reputation.health_generation().unwrap(),
    );
    fixture.decision.publish_archives().unwrap();
    assert_eq!(tree_image(fixture.directory.path()), archives);
    assert_eq!(
        (
            fixture.provider.health_generation().unwrap(),
            fixture.reputation.health_generation().unwrap(),
        ),
        generations
    );
    assert_eq!(tree_image(&fixture.state.kura.store_root()), kura);
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
    let finality = fixture.decision.finality();
    let key = ReputationFinalizedArchiveKeyV1::try_new(
        finality.height_context.network_id,
        finality.height,
        *finality.block_hash.as_ref(),
    )
    .unwrap();
    let projection = fixture.reputation.get_exact(&key).unwrap().unwrap();
    let wait = match fixture.reputation.insert(projection) {
        Err(ReputationFinalizedArchiveError::CaptureReserved { wait }) => wait,
        result => panic!("completed capture must retain its original reservation: {result:?}"),
    };
    assert!(!wait.is_released());
    let capture_released = Arc::clone(&fixture.capture_released);
    let binding_released = Arc::clone(&fixture.binding_released);
    drop(fixture);
    assert!(wait.is_released());
    assert_eq!(capture_released.load(Ordering::SeqCst), 1);
    assert_eq!(binding_released.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_refusal_preserves_both_original_captures_and_never_starts_reputation() {
    let mut fixture = fixture();
    let original = OriginalCustody::capture(&fixture.decision);
    let root = fixture.directory.path().canonicalize().unwrap();
    let records = root.join("provider/records");
    let saved = root.join("original-provider-records");
    fs::rename(&records, &saved).unwrap();
    fs::create_dir(&records).unwrap();
    let before = tree_image(&root);
    assert!(matches!(
        fixture.decision.publish_archives(),
        Err(CarrierArchivePublicationError::Provider(_))
    ));
    assert_eq!(tree_image(&root), before);
    assert!(fixture.reputation.is_empty().unwrap());
    assert_eq!(fs::read_dir(&records).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&saved).unwrap().count(), 0);
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
    fs::remove_dir(&records).unwrap();
    fs::rename(&saved, &records).unwrap();
    fixture.decision.publish_archives().unwrap();
    fixture.assert_exact_archives();
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
}

#[test]
fn reputation_refusal_retains_provider_success_and_retries_original_captures() {
    let mut fixture = fixture();
    let original = OriginalCustody::capture(&fixture.decision);
    let root = fixture.directory.path().canonicalize().unwrap();
    let anchors = root.join("reputation/anchors");
    let saved = root.join("original-reputation-anchors");
    fs::rename(&anchors, &saved).unwrap();
    fs::create_dir(&anchors).unwrap();
    let reputation_before = tree_image(&root.join("reputation"));
    let kura = tree_image(&fixture.state.kura.store_root());
    assert!(matches!(
        fixture.decision.publish_archives(),
        Err(CarrierArchivePublicationError::Reputation(_))
    ));
    assert!(!fixture.provider.is_empty().unwrap());
    assert_eq!(tree_image(&root.join("reputation")), reputation_before);
    assert_eq!(fs::read_dir(&saved).unwrap().count(), 0);
    let provider_after = tree_image(&root.join("provider"));
    let provider_generation = fixture.provider.health_generation().unwrap();
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
    // Retrying while the original namespace is still substituted must repeat
    // the integrity refusal without rewriting/accounting the completed provider.
    assert!(matches!(
        fixture.decision.publish_archives(),
        Err(CarrierArchivePublicationError::Reputation(_))
    ));
    assert_eq!(tree_image(&root.join("provider")), provider_after);
    assert_eq!(
        fixture.provider.health_generation().unwrap(),
        provider_generation
    );
    fs::remove_dir(&anchors).unwrap();
    fs::rename(&saved, &anchors).unwrap();
    fixture.decision.publish_archives().unwrap();
    fixture.assert_exact_archives();
    assert_eq!(tree_image(&root.join("provider")), provider_after);
    assert_eq!(
        fixture.provider.health_generation().unwrap(),
        provider_generation
    );
    assert_eq!(tree_image(&fixture.state.kura.store_root()), kura);
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
}

#[test]
fn kura_busy_preserves_archive_custody_and_uses_actual_release_before_retry() {
    let mut fixture = fixture();
    let original = OriginalCustody::capture(&fixture.decision);
    let before = tree_image(fixture.directory.path());
    let held = fixture.state.kura.canonical_publication_lease();
    let wait = match fixture.decision.publish_archives() {
        Err(CarrierArchivePublicationError::Kura(KuraPublicationPreparationError::Busy {
            wait,
            ..
        })) => wait,
        result => panic!("actual original canonical owner must refuse before writes: {result:?}"),
    };
    let mut wait = wait.wait_for_release();
    let wakes = Arc::new(WakeCount::default());
    let waker = Waker::from(Arc::clone(&wakes));
    let mut context = Context::from_waker(&waker);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    assert_eq!(tree_image(fixture.directory.path()), before);
    assert!(fixture.provider.is_empty().unwrap());
    assert!(fixture.reputation.is_empty().unwrap());
    original.assert_retained(&fixture.decision);
    drop(held);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    fixture.decision.publish_archives().unwrap();
    fixture.assert_exact_archives();
    fixture.assert_unpublished();
}

#[test]
fn missing_or_replaced_checkpoint_refuses_before_either_archive_changes() {
    let mut fixture = fixture();
    let original = OriginalCustody::capture(&fixture.decision);
    let before = tree_image(fixture.directory.path());
    fixture
        .state
        .kura
        .remove_wsv_checkpoint_without_binding_for_tests(1)
        .unwrap();
    for replacement in [false, true] {
        if replacement {
            let receipt = fixture
                .state
                .kura
                .persist_wsv_checkpoint_for_v2_commit(
                    fixture.decision.checkpoint.finality_receipt(),
                    fixture.decision.journals.checkpoint,
                )
                .unwrap();
            fixture
                .state
                .kura
                .reauthenticate_wsv_checkpoint_receipt(
                    &receipt,
                    fixture.decision.finality(),
                    fixture.decision.journals.checkpoint,
                )
                .unwrap();
        }
        assert!(matches!(
            fixture.decision.publish_archives(),
            Err(CarrierArchivePublicationError::Checkpoint(_))
        ));
        assert_eq!(tree_image(fixture.directory.path()), before);
        assert!(fixture.provider.is_empty().unwrap());
        assert!(fixture.reputation.is_empty().unwrap());
        original.assert_retained(&fixture.decision);
        fixture.assert_unpublished();
    }
}
