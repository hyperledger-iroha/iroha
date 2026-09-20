//! Original ordinary execution proves durable witness publication and exact retry.

use super::*;
use crate::state::{State, carrier_preparation::tests::prepare};
use iroha_data_model::{account::Account, isi::Register};
use std::{
    collections::BTreeMap,
    convert::Infallible,
    fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::SystemTime,
};

struct Reservation(Arc<AtomicUsize>);
impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

type Decision = DecisionBoundCarrierJournals<
    Reservation,
    Reservation,
    DetachedCarrierComponents,
    KuraWsvCheckpointReceipt,
>;

struct Fixture {
    decision: Decision,
    state: Box<State>,
    released: Arc<AtomicUsize>,
    state_hash: iroha_crypto::Hash,
    generation: u64,
}

#[inline(never)]
fn witness_fixture(foreign: bool) -> Box<Fixture> {
    let instructions = if foreign {
        vec![Register::account(Account::new(iroha_test_samples::ALICE_ID.clone())).into()]
    } else {
        Vec::new()
    };
    let (state, proposal, topology, context) =
        crate::state::carrier_preparation::tests::fixture_with_instructions(&instructions);
    let state_hash = crate::snapshot::canonical_state_snapshot_hash(&state).unwrap();
    let generation = state.state_view_generation();
    let released = Arc::new(AtomicUsize::new(0));
    let journals = prepare(&state, proposal, &topology, &context)
        .unwrap_or_else(|(_, error)| panic!("actual original execution: {error}"))
        .prepare_journals(None, None, |_| {
            Ok::<_, Infallible>(Reservation(Arc::clone(&released)))
        })
        .unwrap();
    let finality = super::super::tests::signed_finality(
        context,
        super::super::tests::subject(journals.valid.as_ref()),
        journals.execution_prefix,
        0,
    );
    let decision = journals
        .bind_decision(finality, |_| {
            Ok::<_, Infallible>(Reservation(Arc::clone(&released)))
        })
        .unwrap_or_else(|refusal| panic!("actual original decision: {:?}", refusal.error));
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
        released,
        state_hash,
        generation,
    })
}

impl Fixture {
    fn assert_unpublished(&self) {
        assert_eq!(self.state.committed_height(), 0);
        assert_eq!(self.state.state_view_generation(), self.generation);
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_hash(&self.state).unwrap(),
            self.state_hash
        );
        assert_eq!(self.released.load(Ordering::SeqCst), 0);
        assert!(self.state.state_commit_lock.try_lock_or_wait().is_ok());
        assert!(self.state.state_write_lock.try_lock_or_wait().is_ok());
        drop(self.state.kura.try_publication_lease().unwrap());
    }

    fn proof_path(&self, staged: bool) -> PathBuf {
        // These are the fixed canonical namespace and exact height encoding
        // owned by Kura, used only to inject local persistence corruption.
        self.state
            .kura
            .store_root()
            .join("blocks/canonical")
            .join(if staged {
                "kagemusha_v1_finality_staging"
            } else {
                "kagemusha_v1_finality"
            })
            .join(format!("{:020}.norito", self.decision.finality().height))
    }

    fn stage_original(&self) {
        self.state
            .kura
            .stage_kagemusha_finality_sidecar(
                self.decision.finality().height,
                self.decision.finality().block_hash,
                self.decision.journals.source_prefix.witness(),
                self.decision.journals.execution_prefix,
                self.decision
                    .journals
                    .source_prefix
                    .parliament_timed_ovn_casting_bindings()
                    .unwrap_or(&[]),
            )
            .unwrap();
    }

    fn check_final(&self) -> Result<(), crate::kura::Error> {
        let lease = self.state.kura.try_publication_lease().unwrap();
        lease.reauthenticate_checkpoint(
            &self.decision.checkpoint,
            self.decision.finality(),
            self.decision.journals.checkpoint,
        )?;
        lease.reauthenticate_execution_witness(self.decision.finality())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FileImage {
    bytes: Vec<u8>,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64),
}

fn image(path: &Path) -> FileImage {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file());
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    FileImage {
        bytes: fs::read(path).unwrap(),
        modified: metadata.modified().unwrap(),
        #[cfg(unix)]
        identity: (metadata.dev(), metadata.ino()),
    }
}

fn tree(root: &Path) -> BTreeMap<PathBuf, FileImage> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, FileImage>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(path.strip_prefix(root).unwrap().to_owned(), image(&path));
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

struct Original {
    writes: *const (),
    sources: *const (),
    hashes: *const (),
    wire: Vec<u8>,
}

impl Original {
    fn capture(decision: &Decision) -> Self {
        Self {
            writes: decision
                .journals
                .source_prefix
                .witness()
                .writes
                .as_ptr()
                .cast(),
            sources: decision
                .journals
                .source_prefix
                .sources()
                .entries()
                .as_ptr()
                .cast(),
            hashes: decision
                .journals
                .components
                .block_hashes
                .get(0)
                .map_or(std::ptr::null(), std::ptr::from_ref)
                .cast(),
            wire: decision.block().encode_wire().unwrap(),
        }
    }

    fn assert_retained(&self, decision: &Decision) {
        let actual = Self::capture(decision);
        assert_eq!(self.writes, actual.writes);
        assert_eq!(self.sources, actual.sources);
        assert_eq!(self.hashes, actual.hashes);
        assert_eq!(self.wire, actual.wire);
    }
}

#[test]
fn original_witness_promotes_staged_only_proof_and_exact_retry_keeps_final_object() {
    let mut fixture = witness_fixture(false);
    let original = Original::capture(&fixture.decision);
    assert!(
        fixture.check_final().is_err(),
        "missing proof is not publication permission"
    );
    fixture.stage_original();
    assert!(fixture.proof_path(true).is_file());
    assert!(
        fixture.check_final().is_err(),
        "staged-only proof is not final durability"
    );
    fixture.decision.publish_execution_witness().unwrap();
    fixture.check_final().unwrap();
    assert!(!fixture.proof_path(true).exists());
    let final_file = image(&fixture.proof_path(false));
    let before_retry = tree(&fixture.state.kura.store_root());
    fixture.decision.publish_execution_witness().unwrap();
    fixture.check_final().unwrap();
    assert_eq!(image(&fixture.proof_path(false)), final_file);
    assert_eq!(tree(&fixture.state.kura.store_root()), before_retry);
    let (finality, contexts) = fixture
        .state
        .kura
        .lane_consensus_contexts_finality(fixture.decision.finality().height)
        .unwrap()
        .unwrap();
    assert_eq!(&finality, fixture.decision.finality());
    assert!(contexts.verify(
        finality.height_context.network_id,
        finality.height,
        finality.commit_qc.execution_commitment.ordinary_writes_root
    ));
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
    let released = Arc::clone(&fixture.released);
    drop(fixture);
    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn tampered_or_foreign_final_proof_refuses_without_state_or_custody_changes() {
    let mut fixture = witness_fixture(false);
    let mut foreign = witness_fixture(true);
    fixture.decision.publish_execution_witness().unwrap();
    foreign.decision.publish_execution_witness().unwrap();
    let original = Original::capture(&fixture.decision);
    let final_path = fixture.proof_path(false);
    let original_bytes = fs::read(&final_path).unwrap();
    let foreign_bytes = fs::read(foreign.proof_path(false)).unwrap();
    assert_ne!(original_bytes, foreign_bytes);
    for bytes in [b"damaged canonical final witness".to_vec(), foreign_bytes] {
        // Preserve a complete real interrupted stage. On failure, neither it
        // nor the foreign/corrupt final object may be silently cleaned up.
        fixture.stage_original();
        fs::write(&final_path, &bytes).unwrap();
        let before = tree(&fixture.state.kura.store_root());
        assert!(fixture.check_final().is_err());
        assert!(matches!(
            fixture.decision.publish_execution_witness(),
            Err(CarrierExecutionWitnessPublicationError::Witness(_))
        ));
        assert_eq!(tree(&fixture.state.kura.store_root()), before);
        original.assert_retained(&fixture.decision);
        fixture.assert_unpublished();
        fs::write(&final_path, &original_bytes).unwrap();
        fixture.decision.publish_execution_witness().unwrap();
        fixture.check_final().unwrap();
        assert!(!fixture.proof_path(true).exists());
        original.assert_retained(&fixture.decision);
        fixture.assert_unpublished();
    }
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn witness_kura_busy_waits_for_original_release_before_any_persistence() {
    let mut fixture = witness_fixture(false);
    let original = Original::capture(&fixture.decision);
    let before = tree(&fixture.state.kura.store_root());
    let held = fixture.state.kura.canonical_publication_lease();
    let wait = match fixture.decision.publish_execution_witness() {
        Err(CarrierExecutionWitnessPublicationError::Kura(
            KuraPublicationPreparationError::Busy { wait, .. },
        )) => wait,
        result => panic!("actual owner must refuse before persistence: {result:?}"),
    };
    let mut wait = wait.wait_for_release();
    let wakes = Arc::new(WakeCount::default());
    let waker = Waker::from(Arc::clone(&wakes));
    let mut context = Context::from_waker(&waker);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Pending);
    assert_eq!(tree(&fixture.state.kura.store_root()), before);
    original.assert_retained(&fixture.decision);
    drop(held);
    assert_eq!(wakes.0.load(Ordering::SeqCst), 1);
    assert_eq!(Pin::new(&mut wait).poll(&mut context), Poll::Ready(()));
    fixture.decision.publish_execution_witness().unwrap();
    fixture.check_final().unwrap();
    fixture.assert_unpublished();
}

#[test]
fn foreign_checkpoint_refuses_before_materializing_original_witness() {
    let mut fixture = witness_fixture(false);
    let mut foreign = witness_fixture(true);
    let original = Original::capture(&fixture.decision);
    let before = tree(&fixture.state.kura.store_root());
    std::mem::swap(
        &mut fixture.decision.checkpoint,
        &mut foreign.decision.checkpoint,
    );
    assert!(matches!(
        fixture.decision.publish_execution_witness(),
        Err(CarrierExecutionWitnessPublicationError::Checkpoint(_))
    ));
    assert_eq!(tree(&fixture.state.kura.store_root()), before);
    original.assert_retained(&fixture.decision);
    fixture.assert_unpublished();
    std::mem::swap(
        &mut fixture.decision.checkpoint,
        &mut foreign.decision.checkpoint,
    );
    fixture.decision.publish_execution_witness().unwrap();
    fixture.check_final().unwrap();
    fixture.assert_unpublished();
}
