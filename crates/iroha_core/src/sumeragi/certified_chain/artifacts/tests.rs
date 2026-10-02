//! Original source, exact bulk funding, retry and certificate-verification parity.

use super::*;
use crate::state::World;
use crate::sumeragi::{
    certified_chain::{CertifiedPrefix, QcVerification},
    test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};
use iroha_data_model::{block::CommitCertificate, sumeragi_finality::EpochValidationScope};
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
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            ChainReadError::Certificate { .. }
        ))
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

#[test]
fn funded_original_result_witness_is_borrowed_without_redecoding() {
    use crate::sumeragi::{
        certified_chain::{CertifiedChain, VerifiedAuthority, read_frame},
        schedule,
    };

    let _epoch = crossbeam_epoch::pin();
    let mut chain = CertifiedTestChain::npos_boundary_fixture();
    chain.commit_with(Some(10_000), Vec::new(), Signers::LastThree);
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let original = frame(&chain, 10);
    let preimage = original.commit_certificate().unwrap().result_preimage();
    let original_preimage = preimage.as_ptr();
    let current = read_frame(Arc::clone(&original), 10).unwrap();
    let original_committee = current.commitment().schedule.current.committee.as_ptr();
    let parent = chain.committed(9);
    let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment().schedule.next else {
        panic!("actual authenticated parent authorizes the boundary");
    };
    let mut validation = EpochValidationScope::new();
    let config = scheduled
        .height_config_with_validation(&mut validation)
        .unwrap();
    let authority = VerifiedAuthority::new(scheduled.epoch.clone(), 10, &mut validation).unwrap();
    let budget = AllocationBudget::new(1 << 26);
    let artifacts = read(Arc::clone(&original), &budget);
    let witness = artifacts
        .decoded
        .commit_qc
        .attestation_witness
        .as_ref()
        .unwrap();
    assert!(witness.admitted_to(&budget));
    assert_eq!(witness.as_slice(), preimage);
    let original_witness = witness.as_slice().as_ptr();
    let retained = budget.reserved_bytes();
    assert!(retained > witness.as_slice().len());

    let certified =
        norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, 1 << 26, 128),
            || {
                reader.verification_context().verify_certificate_with_scratch_admission(
            current,
            &authority,
            Some(&config),
            Some(artifacts),
            &mut crate::sumeragi::certified_chain::state_certificate::query_scratch_admission,
        )
            },
        )
        .expect("already decoded exact source must not decode the ResultWitness again");
    assert_eq!(certified.verification(), QcVerification::Verified);
    assert!(Arc::ptr_eq(certified.block(), &original));
    assert_eq!(
        certified.commitment().schedule.current.committee.as_ptr(),
        original_committee
    );
    assert_eq!(
        original
            .commit_certificate()
            .unwrap()
            .result_preimage()
            .as_ptr(),
        original_preimage
    );
    let witness = certified
        .commit_qc()
        .unwrap()
        .attestation_witness
        .as_ref()
        .unwrap();
    assert!(witness.admitted_to(&budget));
    assert_eq!(witness.as_slice().as_ptr(), original_witness);
    assert_eq!(witness.as_slice(), preimage);
    assert!(budget.reserved_bytes() > witness.as_slice().len());
    assert!(budget.reserved_bytes() <= retained);
    drop(certified);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_result_witness_rejects_foreign_canonical_bytes_before_borrowing_graph() {
    use crate::sumeragi::certified_chain::{CertifiedChain, VerifiedAuthority, read_frame};
    use crate::sumeragi::schedule;

    let _epoch = crossbeam_epoch::pin();
    let mut chain = CertifiedTestChain::npos_boundary_fixture();
    chain.commit_with(Some(10_000), Vec::new(), Signers::LastThree);
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    let original = frame(&chain, 10);
    let older = frame(&chain, 9);
    let foreign_bytes = older.commit_certificate().unwrap().result_preimage();
    // Genuine canonical bytes from another original execution, not an invented graph.
    crate::sumeragi::commitment::ExecutionResultCommitment::decode(foreign_bytes).unwrap();
    assert_ne!(
        foreign_bytes,
        original.commit_certificate().unwrap().result_preimage()
    );
    let changed = changed_qc(&original, |qc| {
        qc.attestation_witness =
            Some(ResultWitness::from_untrusted(foreign_bytes.to_vec()).unwrap());
    });
    let parent = chain.committed(9);
    let schedule::ScheduledSlot::Ready(scheduled) = &parent.commitment().schedule.next else {
        panic!("authenticated predecessor authorizes H10");
    };
    let mut validation = EpochValidationScope::new();
    let config = scheduled
        .height_config_with_validation(&mut validation)
        .unwrap();
    let authority = VerifiedAuthority::new(scheduled.epoch.clone(), 10, &mut validation).unwrap();
    let budget = AllocationBudget::new(1 << 26);
    let outcome = reader
        .verification_context()
        .verify_certificate_with_scratch_admission(
            read_frame(Arc::clone(&changed), 10).unwrap(),
            &authority,
            Some(&config),
            Some(read(Arc::clone(&changed), &budget)),
            &mut crate::sumeragi::certified_chain::state_certificate::query_scratch_admission,
        );
    assert!(
        matches!(
            outcome,
            Err(
                crate::sumeragi::certified_chain::VerificationReadError::Source(
                    ChainReadError::Certificate {
                        height: 10,
                        error: iroha_sumeragi::crypto::CertError::BadAttestation
                    }
                )
            )
        ),
        "foreign witness cannot authorize the already-decoded original graph"
    );
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "rejection releases the original partial reader owners"
    );
    let receipt = reader
        .verification_context()
        .verify_certificate_with_scratch_admission(
            read_frame(Arc::clone(&original), 10).unwrap(),
            &authority,
            Some(&config),
            Some(read(Arc::clone(&original), &budget)),
            &mut crate::sumeragi::certified_chain::state_certificate::query_scratch_admission,
        )
        .unwrap();
    assert_eq!(receipt.verification(), QcVerification::Verified);
    assert!(Arc::ptr_eq(receipt.block(), &original));
    assert!(
        receipt
            .commit_qc()
            .unwrap()
            .attestation_witness
            .as_ref()
            .unwrap()
            .admitted_to(&budget)
    );
    drop(receipt);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn original_result_witness_and_untrusted_decoder_share_every_native_seal_predicate() {
    use crate::sumeragi::attestation::NativePastaVerifier;
    use crate::sumeragi::certified_chain::{OriginalResultVerifier, read_frame};
    use crate::sumeragi::crypto::core_key;
    use iroha_sumeragi::{
        crypto::AttestationVerifier,
        preimage::{AttestationStatement, att_preimage},
        types::Hash32,
    };

    let mut chain = CertifiedTestChain::npos_boundary_fixture();
    chain.commit_with(Some(10_000), Vec::new(), Signers::LastThree);
    let original = frame(&chain, 10);
    let current = read_frame(Arc::clone(&original), 10).unwrap();
    let standalone = NativePastaVerifier::new(chain.instance(), chain.network_id());
    let borrowed = OriginalResultVerifier {
        source: &current,
        native: standalone,
    };
    let actual: Qc =
        norito::decode_canonical(original.commit_certificate().unwrap().commit_qc()).unwrap();
    let alternative = chain.commit_qc(
        10,
        current.core_hash(),
        current.result(),
        true,
        Signers::Quorum,
    );
    assert_ne!(actual.signers, alternative.signers);
    for qc in [&actual, &alternative] {
        let statement = qc.statement();
        let witness = qc.attestation_witness.as_ref().unwrap();
        let parsed = AttestationStatement::parse(&statement).unwrap();
        for (signer, signature) in qc.signers.ones().zip(&qc.attestations) {
            let key = core_key(
                current.commitment().schedule.current.committee[signer as usize]
                    .validator
                    .public_key(),
            )
            .unwrap();
            let check =
                |height, signer, key, statement: &[u8], witness, signature: &[u8], expected| {
                    assert_eq!(
                        standalone.verify(height, signer, key, statement, witness, signature),
                        expected
                    );
                    assert_eq!(
                        borrowed.verify(height, signer, key, statement, witness, signature),
                        expected
                    );
                };
            check(
                10,
                signer,
                &key,
                &statement,
                witness,
                signature.as_slice(),
                true,
            );
            check(
                11,
                signer,
                &key,
                &statement,
                witness,
                signature.as_slice(),
                false,
            );
            check(
                10,
                signer + 1,
                &key,
                &statement,
                witness,
                signature.as_slice(),
                false,
            );
            let wrong_key = core_key(
                current.commitment().schedule.current.committee[(signer as usize + 1) % 4]
                    .validator
                    .public_key(),
            )
            .unwrap();
            check(
                10,
                signer,
                &wrong_key,
                &statement,
                witness,
                signature.as_slice(),
                false,
            );
            for index in [0, 4, 36, 68, 100] {
                let mut changed = signature.as_slice().to_vec();
                changed[index] ^= 1;
                check(10, signer, &key, &statement, witness, &changed, false);
            }
            let changes: [fn(&mut AttestationStatement); 6] = [
                |source: &mut AttestationStatement| source.instance.0[0] ^= 1,
                |source: &mut AttestationStatement| source.epoch.epoch += 1,
                |source: &mut AttestationStatement| source.epoch.context.0[31] ^= 1,
                |source: &mut AttestationStatement| source.height += 1,
                |source: &mut AttestationStatement| source.block_hash.0[0] ^= 1,
                |source: &mut AttestationStatement| source.result.0[0] ^= 1,
            ];
            for change in changes {
                let mut altered = parsed;
                change(&mut altered);
                let changed = att_preimage(
                    &altered.instance,
                    &altered.epoch,
                    altered.height,
                    &altered.block_hash,
                    &altered.result,
                );
                check(
                    10,
                    signer,
                    &key,
                    &changed,
                    witness,
                    signature.as_slice(),
                    false,
                );
            }
            let mut bytes = witness.as_slice().to_vec();
            let last = bytes.len() - 1;
            bytes[last] ^= 1;
            let altered = ResultWitness::from_untrusted(bytes).unwrap();
            check(
                10,
                signer,
                &key,
                &statement,
                &altered,
                signature.as_slice(),
                false,
            );
            // The standalone decoder still refuses untrusted input under this scope;
            // only the certificate-local capability can borrow its original decoded graph.
            norito::core::with_decode_limits_scope(
                norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, 1 << 26, 128),
                || {
                    assert!(!standalone.verify(
                        10,
                        signer,
                        &key,
                        &statement,
                        witness,
                        signature.as_slice()
                    ));
                    assert!(borrowed.verify(
                        10,
                        signer,
                        &key,
                        &statement,
                        witness,
                        signature.as_slice()
                    ));
                },
            );
            for native in [
                NativePastaVerifier::new(Hash32([0xA7; 32]), chain.network_id()),
                NativePastaVerifier::new(
                    chain.instance(),
                    iroha_data_model::NetworkId::from_genesis_hash(
                        iroha_crypto::HashOf::from_untyped_unchecked(
                            iroha_crypto::Hash::prehashed([0xB7; 32]),
                        ),
                    ),
                ),
            ] {
                let foreign = OriginalResultVerifier {
                    source: &current,
                    native,
                };
                assert!(!native.verify(
                    10,
                    signer,
                    &key,
                    &statement,
                    witness,
                    signature.as_slice()
                ));
                assert!(!foreign.verify(
                    10,
                    signer,
                    &key,
                    &statement,
                    witness,
                    signature.as_slice()
                ));
            }
        }
    }
}
