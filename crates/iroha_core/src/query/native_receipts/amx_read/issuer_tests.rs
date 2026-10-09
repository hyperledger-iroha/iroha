//! Genuine native certification and refused original-descriptor issuance, without a State borrow.

use std::{cell::Cell, fs, path::PathBuf};

use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBufferError};
use iroha_data_model::{
    block::SignedBlock,
    isi::sumeragi_amx::{BeginAmxV1, RegisterAmxDataspaceV1},
    sumeragi_amx::{AmxForeignInstanceV1, AmxLegV1, AmxRecordKind, AmxRecordV1, AmxTransactionV1},
    sumeragi_finality::{genesis_epoch, test_fixtures::NativeFinalityFixture},
};
use iroha_model_base::topology::DataSpaceId;

use super::{
    NativeAmxRecordProofErrorV1 as Error, NativeAmxRecordProofIssuedV1 as Issued,
    NativeAmxRecordProofPollV1 as Poll,
};
use crate::{
    query::{native_context_archive::NativeContextArchiveError, native_receipts::amx_record_proof},
    state::{NativeExecutionProjectionV1, World},
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

// The genuine canonical transaction is committed by an independent native test-chain owner.
// No issued carrier, proof, context frame or paid graph is installed by this helper.
pub(super) fn committed_source(count: usize) -> (CertifiedTestChain, [u8; 32], PathBuf) {
    assert!((1..=iroha_data_model::sumeragi_amx::MAX_AMX_PENDING).contains(&count));
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.genesis_instructions = [21, 22]
        .into_iter()
        .map(|id| {
            let child = NativeFinalityFixture::start(&format!("issued-amx-source-{id}"));
            RegisterAmxDataspaceV1 {
                dataspace: DataSpaceId::new(id),
                instance: child.verifier().instance().0,
                anchor: norito::encode_canonical(&genesis_epoch(child.genesis()).unwrap()).unwrap(),
            }
            .into()
        })
        .collect();
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
    let instructions: Vec<iroha_data_model::isi::InstructionBox> = (0..count)
        .map(|index| {
            let mut additional = transaction.clone();
            if index != 0 {
                additional.nonce = [0; 32];
                additional.nonce[..8].copy_from_slice(&u64::try_from(index).unwrap().to_le_bytes());
            }
            BeginAmxV1 {
                transaction: additional,
            }
            .into()
        })
        .collect();
    let signed = chain.sign(&authority, instructions, 1_999);
    assert_eq!(chain.commit_at(2_000, vec![signed]), vec![true]);
    let paths = fs::read_dir(chain.kura().store_root().join("native-contexts"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "nrt"))
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("00000000000000000002-")
        })
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 1);
    (chain, tx, paths.into_iter().next().unwrap())
}

// Custody assertions use borrowed/scalar identities only; no proof fields are copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProofIdentity {
    result: *const u8,
    participants: *const DataSpaceId,
    bytes: usize,
}
fn proof_identity(
    proof: &iroha_data_model::sumeragi_amx::AllocatedAmxRecordProofV1,
) -> ProofIdentity {
    let canonical = proof.canonical();
    let AmxRecordV1::Begin(begin) = &canonical.record else {
        panic!("independently committed Begin source");
    };
    ProofIdentity {
        result: canonical.block.result_preimage.as_ptr(),
        participants: begin.participants.as_ptr(),
        bytes: proof.allocation_bytes().unwrap(),
    }
}

struct ReplacedNamespace {
    original: PathBuf,
    retained: PathBuf,
    replaced: Cell<bool>,
}
impl ReplacedNamespace {
    fn new(chain: &CertifiedTestChain) -> Self {
        let original = chain.kura().store_root().join("native-contexts");
        let retained = chain.kura().store_root().join("issued-original-namespace");
        assert!(!retained.exists());
        Self {
            original,
            retained,
            replaced: Cell::new(false),
        }
    }
    fn replace(&self) {
        assert!(!self.replaced.replace(true));
        fs::rename(&self.original, &self.retained).unwrap();
        fs::create_dir(&self.original).unwrap();
    }
    fn restore(&self) {
        assert!(self.replaced.get());
        fs::remove_dir(&self.original).unwrap();
        fs::rename(&self.retained, &self.original).unwrap();
        self.replaced.set(false);
    }
}
impl Drop for ReplacedNamespace {
    fn drop(&mut self) {
        if self.replaced.get() {
            let _ = fs::remove_dir(&self.original);
            let _ = fs::rename(&self.retained, &self.original);
        }
    }
}

#[test]
fn owned_issuer_retains_first_refused_archive_descriptor_after_original_view_drop() {
    let (chain, tx, archive) = committed_source(1);
    let original_file = fs::read(&archive).unwrap();
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    // Complete actual certification before applying pool pressure so the causal refusal is
    // the genuine first archive poll, not an unrelated earlier prefix/scratch refusal.
    read.acquire_original_source().unwrap();
    assert!(
        !read
            .source
            .as_ref()
            .unwrap()
            .read
            .as_ref()
            .unwrap()
            .has_pinned_source()
    );
    let carrier: *const SignedBlock = read
        .source
        .as_ref()
        .unwrap()
        .certified
        .as_ref()
        .unwrap()
        .block()
        .as_ref();
    let retained = budget.reserved_bytes();
    let pressure = budget
        .try_reserve_bytes(budget.limit_bytes() - retained)
        .unwrap();
    let expected = budget.try_reserve_bytes(original_file.len()).unwrap_err();
    assert!(matches!(expected, AllocationRefusal::Capacity { .. }));
    let Issued::Refused {
        mut original,
        cause,
    } = read.try_issue().unwrap()
    else {
        panic!("occupied original frame pool must issue its exact refused job");
    };
    let Error::Archive(NativeContextArchiveError::Allocation(ChargedBufferError::Admission(
        actual,
    ))) = cause
    else {
        panic!("actual original archive Capacity must remain typed: {cause:?}");
    };
    assert_eq!(actual, expected);
    assert!(
        original.source.read.as_ref().unwrap().has_pinned_source(),
        "owned issuer must retain the first refused archive descriptor"
    );
    assert!(original.acquired_frame().is_none());
    assert!(original.source.budget.same_pool(&budget));
    assert!(std::ptr::eq::<SignedBlock>(
        original.source.certified.as_ref().unwrap().block().as_ref(),
        carrier
    ));
    assert_eq!(
        budget.reserved_bytes(),
        budget.limit_bytes(),
        "issuance does not refund prefix/source charges within the original State borrow"
    );
    assert!(read.source.is_none());
    assert!(
        read.chain.is_some(),
        "the borrowed verifier retires only at normal scoped drop"
    );
    assert!(read.try_issue().is_err());
    drop(read);
    drop(view);

    let retained_path = archive.with_extension("issued-original");
    fs::rename(&archive, &retained_path).unwrap();
    fs::write(&archive, vec![0; original_file.len()]).unwrap();
    drop(pressure);
    let baseline = budget.reserved_bytes();
    assert!(matches!(original.poll().unwrap(), Poll::Pending));
    let prefix = original.acquired_frame().unwrap();
    assert!(!prefix.is_empty() && prefix.len() <= 4096);
    assert_eq!(
        prefix,
        &original_file[..prefix.len()],
        "owned issuer must retain the first refused archive descriptor"
    );
    let pointer = prefix.as_ptr();
    while original.acquired_frame().unwrap().len() < original_file.len() {
        assert!(matches!(original.poll().unwrap(), Poll::Pending));
        assert_eq!(original.acquired_frame().unwrap().as_ptr(), pointer);
    }
    assert_eq!(original.acquired_frame().unwrap(), original_file);
    assert_eq!(budget.reserved_bytes(), baseline + original_file.len());
    let proof = original.complete().unwrap().unwrap();
    assert!(proof.belongs_to(&budget));
    let foreign = AllocationBudget::new(budget.limit_bytes());
    assert!(!proof.belongs_to(&foreign));
    let tracker =
        AmxForeignInstanceV1::new(chain.instance().0, genesis_epoch(chain.genesis()).unwrap())
            .unwrap();
    assert_eq!(tracker.verify_record(proof.canonical()).unwrap().height, 2);
    assert!(matches!(&proof.canonical().record, AmxRecordV1::Begin(begin) if begin.tx == tx));
    assert!(original.complete().is_err(), "delivery remains one-shot");
    let before_drop = budget.reserved_bytes();
    let bytes = proof.allocation_bytes().unwrap();
    drop(proof);
    assert_eq!(budget.reserved_bytes(), before_drop - bytes);
    // The archive frame, projection and issued certificate retire at their normal owner
    // drops; native prefix internals remain a separate accounting boundary.
    drop(original);
    fs::remove_file(&archive).unwrap();
    fs::rename(&retained_path, &archive).unwrap();
}

#[test]
fn owned_issuer_preserves_partial_frame_and_same_pool_without_an_extra_poll() {
    let count = 4096 / iroha_data_model::sumeragi_amx::AMX_RECORD_WITNESS_KEY_BYTES + 1;
    let (chain, tx, archive) = committed_source(count);
    let original_file = fs::read(&archive).unwrap();
    assert!(original_file.len() > 4096);
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    assert!(matches!(read.poll().unwrap(), Poll::Pending));
    let prefix = read.acquired_frame().unwrap();
    let length = prefix.len();
    let pointer = prefix.as_ptr();
    let before_issue = budget.reserved_bytes();
    let pressure = budget
        .try_reserve_bytes(budget.limit_bytes() - before_issue)
        .unwrap();
    let mut issued = None;
    let allocations =
        crate::test_allocations::allocations_during(|| issued = Some(read.try_issue()));
    let Issued::Acquired(mut original) = issued.unwrap().unwrap() else {
        panic!("previously acquired prefix must move without a new poll");
    };
    assert_eq!(allocations, 0);
    assert_eq!(original.acquired_frame().unwrap().len(), length);
    assert_eq!(original.acquired_frame().unwrap().as_ptr(), pointer);
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    assert!(original.source.budget.same_pool(&budget));
    drop(read);
    drop(view);
    drop(pressure);
    let retained = budget.reserved_bytes();
    while original.acquired_frame().unwrap().len() < original_file.len() {
        assert!(matches!(original.poll().unwrap(), Poll::Pending));
        assert_eq!(original.acquired_frame().unwrap().as_ptr(), pointer);
        assert_eq!(budget.reserved_bytes(), retained);
    }
    assert_eq!(original.acquired_frame().unwrap(), original_file);
    let proof = original.complete().unwrap().unwrap();
    assert!(proof.belongs_to(&budget));
    let tracker =
        AmxForeignInstanceV1::new(chain.instance().0, genesis_epoch(chain.genesis()).unwrap())
            .unwrap();
    tracker.verify_record(proof.canonical()).unwrap();
}

#[test]
fn owned_issuer_keeps_missing_uncertified_and_armed_probe_sources_in_borrower() {
    let (chain, tx, archive) = committed_source(1);
    let retained_path = archive.with_extension("missing-original");
    fs::rename(&archive, &retained_path).unwrap();
    let view = chain.state().view();
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    assert!(
        matches!(read.try_issue(), Err(Error::Archive(NativeContextArchiveError::Io(ref cause))) if cause.kind() == std::io::ErrorKind::NotFound)
    );
    assert!(read.source.is_some());
    assert!(read.source.as_ref().unwrap().certified.is_some());
    assert!(
        !read
            .source
            .as_ref()
            .unwrap()
            .read
            .as_ref()
            .unwrap()
            .has_pinned_source()
    );
    fs::rename(&retained_path, &archive).unwrap();
    let Issued::Acquired(mut original) = read.try_issue().unwrap() else {
        panic!("restoring the exact unselected file permits ordinary acquisition");
    };
    assert!(original.complete().unwrap().is_some());

    let mut uncertified = amx_record_proof(&view, 3, AmxRecordKind::Begin, tx);
    assert!(matches!(uncertified.try_issue(), Err(Error::Chain(_))));
    assert!(uncertified.source.as_ref().unwrap().certified.is_none());
    assert!(uncertified.source.as_ref().unwrap().read.is_none());
    let called = Cell::new(false);
    let mut probed = amx_record_proof(&view, 2, AmxRecordKind::Decision, tx);
    assert!(matches!(probed.poll().unwrap(), Poll::Pending));
    let pointer = probed.acquired_frame().unwrap().as_ptr();
    probed
        .probe_portable_prepared_once(|proof| {
            assert!(proof.is_none());
            called.set(true);
        })
        .unwrap();
    assert!(matches!(
        probed.try_issue(),
        Err(Error::Source(
            "borrowed portable observer must finish before detach"
        ))
    ));
    assert!(!called.get());
    assert_eq!(probed.acquired_frame().unwrap().as_ptr(), pointer);
    assert!(probed.source.is_some());
    assert!(probed.complete().unwrap().is_none());
    assert!(called.get());
    assert!(
        probed.try_issue().is_err(),
        "completed authenticated absence cannot be reissued"
    );
}

#[test]
fn owned_issuer_carries_final_namespace_refusal_without_rebuilding_completed_graph() {
    let (chain, tx, _) = committed_source(1);
    let replacement = ReplacedNamespace::new(&chain);
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let observed = Cell::new(None);
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    read.probe_portable_prepared_once(|proof| {
        observed.set(Some(proof_identity(proof.as_ref().unwrap())));
        replacement.replace();
    })
    .unwrap();
    assert!(matches!(
        read.complete(),
        Err(Error::Archive(NativeContextArchiveError::Io(_)))
    ));
    let retained = budget.reserved_bytes();
    let Issued::Refused {
        mut original,
        cause,
    } = read.try_issue().unwrap()
    else {
        panic!("a final namespace refusal remains explicit during ownership transfer");
    };
    assert!(
        matches!(cause, Error::Archive(NativeContextArchiveError::Io(ref error)) if error.kind() == std::io::ErrorKind::Other && error.to_string() == "native context record identity changed")
    );
    assert_eq!(budget.reserved_bytes(), retained);
    assert_eq!(
        proof_identity(original.source.portable.as_ref().unwrap().as_ref().unwrap()),
        observed.get().unwrap()
    );
    drop(read);
    drop(view);
    assert!(matches!(
        original.complete(),
        Err(Error::Archive(NativeContextArchiveError::Io(_)))
    ));
    replacement.restore();
    let pressure = budget
        .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
        .unwrap();
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    let mut completed = None;
    let allocations = norito::with_decode_limits_scope(limits, || {
        crate::test_allocations::allocations_during(|| completed = Some(original.complete()))
    });
    assert_eq!(allocations, 0);
    let proof = completed.unwrap().unwrap().unwrap();
    assert_eq!(proof_identity(&proof), observed.get().unwrap());
    assert!(proof.belongs_to(&budget));
    assert!(original.complete().is_err());
    let before_drop = budget.reserved_bytes();
    drop(proof);
    assert_eq!(
        budget.reserved_bytes(),
        before_drop - observed.get().unwrap().bytes
    );
    drop(pressure);
}

#[test]
fn owned_issuer_retains_original_decoder_refusal_and_rejects_substituted_carrier_fields() {
    let (chain, tx, archive) = committed_source(1);
    let original_file = fs::read(&archive).unwrap();
    let mut changed: NativeExecutionProjectionV1 =
        norito::decode_canonical(&original_file).unwrap();
    changed.carrier_hash = iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
        b"foreign issued AMX carrier",
    ));
    fs::write(&archive, norito::encode_canonical(&changed).unwrap()).unwrap();
    let view = chain.state().view();
    let budget = chain.state().ivm_execution_budget();
    let mut read = amx_record_proof(&view, 2, AmxRecordKind::Begin, tx);
    let Issued::Acquired(mut original) = read.try_issue().unwrap() else {
        panic!("acquiring untrusted bytes is distinct from authenticating the carrier fields");
    };
    let pointer = original.acquired_frame().unwrap().as_ptr();
    drop(read);
    drop(view);
    let retained = budget.reserved_bytes();
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
    assert!(
        matches!(norito::with_decode_limits_scope(limits, || original.complete()), Err(Error::Codec(ref cause)) if cause.decode_resource_error().is_some())
    );
    assert_eq!(original.acquired_frame().unwrap().as_ptr(), pointer);
    assert_eq!(
        budget.reserved_bytes(),
        retained,
        "refused decode retires only its temporary plan, retaining the original frame"
    );
    assert!(matches!(
        original.complete(),
        Err(Error::Source(
            "archive differs from the certified carrier or original pool"
        ))
    ));
    fs::write(&archive, original_file).unwrap();
    assert!(matches!(
        original.complete(),
        Err(Error::Source(
            "archive differs from the certified carrier or original pool"
        ))
    ));
    assert_eq!(original.acquired_frame().unwrap().as_ptr(), pointer);
    assert!(original.source.budget.same_pool(&budget));
}
