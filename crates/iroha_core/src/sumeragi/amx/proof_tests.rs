//! Persisted AMX proofs from actual native execution, finality, pruning and certified replay.

use std::{fs, path::PathBuf};

use iroha_data_model::{
    isi::sumeragi_amx::RegisterAmxDataspaceV1,
    sumeragi_amx::{
        AllocatedAmxRecordProofV1, AmxForeignInstanceV1, AmxLegV1, AmxRecordKind, AmxRecordV1,
        AmxTransactionV1,
    },
    sumeragi_finality::{genesis_epoch, test_fixtures::NativeFinalityFixture},
};
use iroha_model_base::topology::DataSpaceId;

use super::{BeginAmxV1, amx_record_proof};
use crate::{
    state::{NativeExecutionProjectionV1, World, WorldReadOnly},
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};

fn config() -> TestChainConfig {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_instructions = [21, 22]
        .into_iter()
        .map(|id| {
            let chain = NativeFinalityFixture::start(&format!("archived-amx-participant-{id}"));
            RegisterAmxDataspaceV1 {
                dataspace: DataSpaceId::new(id),
                instance: chain.verifier().instance().0,
                anchor: norito::encode_canonical(&genesis_epoch(chain.genesis()).unwrap()).unwrap(),
            }
            .into()
        })
        .collect();
    config
}

fn chain() -> (CertifiedTestChain, [u8; 32]) {
    let config = config();
    let authority = config.genesis_key.clone();
    let mut chain = CertifiedTestChain::start(config).unwrap();
    let transaction = AmxTransactionV1 {
        legs: [21, 22]
            .into_iter()
            .map(|id| AmxLegV1 {
                dataspace: DataSpaceId::new(id),
                payload: vec![7],
            })
            .collect(),
        deadline: 3,
        nonce: [7; 32],
    };
    let tx = transaction.id().unwrap();
    let signed = chain.sign(&authority, [BeginAmxV1 { transaction }.into()], 1_999);
    assert_eq!(chain.commit_at(2_000, vec![signed]), vec![true]);
    (chain, tx)
}

// Acquire one fresh original State cut for this actual synchronous read. Keeping this
// lifecycle in a fixture helper prevents repeated inline StateView temporaries from sharing
// the caller's large unoptimized stack frame. No reader, pool or certificate is substituted.
fn read_proof(
    chain: &CertifiedTestChain,
    height: u64,
    kind: AmxRecordKind,
    tx: [u8; 32],
) -> Result<
    Option<AllocatedAmxRecordProofV1>,
    crate::query::native_receipts::NativeAmxRecordProofErrorV1,
> {
    let view = chain.state().view();
    amx_record_proof(&view, height, kind, tx).complete()
}

fn path(chain: &CertifiedTestChain) -> PathBuf {
    let paths = fs::read_dir(chain.kura().store_root().join("native-contexts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("00000000000000000002-")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    paths.into_iter().next().unwrap()
}

#[test]
fn persisted_amx_records_survive_deadline_pruning_and_certified_replay() {
    let (mut chain, tx) = chain();
    let tracker =
        AmxForeignInstanceV1::new(chain.instance().0, genesis_epoch(chain.genesis()).unwrap())
            .unwrap();
    let begin = read_proof(&chain, 2, AmxRecordKind::Begin, tx)
        .unwrap()
        .unwrap();
    assert!(matches!(begin.canonical().record, AmxRecordV1::Begin(_)));
    assert_eq!(tracker.verify_record(begin.canonical()).unwrap().height, 2);
    assert!(
        read_proof(&chain, 2, AmxRecordKind::Decision, tx)
            .unwrap()
            .is_none()
    );
    assert!(
        read_proof(&chain, 2, AmxRecordKind::Begin, [0; 32])
            .unwrap()
            .is_none()
    );
    assert!(read_proof(&chain, 1, AmxRecordKind::Begin, tx).is_err());
    assert!(read_proof(&chain, 3, AmxRecordKind::Begin, tx).is_err());
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    assert!(
        chain
            .state()
            .view()
            .world()
            .sumeragi_amx()
            .transaction(&tx)
            .is_none()
    );
    let decision = read_proof(&chain, 4, AmxRecordKind::Decision, tx)
        .unwrap()
        .unwrap();
    assert!(matches!(
        decision.canonical().record,
        AmxRecordV1::Decision(_)
    ));
    tracker.verify_record(decision.canonical()).unwrap();
    assert_eq!(
        read_proof(&chain, 2, AmxRecordKind::Begin, tx)
            .unwrap()
            .as_ref()
            .map(|proof| proof.canonical()),
        Some(begin.canonical())
    );
    let mut restored = CertifiedTestChain::start(config()).unwrap();
    restored.replay_from(&chain).unwrap();
    for (height, kind, expected) in [
        (2, AmxRecordKind::Begin, begin),
        (4, AmxRecordKind::Decision, decision),
    ] {
        let proof = read_proof(&restored, height, kind, tx).unwrap().unwrap();
        assert_eq!(proof, expected);
        tracker.verify_record(proof.canonical()).unwrap();
    }
}

#[test]
fn persisted_amx_proof_rejects_missing_corrupt_and_substituted_archives() {
    let (chain, tx) = chain();
    let path = path(&chain);
    let original = fs::read(&path).unwrap();
    let projection: NativeExecutionProjectionV1 = norito::decode_canonical(&original).unwrap();
    for change in 0..6 {
        let mut changed = projection.clone();
        match change {
            0 => changed.carrier_height += 1,
            1 => {
                changed.carrier_hash = iroha_crypto::HashOf::from_untyped_unchecked(
                    iroha_crypto::Hash::new(b"foreign AMX carrier"),
                )
            }
            2 => changed.lanes.incarnations += 1,
            3 => changed
                .ordinary_writes
                .retain(|write| write.key.first() != Some(&0xD9)),
            4 => changed
                .ordinary_writes
                .retain(|write| write.key.first() == Some(&0xD9)),
            _ => changed
                .ordinary_writes
                .push(iroha_data_model::block::consensus::ExecKv {
                    key: b"unexecuted".to_vec(),
                    value: vec![1],
                }),
        }
        fs::write(&path, norito::encode_canonical(&changed).unwrap()).unwrap();
        assert!(
            amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
                .complete()
                .is_err(),
            "canonical archive mutation {change} must fail authentication"
        );
    }
    fs::write(&path, b"corrupt original archive").unwrap();
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .complete()
            .is_err()
    );
    fs::remove_file(&path).unwrap();
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .complete()
            .is_err()
    );
    fs::write(&path, original).unwrap();
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .complete()
            .unwrap()
            .is_some()
    );
}

#[test]
fn persisted_amx_proof_refuses_original_pool_exhaustion_and_unverified_certificates() {
    let (chain, tx) = chain();
    let budget = chain.state().ivm_execution_budget();
    let limit = budget.limit_bytes();
    let reserved = budget.reserved_bytes();
    budget.set_limit_bytes(reserved);
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .complete()
            .is_err()
    );
    assert_eq!(budget.reserved_bytes(), reserved);
    budget.set_limit_bytes(limit);
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .complete()
            .unwrap()
            .is_some()
    );
    assert_eq!(budget.reserved_bytes(), reserved);
    chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
    assert!(
        amx_record_proof(&chain.state().view(), 2, AmxRecordKind::Begin, tx)
            .complete()
            .is_err()
    );
}

#[test]
fn persisted_amx_original_read_retains_acquired_inode_and_exact_pool_through_decode_refusal() {
    use crate::query::native_receipts::{NativeAmxRecordProofErrorV1, NativeAmxRecordProofPollV1};
    use iroha_allocation::AllocationRefusal;
    let (chain, tx) = chain();
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let archive = path(&chain);
    let original_file = fs::read(&archive).unwrap();
    let chunks = original_file.len().div_ceil(4096);
    assert!(chunks > 0);
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    for _ in 0..chunks {
        assert!(matches!(
            read.poll().unwrap(),
            NativeAmxRecordProofPollV1::Pending
        ));
    }
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let baseline = budget.reserved_bytes();
    let pressure = budget
        .try_reserve_bytes(budget.limit_bytes() - baseline)
        .unwrap();
    let cause = read.complete().unwrap_err();
    let NativeAmxRecordProofErrorV1::Admission(AllocationRefusal::Capacity {
        requested_bytes,
        reserved_bytes,
        limit_bytes,
        release,
    }) = cause
    else {
        panic!("actual decoded original write graph capacity refusal: {cause:?}");
    };
    assert_eq!(reserved_bytes, limit_bytes);
    let expected = budget.try_reserve_bytes(requested_bytes).unwrap_err();
    assert_eq!(
        expected,
        AllocationRefusal::Capacity {
            requested_bytes,
            reserved_bytes,
            limit_bytes,
            release: release.clone()
        }
    );
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    // The read owns the original descriptor and complete prefix, not this now replaced name.
    let retained_path = archive.with_extension("original-retained-test");
    fs::rename(&archive, &retained_path).unwrap();
    fs::write(
        &archive,
        b"replacement inode must never be selected by a retry",
    )
    .unwrap();
    drop(pressure);
    assert!(registration.poll_wait(&release, &mut context).is_ready());
    assert_eq!(budget.reserved_bytes(), baseline);
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let refusal = norito::with_decode_limits_scope(limits, || read.complete()).unwrap_err();
    let NativeAmxRecordProofErrorV1::Codec(refusal) = refusal else {
        panic!("exact inherited decoder refusal: {refusal:?}");
    };
    assert!(refusal.decode_resource_error().is_some());
    assert_eq!(
        budget.reserved_bytes(),
        baseline,
        "the complete original archive and reader still own their charges"
    );
    let proof = read.complete().unwrap().unwrap();
    assert!(proof.belongs_to(&budget));
    let tracker =
        AmxForeignInstanceV1::new(chain.instance().0, genesis_epoch(chain.genesis()).unwrap())
            .unwrap();
    assert_eq!(tracker.verify_record(proof.canonical()).unwrap().height, 2);
    assert!(matches!(&proof.canonical().record, AmxRecordV1::Begin(begin) if begin.tx == tx));
    assert!(
        read.complete().is_err(),
        "completion transfers the proof exactly once"
    );
    fs::remove_file(&archive).unwrap();
    fs::rename(&retained_path, &archive).unwrap();
    assert_eq!(fs::read(&archive).unwrap(), original_file);
    let retained = budget.reserved_bytes();
    let proof_bytes = proof.allocation_bytes().unwrap();
    drop(proof);
    assert_eq!(
        budget.reserved_bytes(),
        retained - proof_bytes,
        "the complete proof graph refunds exactly after its last owner"
    );
    drop(registration);
    drop(read);
    // All job fields and their actual charges are destroyed with the original job; the State
    // view and authenticated history remain independent owners until their own normal drops.
}

// Replace only the name of the archive directory. The reader still owns its original
// descriptor and original file; restoration installs that exact inode, never a copy.
struct ReplacedAmxArchiveNamespace {
    original: PathBuf,
    retained: PathBuf,
    replaced: std::cell::Cell<bool>,
}
impl ReplacedAmxArchiveNamespace {
    fn new(chain: &CertifiedTestChain) -> Self {
        let original = chain.kura().store_root().join("native-contexts");
        let retained = chain
            .kura()
            .store_root()
            .join("native-contexts-original-test");
        assert!(!retained.exists());
        Self {
            original,
            retained,
            replaced: std::cell::Cell::new(false),
        }
    }
    fn replace(&self) {
        assert!(!self.replaced.get());
        fs::rename(&self.original, &self.retained).unwrap();
        self.replaced.set(true);
        fs::create_dir(&self.original).unwrap();
    }
    fn restore(&self) {
        assert!(self.replaced.get());
        fs::remove_dir(&self.original).unwrap();
        fs::rename(&self.retained, &self.original).unwrap();
        self.replaced.set(false);
    }
}
impl Drop for ReplacedAmxArchiveNamespace {
    fn drop(&mut self) {
        if self.replaced.get() {
            // Best-effort fixture cleanup also runs when a causal assertion unwinds.
            let _ = fs::remove_dir(&self.original);
            let _ = fs::rename(&self.retained, &self.original);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AmxProofBackingIdentity {
    pointers: [usize; 5],
    capacities: [usize; 5],
    allocation_bytes: usize,
}
fn amx_proof_backing_identity(proof: &AllocatedAmxRecordProofV1) -> AmxProofBackingIdentity {
    let canonical = proof.canonical();
    let AmxRecordV1::Begin(begin) = &canonical.record else {
        panic!("genuinely committed original Begin fixture");
    };
    AmxProofBackingIdentity {
        pointers: [
            canonical.block.consensus_header.as_ptr() as usize,
            canonical.block.commit_qc.as_ptr() as usize,
            canonical.block.result_preimage.as_ptr() as usize,
            canonical.write.siblings.as_ptr() as usize,
            begin.participants.as_ptr() as usize,
        ],
        capacities: [
            canonical.block.consensus_header.capacity(),
            canonical.block.commit_qc.capacity(),
            canonical.block.result_preimage.capacity(),
            canonical.write.siblings.capacity(),
            begin.participants.capacity(),
        ],
        allocation_bytes: proof.allocation_bytes().unwrap(),
    }
}

#[test]
fn persisted_amx_completed_proof_retains_exact_graph_through_final_namespace_refusal() {
    use crate::query::{
        native_context_archive::NativeContextArchiveError,
        native_receipts::NativeAmxRecordProofErrorV1,
    };
    use iroha_allocation::AllocationRefusal;
    use std::cell::Cell;

    let (chain, tx) = chain();
    let replacement = ReplacedAmxArchiveNamespace::new(&chain);
    #[cfg(unix)]
    let original_directory_identity = {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = fs::metadata(&replacement.original).unwrap();
        (metadata.dev(), metadata.ino())
    };
    let archive = path(&chain);
    let original_file = fs::read(&archive).unwrap();
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let observed_backing = Cell::new(None);
    let observed_reserved = Cell::new(0);
    let mut registration = crate::unit_test_support::release_registration(&budget);
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    read.probe_portable_prepared_once(|proof| {
        let proof = proof.as_ref().expect("actual complete funded Begin proof");
        assert!(proof.belongs_to(&budget));
        observed_backing.set(Some(amx_proof_backing_identity(proof)));
        observed_reserved.set(budget.reserved_bytes());
        replacement.replace();
    })
    .unwrap();
    let refusal = read.complete().unwrap_err();
    let NativeAmxRecordProofErrorV1::Archive(NativeContextArchiveError::Io(error)) = refusal else {
        panic!("final original archive namespace refusal: {refusal:?}");
    };
    assert_eq!(error.kind(), std::io::ErrorKind::Other);
    assert_eq!(error.to_string(), "native context record identity changed");
    drop(error);
    let original = observed_backing
        .get()
        .expect("construction seam was reached");
    assert!(original.allocation_bytes > 0);
    assert_eq!(
        budget.reserved_bytes(),
        observed_reserved.get(),
        "a failed final namespace guard must retain the completed original graph"
    );
    replacement.restore();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let metadata = fs::metadata(&replacement.original).unwrap();
        assert_eq!(
            (metadata.dev(), metadata.ino()),
            original_directory_identity
        );
    }
    assert_eq!(fs::read(&archive).unwrap(), original_file);

    // Neither a new tree nor a new portable graph can be admitted while all remaining
    // original pool credit is occupied, and the caller grants no new decode allocation.
    let pressure = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let refusal = budget.try_reserve_bytes(1).unwrap_err();
    let AllocationRefusal::Capacity { release, .. } = refusal else {
        panic!("actual occupied original pool: {refusal:?}");
    };
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let mut retry = None;
    let allocations = norito::with_decode_limits_scope(limits, || {
        crate::test_allocations::allocations_during(|| retry = Some(read.complete()))
    });
    let proof = retry.unwrap().unwrap().unwrap();
    assert_eq!(
        allocations, 0,
        "same completed graph retry allocates nothing"
    );
    assert!(proof.belongs_to(&budget));
    assert_eq!(amx_proof_backing_identity(&proof), original);
    let tracker =
        AmxForeignInstanceV1::new(chain.instance().0, genesis_epoch(chain.genesis()).unwrap())
            .unwrap();
    assert_eq!(tracker.verify_record(proof.canonical()).unwrap().height, 2);
    assert!(matches!(&proof.canonical().record, AmxRecordV1::Begin(begin) if begin.tx == tx));
    assert!(
        read.complete().is_err(),
        "completed proof delivers exactly once"
    );
    let retained = budget.reserved_bytes();
    assert_eq!(retained, budget.limit_bytes());
    assert!(registration.poll_wait(&release, &mut context).is_pending());
    drop(proof);
    assert_eq!(
        budget.reserved_bytes(),
        retained - original.allocation_bytes,
        "completed proof fields must refund only after their last owner"
    );
    assert!(registration.poll_wait(&release, &mut context).is_ready());
    drop(pressure);
    drop(registration);
    drop(read);
}

#[test]
fn persisted_amx_authenticated_absence_retains_final_guard_and_one_shot_delivery() {
    use crate::query::{
        native_context_archive::NativeContextArchiveError,
        native_receipts::NativeAmxRecordProofErrorV1,
    };
    use std::cell::Cell;

    let (chain, tx) = chain();
    let replacement = ReplacedAmxArchiveNamespace::new(&chain);
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let observed = Cell::new(0);
    let retained = Cell::new(0);
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Decision, tx);
    read.probe_portable_prepared_once(|proof| {
        assert!(
            proof.is_none(),
            "the certified original height has no Decision"
        );
        observed.set(observed.get() + 1);
        retained.set(budget.reserved_bytes());
        replacement.replace();
    })
    .unwrap();
    assert!(matches!(
        read.complete(),
        Err(NativeAmxRecordProofErrorV1::Archive(
            NativeContextArchiveError::Io(_)
        ))
    ));
    assert_eq!(observed.get(), 1);
    assert_eq!(budget.reserved_bytes(), retained.get());
    replacement.restore();
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let mut retry = None;
    let allocations = norito::with_decode_limits_scope(limits, || {
        crate::test_allocations::allocations_during(|| retry = Some(read.complete()))
    });
    assert!(retry.unwrap().unwrap().is_none());
    assert_eq!(allocations, 0);
    assert_eq!(
        observed.get(),
        1,
        "authenticated absence is not a fresh construction"
    );
    assert_eq!(budget.reserved_bytes(), retained.get());
    assert!(
        read.complete().is_err(),
        "authenticated absence delivers exactly once"
    );
    assert_eq!(budget.reserved_bytes(), retained.get());
}
