//! Mandatory authenticated-body storage, original-owner retries and source-bound reads.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::sumeragi::crypto::BlsCrypto;

use super::*;
use crate::sumeragi::{body_read::BodyReadPoll, body_record::tests::fixture};

fn open(root: &Path, source: &AvailabilitySource, budget: &AllocationBudget) -> FileBodyStore {
    FileBodyStore::open(
        root,
        &source.instance(),
        Arc::new(BlsCrypto::new()),
        BodyLimits::default(),
        budget.clone(),
    )
    .unwrap()
}

fn restored(
    store: &FileBodyStore,
    source: AvailabilitySource,
    budget: &AllocationBudget,
) -> AvailableBody {
    let mut read = store.begin_read(source.clone()).unwrap();
    assert_eq!(read.source(), &source);
    let BodyReadPoll::Ready(job) = read.poll(budget).unwrap() else {
        panic!("complete record")
    };
    assert!(matches!(read.poll(budget), Err(BodyReadError::Completed)));
    job.complete(budget, &BlsCrypto::new())
        .unwrap_or_else(|_| panic!("full signed restoration"))
}

#[test]
fn durable_record_restores_through_actual_signature_and_rs16_verification() {
    let root = tempfile::tempdir().unwrap();
    let (body, source, budget) = fixture(23572);
    let store = open(root.path(), &source, &budget);
    store.put(&source.block_hash(), &body).unwrap();
    let original_bytes = fs::read(store.body_path(source.height(), &source.block_hash())).unwrap();
    assert_eq!(restored(&store, source.clone(), &budget), body);
    store.put(&source.block_hash(), &body).unwrap();
    assert_eq!(
        fs::read(store.body_path(source.height(), &source.block_hash())).unwrap(),
        original_bytes
    );
    assert_eq!(store.bytes().unwrap(), original_bytes.len() as u64);
    let reopened = open(root.path(), &source, &budget);
    assert_eq!(restored(&reopened, source.clone(), &budget), body);
    reopened.prune_through(source.height()).unwrap();
    assert_eq!(reopened.bytes().unwrap(), 0);
    assert!(matches!(
        reopened.begin_read(source).unwrap().poll(&budget).unwrap(),
        BodyReadPoll::Absent
    ));
}

#[test]
fn retirement_authority_is_local_monotonic_and_never_inferred_from_absence() {
    let root = tempfile::tempdir().unwrap();
    let (_, source, budget) = fixture(1025);
    let store = open(root.path(), &source, &budget);
    assert!(!store.retirement_authorized(source.height()));
    store.prune_through(source.height()).unwrap();
    store.prune_through(source.height() - 1).unwrap();
    assert!(store.retirement_authorized(source.height()));
    assert!(!store.retirement_authorized(0));
    assert!(!store.retirement_authorized(source.height() + 1));
    assert!(!open(root.path(), &source, &budget).retirement_authorized(source.height()));
}

#[test]
fn read_refusal_keeps_source_and_foreign_pool_is_rejected_before_allocation() {
    let root = tempfile::tempdir().unwrap();
    let (body, source, budget) = fixture(1025);
    let store = open(root.path(), &source, &budget);
    store.put(&source.block_hash(), &body).unwrap();
    let mut read = store.begin_read(source.clone()).unwrap();
    let foreign = AllocationBudget::new(budget.limit_bytes());
    assert!(matches!(
        read.poll(&foreign),
        Err(BodyReadError::ForeignBudget)
    ));
    assert_eq!(foreign.reserved_bytes(), 0);
    let reserved = budget.reserved_bytes();
    budget.set_limit_bytes(reserved);
    assert!(matches!(
        read.poll(&budget).unwrap(),
        BodyReadPoll::Pending(_)
    ));
    assert_eq!(read.source(), &source);
    assert_eq!(budget.reserved_bytes(), reserved);
    budget.set_limit_bytes(1 << 25);
    let BodyReadPoll::Ready(job) = read.poll(&budget).unwrap() else {
        panic!("same job resumes")
    };
    assert_eq!(
        job.complete(&budget, &BlsCrypto::new())
            .unwrap_or_else(|_| panic!("restoration")),
        body
    );
}

#[test]
fn corrupt_record_is_an_error_and_never_absence_or_available_custody() {
    let root = tempfile::tempdir().unwrap();
    let (body, source, budget) = fixture(1025);
    let store = open(root.path(), &source, &budget);
    store.put(&source.block_hash(), &body).unwrap();
    let path = store.body_path(source.height(), &source.block_hash());
    fs::write(&path, b"not a canonical body record").unwrap();
    let mut read = store.begin_read(source.clone()).unwrap();
    assert!(matches!(read.poll(&budget), Err(BodyReadError::Decode(_))));
    assert!(matches!(read.poll(&budget), Err(BodyReadError::Decode(_))));
    assert!(store.put(&source.block_hash(), &body).is_err());
    assert_eq!(fs::read(path).unwrap(), b"not a canonical body record");
}

#[derive(Default)]
struct FailPublishedSync(AtomicBool);
impl Faults for FailPublishedSync {
    fn before(&self, step: FsStep, path: &Path) -> io::Result<()> {
        if step == FsStep::SyncDir
            && path
                .file_name()
                .and_then(|p| p.to_str())
                .and_then(parse_height)
                .is_some()
            && self.0.load(Ordering::SeqCst)
        {
            return Err(io::Error::other("directory sync refused"));
        }
        Ok(())
    }
}

#[test]
fn publication_refusal_keeps_the_actual_original_owners_and_encoded_pointer() {
    let root = tempfile::tempdir().unwrap();
    let (body, source, budget) = fixture(23572);
    let faults = Arc::new(FailPublishedSync::default());
    let store = FileBodyStore::open_with_faults(
        root.path(),
        &source.instance(),
        Arc::new(BlsCrypto::new()),
        BodyLimits::default(),
        budget.clone(),
        faults.clone(),
    )
    .unwrap();
    faults.0.store(true, Ordering::SeqCst);
    assert!(store.put(&source.block_hash(), &body).is_err());
    let owner_pointers = |store: &FileBodyStore| {
        let mut pending = store.pending_write.lock();
        let original = pending.as_mut().unwrap();
        let payload = original.body().payload().as_slice().as_ptr();
        let frame = original.body().availability().as_slice().as_ptr();
        let encoded = original.prepare(&budget).unwrap().as_ptr();
        (payload, frame, encoded)
    };
    let pointers = owner_pointers(&store);
    assert_eq!(pointers.0, body.payload().as_slice().as_ptr());
    assert_eq!(pointers.1, body.availability().as_slice().as_ptr());
    let reserved = budget.reserved_bytes();
    assert!(store.put(&source.block_hash(), &body).is_err());
    assert_eq!(owner_pointers(&store), pointers);
    assert_eq!(budget.reserved_bytes(), reserved);
    faults.0.store(false, Ordering::SeqCst);
    store.put(&source.block_hash(), &body).unwrap();
    assert!(store.pending_write.lock().is_none());
    assert_eq!(restored(&store, source, &budget), body);
}

#[test]
fn store_requires_the_independent_original_pool_and_source_instance() {
    let root = tempfile::tempdir().unwrap();
    let (body, source, budget) = fixture(1025);
    let foreign = AllocationBudget::new(budget.limit_bytes());
    let store = open(root.path(), &source, &foreign);
    assert!(store.put(&source.block_hash(), &body).is_err());
    assert!(store.pending_write.lock().is_none());
    assert_eq!(store.bytes().unwrap(), 0);
    let wrong_source = AvailabilitySource::new(
        Hash32([99; 32]),
        source.height(),
        source.block_hash(),
        source.config().clone(),
    )
    .unwrap();
    assert!(store.begin_read(wrong_source).is_err());
}
