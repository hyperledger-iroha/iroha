//! Actual signed model fixtures projected through the retained stored-body boundary.

use super::*;
use crate::sumeragi::crypto::BlsCrypto;
use iroha_data_model::{
    block::{CommitCertificate, decode_versioned_signed_block},
    sumeragi_finality::{ScheduledSlot, test_fixtures::NativeFinalityFixture},
};
use iroha_sumeragi::{availability::AvailabilityFrame, types::Hash32};

fn fixture() -> (Arc<SignedBlock>, AvailabilitySource, SharedCrypto) {
    let fixture = NativeFinalityFixture::new();
    let parent = fixture
        .verifier()
        .verify_retained_decision(fixture.genesis_proof())
        .unwrap();
    let ScheduledSlot::Ready(scheduled) = &parent.commitment().schedule.next else {
        panic!("authenticated next fixture authority");
    };
    let source = Arc::new(decode_versioned_signed_block(&fixture.latest().block_wire).unwrap());
    let header: iroha_sumeragi::message::BlockHeader =
        norito::decode_canonical(source.commit_certificate().unwrap().consensus_header()).unwrap();
    let crypto: SharedCrypto = Arc::new(BlsCrypto::new());
    let authority = AvailabilitySource::new(
        fixture.verifier().instance(),
        2,
        header.hash(&*crypto),
        scheduled.height_config().unwrap(),
    )
    .unwrap();
    (source, authority, crypto)
}

#[test]
fn stored_body_retains_original_certificate_and_projected_payload_across_refusals() {
    let (block, source, crypto) = fixture();
    let measure = AllocationBudget::new(1 << 25);
    let decoded = CertificateRead::new(block.clone(), measure.clone())
        .complete(&measure)
        .unwrap_or_else(|_| panic!("measure actual decoded artifact"));
    let decoded_size = measure.reserved_bytes();
    drop(decoded);
    assert_eq!(measure.reserved_bytes(), 0);
    let budget = AllocationBudget::new(0);
    let mut read = StoredBodyRead::new(
        source.clone(),
        Some(block.clone()),
        budget.clone(),
        crypto.clone(),
    );
    assert!(matches!(read.poll(&budget), Ok(BodyReadPoll::Pending(_))));
    assert_eq!(read.source(), &source);
    let Stage::Certificate(job) = &read.stage else {
        panic!("same certificate owner")
    };
    assert!(Arc::ptr_eq(job.source(), &block));

    budget.set_limit_bytes(decoded_size);
    assert!(matches!(read.poll(&budget), Ok(BodyReadPoll::Pending(_))));
    let Stage::Projecting(job) = &read.stage else {
        panic!("one funded projection")
    };
    assert!(Arc::ptr_eq(&job.source().source, &block));
    let table_pointer = job.source().availability.as_slice().as_ptr();
    let payload_len = block.resultless_proposal_wire_len().unwrap();
    budget.set_limit_bytes(decoded_size + payload_len);
    assert!(matches!(read.poll(&budget), Ok(BodyReadPoll::Pending(_))));
    let held = budget.reserved_bytes();
    assert_eq!(held, decoded_size + payload_len);
    assert!(matches!(read.poll(&budget), Ok(BodyReadPoll::Pending(_))));
    assert_eq!(budget.reserved_bytes(), held);
    let Stage::Projecting(job) = &read.stage else {
        panic!("same encoded projection")
    };
    assert!(Arc::ptr_eq(&job.source().source, &block));
    assert_eq!(job.source().availability.as_slice().as_ptr(), table_pointer);

    budget.set_limit_bytes(1 << 25);
    let BodyReadPoll::Ready(restoration) = read.poll(&budget).unwrap() else {
        panic!("original exact body")
    };
    assert_eq!(restoration.source(), &source);
    assert!(matches!(read.poll(&budget), Err(BodyReadError::Completed)));
    let body = restoration
        .complete(&budget, &*crypto)
        .unwrap_or_else(|_| panic!("real signatures and full codeword verify"));
    assert_eq!(body.availability().as_slice().as_ptr(), table_pointer);
    assert_eq!(
        body.payload().as_slice(),
        block
            .canonical_resultless_proposal()
            .expect("valid fixture proposal projection")
            .encode_wire()
            .unwrap()
    );
    assert!(body.admitted_to(&budget));
    drop(body);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn absence_foreign_pool_and_corrupt_storage_are_separate_outcomes() {
    let (block, source, crypto) = fixture();
    let budget = AllocationBudget::new(1 << 25);
    let mut absent = StoredBodyRead::new(source.clone(), None, budget.clone(), crypto.clone());
    assert!(matches!(
        absent.poll(&AllocationBudget::new(1 << 25)),
        Err(BodyReadError::ForeignBudget)
    ));
    assert!(matches!(absent.poll(&budget), Ok(BodyReadPoll::Absent)));
    assert!(matches!(
        absent.poll(&budget),
        Err(BodyReadError::Completed)
    ));
    let corrupt = Arc::new(block.as_ref().clone().with_commit_certificate(None));
    let mut read = StoredBodyRead::new(source, Some(corrupt), budget.clone(), crypto);
    for _ in 0..2 {
        assert!(
            matches!(read.poll(&budget), Err(BodyReadError::Io(ref e)) if e.kind() == io::ErrorKind::InvalidData)
        );
    }
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn requested_hash_cannot_be_replaced_by_the_stored_certificate() {
    let (block, source, crypto) = fixture();
    let source = AvailabilitySource::new(
        source.instance(),
        source.height(),
        Hash32([0x91; 32]),
        source.config().clone(),
    )
    .unwrap();
    let budget = AllocationBudget::new(1 << 25);
    let mut read = StoredBodyRead::new(source.clone(), Some(block.clone()), budget.clone(), crypto);
    for _ in 0..2 {
        assert!(
            matches!(read.poll(&budget), Err(BodyReadError::Io(ref e)) if e.kind() == io::ErrorKind::InvalidData)
        );
        let Stage::Decoded(decoded) = &read.stage else {
            panic!("keep exact rejected artifact")
        };
        assert!(Arc::ptr_eq(&decoded.source, &block));
        assert_eq!(read.source(), &source);
    }
}

#[test]
fn canonical_but_invalid_author_signature_never_becomes_available_custody() {
    let (block, source, crypto) = fixture();
    let old = block.commit_certificate().unwrap();
    let original: AvailabilityFrame = norito::decode_canonical(old.availability()).unwrap();
    let mut table = original.as_slice().to_vec();
    let last = table.len() - 1;
    table[last] ^= 1;
    let table = AvailabilityFrame::from_untrusted(table).unwrap();
    let certificate = CommitCertificate::from_untrusted_parts(
        old.consensus_header().to_vec(),
        old.commit_qc().to_vec(),
        old.result_preimage().to_vec(),
        norito::encode_canonical(&table).unwrap(),
    );
    let changed = Arc::new(
        block
            .as_ref()
            .clone()
            .with_commit_certificate(Some(certificate)),
    );
    let budget = AllocationBudget::new(1 << 25);
    let mut read = StoredBodyRead::new(source, Some(changed), budget.clone(), crypto.clone());
    let BodyReadPoll::Ready(restoration) = read.poll(&budget).unwrap() else {
        panic!("canonical untrusted input only")
    };
    let (_, error) = restoration
        .complete(&budget, &*crypto)
        .err()
        .expect("actual signature rejection");
    assert!(!error.is_local_refusal());
}
