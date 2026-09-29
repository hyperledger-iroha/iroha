//! Current four-validator checkpoint and publication authentication regressions.
use super::super::tests::{Fixture, result, sign_qc};
use super::*;
use crate::{
    account::AccountId,
    block::{CommitCertificate, builder::BlockBuilder, output_test_support},
    isi::{Log, sorafs::AssertSorafsPublicationV1},
    level::Level,
    sorafs::{
        pin_registry::{ManifestDigest, ReplicationOrderId},
        publication::{SorafsPublicationProofV1, verify_sorafs_publication_v1},
    },
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionBuilder, error::TransactionRejectionReason,
    },
};
use iroha_crypto::KeyPair;
use iroha_sumeragi::types::Bitmap;
use std::{collections::BTreeSet, num::NonZeroU64};

const CHAIN: &str = "portable-finality-test";

fn extend(
    fixture: &Fixture,
    parent: &SumeragiFinalityProof,
    tx: SignedTransaction,
    succeeds: bool,
) -> SumeragiFinalityProof {
    let parent = parent.decode_checked().unwrap();
    let height = parent.block.header().height().get() + 1;
    let mut builder = BlockBuilder::new(BlockHeader::new(
        NonZeroU64::new(height).unwrap(),
        Some(parent.block.hash()),
        None,
        parent.block.header().creation_time_ms + 1,
        0,
    ));
    builder.push_transaction(tx);
    let mut block = builder.build(BTreeSet::new());
    let output = if succeeds {
        Ok(Default::default())
    } else {
        Err(TransactionRejectionReason::Validation(
            crate::ValidationFail::NotPermitted("publication assertion rejected".into()),
        ))
    };
    output_test_support::install_network(&mut block, vec![output]).unwrap();
    let (crypto, _) = ProofCrypto::new(&fixture.validators).unwrap();
    let result = result(&block, &parent.commitment.schedule.current);
    let payload = block.canonical_resultless_proposal().encode_wire().unwrap();
    let header = CoreHeader {
        instance: fixture.verifier().instance(),
        epoch: core_epoch(&parent.commitment.schedule.current).unwrap().id,
        height,
        origin_view: 0,
        parent_hash: parent.core_hash,
        parent_result: parent.result,
        payload_hash: payload_hash(&crypto, &payload),
        payload_len: payload.len().try_into().unwrap(),
        proposer: 0,
        skipped_leaders: vec![],
        attest: false,
    };
    let mut qc = Qc {
        kind: VoteKind::Commit,
        instance: header.instance,
        epoch: header.epoch,
        height,
        view: 0,
        block_hash: header.hash(&crypto),
        result: result.result().unwrap(),
        attest: false,
        signers: Bitmap::new(4),
        agg_sig: AggregateSignature([0; 96]),
        attestations: vec![],
    };
    sign_qc(&mut qc, &fixture.keys, &[0, 1, 2]);
    block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        norito::encode_canonical(&header).unwrap(),
        norito::encode_canonical(&qc).unwrap(),
        result.preimage().unwrap(),
    )));
    SumeragiFinalityProof {
        block_header: block.header(),
        block_wire: block.encode_wire().unwrap(),
        committee: fixture.validators.clone(),
    }
}

fn transaction(fixture: &Fixture, text: &str) -> SignedTransaction {
    let key = KeyPair::from_seed(vec![42; 32], Algorithm::Ed25519);
    TransactionBuilder::new(
        fixture.network,
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions([Log::new(Level::INFO, text.into())])
    .sign(key.private_key())
}

fn checkpoint(fixture: &Fixture) -> SumeragiFinalityCheckpoint {
    let mut verifier = fixture.verifier();
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&fixture.second).unwrap();
    verifier.export_checkpoint(&fixture.second).unwrap()
}

fn assertion(
    fixture: &Fixture,
    checkpoint: &SumeragiFinalityCheckpoint,
    complete: bool,
    challenge: u8,
) -> SignedTransaction {
    let key = KeyPair::from_seed(vec![43; 32], Algorithm::Ed25519);
    TransactionBuilder::new(
        fixture.network,
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(vec![], None),
    )
    .with_instructions([AssertSorafsPublicationV1 {
        manifest_digest: ManifestDigest([0x31; 32]),
        order_id: ReplicationOrderId([0x32; 32]),
        assignment_revision: 1,
        canonical_order_digest: [0x33; 32],
        require_complete: complete,
        challenge: [challenge; 32],
        minimum_height: checkpoint.height(),
        minimum_block_hash: *checkpoint.block_hash().as_ref(),
    }])
    .sign(key.private_key())
}

#[test]
fn checkpoint_roundtrip_retains_only_three_authenticated_decisions_and_extends() {
    let fixture = Fixture::new();
    let mut verifier = fixture.verifier();
    let mut tip = fixture.first.clone();
    verifier.verify(&tip).unwrap();
    for height in 1..=5 {
        if height > 1 {
            tip = extend(
                &fixture,
                &tip,
                transaction(&fixture, &format!("height {height}")),
                true,
            );
            verifier.verify(&tip).unwrap();
        }
        let checkpoint = verifier.export_checkpoint(&tip).unwrap();
        assert_eq!(checkpoint.decisions.len(), height.min(3));
        assert_eq!(checkpoint.height(), height as u64);
        assert_eq!(checkpoint.block_hash(), tip.block_header.hash());
        assert_eq!(checkpoint.network_id(), fixture.network);
        assert_eq!(checkpoint.chain_id(), CHAIN);
        let bytes = checkpoint.encode_canonical().unwrap();
        let decoded = SumeragiFinalityCheckpoint::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, checkpoint);
        let mut resumed =
            SumeragiFinalityVerifier::from_trusted_checkpoint(&decoded, &fixture.network, CHAIN)
                .unwrap();
        assert_eq!(resumed.decisions.len(), height.min(3));
        let next = extend(&fixture, &tip, transaction(&fixture, "after restart"), true);
        resumed.verify(&next).unwrap();
        assert_eq!(
            resumed.export_checkpoint(&next).unwrap().height(),
            height as u64 + 1
        );
    }
    // The general verifier's existing prefix retention contract stays unchanged.
    assert_eq!(verifier.decisions.len(), 5);
}

#[test]
fn checkpoint_rejects_substituted_roots_schedule_tip_and_noncanonical_material() {
    let fixture = Fixture::new();
    let good = checkpoint(&fixture);
    let foreign =
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
    assert!(SumeragiFinalityVerifier::from_trusted_checkpoint(&good, &foreign, CHAIN).is_err());
    assert!(
        SumeragiFinalityVerifier::from_trusted_checkpoint(&good, &fixture.network, "wrong chain")
            .is_err()
    );
    for mutation in 0..10 {
        let mut bad = good.clone();
        match mutation {
            0 => bad.genesis_wire = fixture.second.block_wire.clone(),
            1 => bad.genesis_committee[0].proof_of_possession[0] ^= 1,
            2 => bad.tip.committee[0].proof_of_possession[0] ^= 1,
            3 => {
                bad.decisions[0].block_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"other genesis"))
            }
            4 => bad.decisions[0].result[0] ^= 1,
            5 => bad.decisions[1].committee_digest[0] ^= 1,
            6 => bad.decisions[1].executed_len += 1,
            7 => bad.decisions[1].height += 1,
            8 => {
                bad.tip = extend(
                    &fixture,
                    &fixture.first,
                    transaction(&fixture, "fork"),
                    true,
                )
            }
            _ => {
                bad.decisions.pop();
            }
        }
        assert!(
            SumeragiFinalityVerifier::from_trusted_checkpoint(&bad, &fixture.network, CHAIN)
                .is_err(),
            "mutation {mutation}"
        );
    }
    let mut trailing = good.encode_canonical().unwrap();
    trailing.push(0);
    assert!(SumeragiFinalityCheckpoint::decode_canonical(&trailing).is_err());
    assert!(SumeragiFinalityCheckpoint::decode_canonical(&[]).is_err());
    let mut oversized = good;
    oversized.chain_id = "x".repeat(1025);
    assert!(oversized.encode_canonical().is_err());
}

#[test]
fn checkpoint_exports_only_verified_tip_and_accepts_alternate_certificate() {
    let fixture = Fixture::new();
    let mut verifier = fixture.verifier();
    assert!(verifier.export_checkpoint(&fixture.first).is_err());
    verifier.verify(&fixture.first).unwrap();
    assert!(verifier.export_checkpoint(&fixture.second).is_err());
    verifier.verify(&fixture.second).unwrap();
    assert!(verifier.export_checkpoint(&fixture.first).is_err());
    let checkpoint = verifier.export_checkpoint(&fixture.alternate()).unwrap();
    let resumed =
        SumeragiFinalityVerifier::from_trusted_checkpoint(&checkpoint, &fixture.network, CHAIN)
            .unwrap();
    resumed
        .verify_same_decision(checkpoint.tip(), &fixture.second)
        .unwrap();
}

#[test]
fn checkpoint_lag_two_binding_and_height_exhaustion_refuse() {
    let fixture = Fixture::new();
    let mut verifier = fixture.verifier();
    verifier.verify(&fixture.first).unwrap();
    verifier.verify(&fixture.second).unwrap();
    let third = extend(
        &fixture,
        &fixture.second,
        transaction(&fixture, "third"),
        true,
    );
    verifier.verify(&third).unwrap();
    let mut checkpoint = verifier.export_checkpoint(&third).unwrap();
    checkpoint.decisions[0].schedule.current.leader_seed[0] ^= 1;
    assert!(
        SumeragiFinalityVerifier::from_trusted_checkpoint(&checkpoint, &fixture.network, CHAIN)
            .is_err()
    );
    let decision = verifier.decisions.get(&3).unwrap().clone();
    verifier.decisions.insert(u64::MAX, decision);
    assert!(
        verifier
            .verify(&third)
            .unwrap_err()
            .0
            .contains("height exhausted")
    );
    checkpoint.tip.block_header.height = NonZeroU64::new(u64::MAX).unwrap();
    assert!(checkpoint.encode_canonical().is_err());
}

#[test]
fn publication_proof_authenticates_both_phases_and_canonical_current_wire() {
    let fixture = Fixture::new();
    let checkpoint = checkpoint(&fixture);
    for complete in [false, true] {
        let tx = assertion(&fixture, &checkpoint, complete, 1);
        let tip = extend(&fixture, &fixture.second, tx.clone(), true);
        let proof = SorafsPublicationProofV1 {
            lineage: vec![fixture.alternate(), tip.clone()],
        };
        let bytes = norito::encode_canonical(&proof).unwrap();
        let decoded: SorafsPublicationProofV1 = norito::decode_canonical(&bytes).unwrap();
        assert_eq!(decoded, proof);
        let verified =
            verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &decoded).unwrap();
        assert_eq!(verified.completed(), complete);
        assert_eq!(verified.finality().tip(), &tip);
        assert_eq!(tip.committee.len(), 4);
        let block = tip.decode_checked().unwrap();
        let qc: Qc =
            norito::decode_canonical(block.block.commit_certificate().unwrap().commit_qc())
                .unwrap();
        assert_eq!(qc.signers.count_ones(), 3);
        assert!(
            SumeragiFinalityVerifier::from_trusted_checkpoint(
                verified.finality(),
                &fixture.network,
                CHAIN
            )
            .is_ok()
        );
    }
}

#[test]
fn publication_proof_rejects_signed_failure_replay_phase_and_output_substitution() {
    let fixture = Fixture::new();
    let checkpoint = checkpoint(&fixture);
    let tx = assertion(&fixture, &checkpoint, false, 1);
    let tip = extend(&fixture, &fixture.second, tx.clone(), true);
    let valid = SorafsPublicationProofV1 {
        lineage: vec![fixture.second.clone(), tip],
    };
    for expected in [
        assertion(&fixture, &checkpoint, false, 2),
        assertion(&fixture, &checkpoint, true, 1),
        assertion(&fixture, &checkpoint, false, 0),
        transaction(&fixture, "not an assertion"),
    ] {
        assert!(
            verify_sorafs_publication_v1(&fixture.network, &checkpoint, &expected, &valid).is_err()
        );
    }
    let failed_tip = extend(&fixture, &fixture.second, tx.clone(), false);
    failed_tip.decode_checked().unwrap();
    let failed = SorafsPublicationProofV1 {
        lineage: vec![fixture.second.clone(), failed_tip.clone()],
    };
    assert!(verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &failed).is_err());
    let mut substituted = valid.clone();
    let mut failed_block = failed_tip.decode_checked().unwrap().block;
    let valid_certificate = valid.lineage[1]
        .decode_checked()
        .unwrap()
        .block
        .commit_certificate()
        .unwrap()
        .clone();
    failed_block.set_commit_certificate(Some(valid_certificate));
    substituted.lineage[1].block_wire = failed_block.encode_wire().unwrap();
    assert!(
        verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &substituted).is_err()
    );
    let mut corrupt = valid.clone();
    let mut block = corrupt.lineage[1].decode_checked().unwrap().block;
    let certificate = block.commit_certificate().unwrap();
    let consensus_header = certificate.consensus_header().to_vec();
    let result_preimage = certificate.result_preimage().to_vec();
    let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
    qc.agg_sig.0[0] ^= 1;
    block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
        consensus_header,
        norito::encode_canonical(&qc).unwrap(),
        result_preimage,
    )));
    corrupt.lineage[1].block_wire = block.encode_wire().unwrap();
    assert!(verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &corrupt).is_err());
    let mut truncated = valid.clone();
    truncated.lineage.remove(0);
    assert!(verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &truncated).is_err());
    let replayed =
        verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &valid).unwrap();
    assert!(
        verify_sorafs_publication_v1(&fixture.network, replayed.finality(), &tx, &valid).is_err()
    );
}

#[test]
fn publication_proof_rejects_independently_valid_fork_gap_and_unbound_floor() {
    let fixture = Fixture::new();
    let checkpoint = checkpoint(&fixture);
    let tx = assertion(&fixture, &checkpoint, true, 3);
    let fork = extend(
        &fixture,
        &fixture.first,
        transaction(&fixture, "independently valid fork"),
        true,
    );
    let mut foreign = fixture.verifier();
    foreign.verify(&fixture.first).unwrap();
    foreign.verify(&fork).unwrap();
    let foreign_checkpoint = foreign.export_checkpoint(&fork).unwrap();
    let proof = SorafsPublicationProofV1 {
        lineage: vec![
            fixture.second.clone(),
            extend(&fixture, &fixture.second, tx.clone(), true),
        ],
    };
    assert!(
        verify_sorafs_publication_v1(&fixture.network, &foreign_checkpoint, &tx, &proof).is_err()
    );
    let wrong_floor = assertion(&fixture, &foreign_checkpoint, true, 3);
    let wrong_floor_proof = SorafsPublicationProofV1 {
        lineage: vec![
            fixture.second.clone(),
            extend(&fixture, &fixture.second, wrong_floor.clone(), true),
        ],
    };
    assert!(
        verify_sorafs_publication_v1(
            &fixture.network,
            &checkpoint,
            &wrong_floor,
            &wrong_floor_proof
        )
        .is_err()
    );
    let fourth = extend(&fixture, &proof.lineage[1], tx.clone(), true);
    let gap = SorafsPublicationProofV1 {
        lineage: vec![fixture.second.clone(), fourth],
    };
    assert!(verify_sorafs_publication_v1(&fixture.network, &checkpoint, &tx, &gap).is_err());
}
