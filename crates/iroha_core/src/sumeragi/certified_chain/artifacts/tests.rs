//! Original source, exact bulk funding, retry and certificate-verification parity.

use super::*;
use crate::state::World;
use crate::sumeragi::{
    certified_chain::{CertifiedPrefix, QcVerification},
    test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};
use iroha_data_model::block::CommitCertificate;
use iroha_model_base::chain::ChainId;
use iroha_sumeragi::message::ResultWitness;
use std::num::NonZeroUsize;

fn chain() -> CertifiedTestChain {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    chain
}
fn frame(chain: &CertifiedTestChain, height: usize) -> Arc<SignedBlock> {
    chain
        .kura()
        .get_block(NonZeroUsize::new(height).unwrap())
        .unwrap()
}
fn read(source: Arc<SignedBlock>, budget: &AllocationBudget) -> PrefixArtifacts {
    PrefixArtifactsRead::new(source, budget.clone())
        .complete(budget)
        .unwrap_or_else(|(_, error)| panic!("canonical original artifacts: {error}"))
}
fn changed_qc(source: &SignedBlock, mutate: impl FnOnce(&mut Qc)) -> Arc<SignedBlock> {
    let certificate = source.commit_certificate().unwrap();
    let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
    mutate(&mut qc);
    Arc::new(
        source
            .clone()
            .with_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
                certificate.consensus_header().to_vec(),
                norito::encode_canonical(&qc).unwrap(),
                certificate.result_preimage().to_vec(),
                certificate.availability().to_vec(),
            ))),
    )
}

#[test]
fn prefix_artifacts_keep_exact_source_and_all_bulk_owners_in_original_pool() {
    let chain = chain();
    let source = frame(&chain, 2);
    let budget = AllocationBudget::new(1 << 26);
    let owner = read(Arc::clone(&source), &budget);
    assert!(Arc::ptr_eq(owner.source(), &source));
    assert!(owner.payload.admitted_to(&budget));
    assert!(owner.decoded.availability.admitted_to(&budget));
    assert!(!owner.payload.admitted_to(&AllocationBudget::new(1 << 26)));
    let mut expected = Vec::new();
    source
        .write_resultless_proposal_wire(&mut expected)
        .unwrap();
    assert_eq!(owner.payload.as_slice(), expected);
    assert_eq!(
        norito::encode_canonical(&owner.decoded.commit_qc).unwrap(),
        source.commit_certificate().unwrap().commit_qc()
    );
    assert!(budget.reserved_bytes() > expected.len() + owner.decoded.availability.as_slice().len());
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prefix_artifacts_refusal_keeps_table_witness_and_completed_projection() {
    let chain = chain();
    // Parser-only candidate; independent prefix verification must still reject this invented witness.
    let source = changed_qc(&frame(&chain, 2), |qc| {
        qc.attestation_witness = Some(ResultWitness::from_untrusted(vec![7; 4096]).unwrap());
    });
    let measure = AllocationBudget::new(1 << 26);
    let decoded = CertificateRead::new(Arc::clone(&source), measure.clone())
        .complete(&measure)
        .unwrap_or_else(|_| panic!("bounded candidate"));
    let certificate_bytes = measure.reserved_bytes();
    let table_len = decoded.availability.as_slice().len();
    drop(decoded);
    let measured = read(Arc::clone(&source), &measure);
    let full_bytes = measure.reserved_bytes();
    let payload_len = measured.payload.as_slice().len();
    drop(measured);
    assert_eq!(measure.reserved_bytes(), 0);

    let budget = AllocationBudget::new(0);
    let (job, error) = PrefixArtifactsRead::new(Arc::clone(&source), budget.clone())
        .complete(&budget)
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(budget.reserved_bytes(), 0);
    budget.set_limit_bytes(table_len);
    let (job, error) = job.complete(&budget).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    let Stage::Certificate(ref partial) = job.stage else {
        panic!("table control refused")
    };
    let originals = partial.retained_owners_for_test();
    assert_eq!(originals.0, Arc::as_ptr(&source));
    assert!(originals.1.is_some());
    let (job, _) = job.complete(&budget).err().unwrap();
    let Stage::Certificate(ref partial) = job.stage else {
        panic!("same certificate job")
    };
    assert_eq!(partial.retained_owners_for_test(), originals);

    budget.set_limit_bytes(certificate_bytes);
    let (job, error) = job.complete(&budget).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    let Stage::Projecting(ref partial) = job.stage else {
        panic!("payload backing refused")
    };
    let table = partial.source().availability.as_slice().as_ptr();
    let witness = partial
        .source()
        .commit_qc
        .attestation_witness
        .as_ref()
        .unwrap()
        .as_slice()
        .as_ptr();
    assert_eq!(Some(table), originals.1);
    assert_eq!(budget.reserved_bytes(), certificate_bytes);

    budget.set_limit_bytes(certificate_bytes + payload_len);
    let (job, error) = job.complete(&budget).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(budget.reserved_bytes(), certificate_bytes + payload_len);
    let (job, _) = job.complete(&budget).err().unwrap();
    assert_eq!(budget.reserved_bytes(), certificate_bytes + payload_len);
    budget.set_limit_bytes(full_bytes);
    let owner = job
        .complete(&budget)
        .unwrap_or_else(|(_, error)| panic!("retry: {error}"));
    assert!(Arc::ptr_eq(owner.source(), &source));
    assert_eq!(owner.decoded.availability.as_slice().as_ptr(), table);
    assert_eq!(
        owner
            .decoded
            .commit_qc
            .attestation_witness
            .as_ref()
            .unwrap()
            .as_slice()
            .as_ptr(),
        witness
    );
    assert!(
        owner
            .decoded
            .commit_qc
            .attestation_witness
            .as_ref()
            .unwrap()
            .admitted_to(&budget)
    );
    assert_eq!(budget.reserved_bytes(), full_bytes);
    drop(owner);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prefix_artifacts_reject_foreign_pool_and_drop_partial_owners_once() {
    let chain = chain();
    let source = frame(&chain, 2);
    let measure = AllocationBudget::new(1 << 26);
    let owner = read(Arc::clone(&source), &measure);
    let table_len = owner.decoded.availability.as_slice().len();
    drop(owner);
    let budget = AllocationBudget::new(table_len);
    let foreign = AllocationBudget::new(1 << 26);
    let (job, error) = PrefixArtifactsRead::new(source, budget.clone())
        .complete(&foreign)
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(foreign.reserved_bytes(), 0);
    let (job, error) = job.complete(&budget).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(budget.reserved_bytes(), table_len);
    let (job, error) = job.complete(&foreign).err().unwrap();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(budget.reserved_bytes(), table_len);
    drop(job);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prefix_artifacts_cannot_be_rebound_to_equal_hash_carrier_or_changed_header() {
    let chain = chain();
    let source = frame(&chain, 2);
    let budget = AllocationBudget::new(1 << 26);
    let owner = read(Arc::clone(&source), &budget);
    let replacement = Arc::new(source.as_ref().clone());
    assert_eq!(replacement.hash(), source.hash());
    let header = owner.decoded.header.clone();
    assert!(matches!(
        owner.into_parts(&replacement, &header),
        Err(ChainReadError::HeaderMismatch { height: 2 })
    ));
    let owner = read(Arc::clone(&source), &budget);
    let mut header = owner.decoded.header.clone();
    header.origin_view += 1;
    assert!(matches!(
        owner.into_parts(&source, &header),
        Err(ChainReadError::HeaderMismatch { height: 2 })
    ));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn funded_prefix_keeps_quorum_checks_and_matches_portable_prefix() {
    let chain = chain();
    let id = chain.state().chain_id_ref();
    let mut funded = CertifiedPrefix::new(id, chain.network_id(), frame(&chain, 1)).unwrap();
    let mut portable = CertifiedPrefix::new(id, chain.network_id(), frame(&chain, 1)).unwrap();
    let budget = AllocationBudget::new(1 << 26);
    let bad = changed_qc(&frame(&chain, 2), |qc| qc.agg_sig.0[0] ^= 1);
    assert!(funded.push_prepared(read(bad, &budget)).is_err());
    assert_eq!(
        funded.prefix.tip.height(),
        1,
        "invalid signature cannot advance cursor"
    );
    assert_eq!(budget.reserved_bytes(), 0);
    let original = frame(&chain, 2);
    let certificate = original.commit_certificate().unwrap();
    let mut table: AvailabilityFrame =
        norito::decode_canonical(certificate.availability()).unwrap();
    let mut bytes = table.as_slice().to_vec();
    bytes[10] ^= 1;
    table = AvailabilityFrame::from_untrusted(bytes).unwrap();
    let changed = Arc::new(original.as_ref().clone().with_commit_certificate(Some(
        CommitCertificate::from_untrusted_parts(
            certificate.consensus_header().to_vec(),
            certificate.commit_qc().to_vec(),
            certificate.result_preimage().to_vec(),
            norito::encode_canonical(&table).unwrap(),
        ),
    )));
    assert!(
        funded.push_prepared(read(changed, &budget)).is_err(),
        "signed table remains verified"
    );
    assert_eq!(funded.prefix.tip.height(), 1);
    assert_eq!(budget.reserved_bytes(), 0);
    for height in 2..=3 {
        let (actual, anchor) = funded
            .push_prepared(read(frame(&chain, height), &budget))
            .unwrap()
            .into_parts();
        let (expected, expected_anchor) =
            portable.push(frame(&chain, height)).unwrap().into_parts();
        assert_eq!(actual.verification(), QcVerification::Verified);
        assert_eq!(actual.core_hash(), expected.core_hash());
        assert_eq!(actual.result(), expected.result());
        assert_eq!(actual.commit_qc(), expected.commit_qc());
        assert_eq!(
            anchor.map(|a| a.successor()),
            expected_anchor.map(|a| a.successor())
        );
        drop(actual);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn funded_boundary_keeps_genuine_result_witness_until_receipt_drop() {
    // A global epoch pin cannot postpone release of the reader's actual witness owner.
    let _epoch = crossbeam_epoch::pin();
    let mut chain = CertifiedTestChain::npos_boundary_fixture();
    chain.commit_with(Some(10_000), Vec::new(), Signers::LastThree);
    let mut prefix = CertifiedPrefix::new(
        &ChainId::from("sumeragi-certified-test-chain"),
        chain.network_id(),
        frame(&chain, 1),
    )
    .unwrap();
    let budget = AllocationBudget::new(1 << 26);
    for height in 2..10 {
        prefix
            .push_prepared(read(frame(&chain, height), &budget))
            .unwrap();
    }
    assert_eq!(budget.reserved_bytes(), 0);
    let original = frame(&chain, 10);
    let tampered = changed_qc(&original, |qc| qc.attestation_witness = None);
    assert!(matches!(
        prefix.push_prepared(read(tampered, &budget)),
        Err(ChainReadError::Certificate { .. })
    ));
    assert_eq!(prefix.prefix.tip.height(), 9);
    let (receipt, _) = prefix
        .push_prepared(read(original, &budget))
        .unwrap()
        .into_parts();
    let witness = receipt
        .commit_qc()
        .unwrap()
        .attestation_witness
        .as_ref()
        .unwrap();
    assert!(witness.admitted_to(&budget));
    assert!(budget.reserved_bytes() > witness.as_slice().len());
    drop(receipt);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn physical_metadata_refusal_is_retryable_but_format_limits_are_not() {
    let refused = PrefixArtifactsError::Certificate(CertificateReadError::Decode(
        norito::Error::AllocationFailed { bytes: 128 },
    ));
    assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    assert!(matches!(
        refused,
        PrefixArtifactsError::Certificate(CertificateReadError::Decode(
            norito::Error::AllocationFailed { bytes: 128 }
        ))
    ));
    for error in [
        norito::Error::LengthMismatch,
        norito::Error::TotalAllocationExceeded {
            attempted: 129,
            limit: 128,
        },
    ] {
        assert_eq!(
            PrefixArtifactsError::Certificate(CertificateReadError::Decode(error)).kind(),
            io::ErrorKind::InvalidData
        );
    }
}
