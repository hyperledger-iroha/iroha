//! Publication evidence tests with exact signed executions and a real four-validator RS16 lineage.

use super::*;
use iroha_data_model::{
    isi::sorafs::AssertSorafsPublicationV1,
    sorafs::{
        pin_registry::{ManifestDigest, ReplicationOrderId},
        publication::{SorafsPublicationProofV1, verify_sorafs_publication_v1},
    },
    transaction::{FeePaymentIntent, SignedTransaction, error::TransactionRejectionReason},
};

fn publication_block(
    parent: Option<&SccpFinalizedBlockTestFixtureV1>,
    transaction: Option<SignedTransaction>,
    succeeds: bool,
) -> SignedBlock {
    let transactions = transaction.into_iter().collect::<Vec<_>>();
    let key = KeyPair::try_from_seed(vec![0x74; 32], Algorithm::Ed25519).unwrap();
    let header = BlockHeader::new(
        NonZeroU64::new(parent.map_or(1, |parent| parent.block().header().height().get() + 1))
            .unwrap(),
        parent.map(|parent| parent.block().hash()),
        MerkleTree::root_from_typed_leaves(
            transactions
                .iter()
                .map(SignedTransaction::hash_as_entrypoint),
        ),
        1_700_000_000_002,
        0,
    );
    let signature = BlockSignature::new(
        0,
        SignatureOf::try_from_hash(key.private_key(), header.hash()).unwrap(),
    );
    let mut block = SignedBlock::presigned(signature, header, transactions);
    let outputs = (0..block.network_entrypoint_count())
        .map(|index| {
            ExecutionOutputV1::Network(NetworkExecutionOutputV1 {
                input_index: u32::try_from(index).unwrap(),
                result: TransactionResult::new(if succeeds {
                    Ok(DataTriggerSequence::default())
                } else {
                    Err(TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::NotPermitted(
                            "publication assertion rejected".to_owned(),
                        ),
                    ))
                }),
                completions: Vec::new(),
            })
        })
        .collect::<Vec<_>>();
    let accepted = if succeeds { outputs.len() as u64 } else { 0 };
    block
        .set_execution_outputs(
            outputs,
            accepted,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &exact_fixture_output_limits(),
        )
        .unwrap();
    block
        .replace_signatures(
            [BlockSignature::new(
                0,
                SignatureOf::try_from_hash(key.private_key(), block.hash()).unwrap(),
            )]
            .into_iter()
            .collect(),
        )
        .unwrap();
    block
}

fn assertion(checkpoint: &V2FinalityArtifact, complete: bool, challenge: u8) -> SignedTransaction {
    let key = KeyPair::try_from_seed(vec![0x75; 32], Algorithm::Ed25519).unwrap();
    let mut builder = TransactionBuilder::new(
        sccp_taira_finality_network_id_v1(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(1_700_000_000_001));
    builder
        .with_instructions([AssertSorafsPublicationV1 {
            manifest_digest: ManifestDigest([0x31; 32]),
            order_id: ReplicationOrderId([0x32; 32]),
            assignment_revision: 1,
            canonical_order_digest: [0x33; 32],
            require_complete: complete,
            challenge: [challenge; 32],
            minimum_height: checkpoint.height,
            minimum_block_hash: *checkpoint.block_hash.as_ref(),
        }])
        .sign(key.private_key())
}

fn evidence(
    parent: &SccpFinalizedBlockTestFixtureV1,
    transaction: SignedTransaction,
    succeeds: bool,
) -> SorafsPublicationProofV1 {
    let block = publication_block(Some(parent), Some(transaction), succeeds);
    let child = sccp_finalize_taira_block_test_fixture_v1(&block, Some(parent));
    SorafsPublicationProofV1 {
        lineage: vec![
            parent.proof().finality_artifact.clone(),
            child.proof().finality_artifact.clone(),
        ],
        executed_block: block.encode_wire().unwrap(),
    }
}

#[test]
fn publication_proof_authenticates_both_phases_and_canonical_wire() {
    let parent =
        sccp_finalize_taira_block_test_fixture_v1(&publication_block(None, None, true), None);
    let checkpoint = &parent.proof().finality_artifact;
    for complete in [false, true] {
        let transaction = assertion(checkpoint, complete, 1);
        let proof = evidence(&parent, transaction.clone(), true);
        let bytes = norito::to_bytes(&proof).unwrap();
        let decoded: SorafsPublicationProofV1 = norito::decode_from_bytes(&bytes).unwrap();
        assert_eq!(decoded, proof);
        let verified = verify_sorafs_publication_v1(
            &sccp_taira_finality_network_id_v1(),
            checkpoint,
            &transaction,
            &decoded,
        )
        .unwrap();
        assert_eq!(verified.completed(), complete);
        assert_eq!(verified.finality(), proof.lineage.last().unwrap());
        assert_eq!(verified.finality().height_context.roster.len(), 4);
        assert_eq!(verified.finality().commit_qc.signers.len(), 3);
        assert_eq!(
            verified.finality().height_context.da_layout.encoding,
            PayloadEncoding::ReedSolomon16
        );
    }
}

#[test]
fn publication_proof_rejects_signed_failure_replay_and_phase_substitution() {
    let parent =
        sccp_finalize_taira_block_test_fixture_v1(&publication_block(None, None, true), None);
    let checkpoint = &parent.proof().finality_artifact;
    let transaction = assertion(checkpoint, false, 1);
    let valid = evidence(&parent, transaction.clone(), true);
    let network = sccp_taira_finality_network_id_v1();
    for expected in [
        assertion(checkpoint, false, 2),
        assertion(checkpoint, true, 1),
        assertion(checkpoint, false, 0),
    ] {
        assert!(verify_sorafs_publication_v1(&network, checkpoint, &expected, &valid).is_err());
    }
    let failed = evidence(&parent, transaction.clone(), false);
    failed.lineage.last().unwrap().verify().unwrap();
    assert!(verify_sorafs_publication_v1(&network, checkpoint, &transaction, &failed).is_err());
    let mut substituted = valid.clone();
    substituted.executed_block = failed.executed_block;
    assert!(
        verify_sorafs_publication_v1(&network, checkpoint, &transaction, &substituted).is_err()
    );
    let mut corrupt = valid.clone();
    corrupt.lineage[1].commit_qc.aggregate_signature[0] ^= 1;
    assert!(verify_sorafs_publication_v1(&network, checkpoint, &transaction, &corrupt).is_err());
    let mut truncated = valid;
    truncated.lineage.remove(0);
    assert!(verify_sorafs_publication_v1(&network, checkpoint, &transaction, &truncated).is_err());
}

#[test]
fn publication_proof_rejects_an_independently_valid_fork_and_unbound_floor() {
    let parent =
        sccp_finalize_taira_block_test_fixture_v1(&publication_block(None, None, true), None);
    let checkpoint = &parent.proof().finality_artifact;
    let foreign = sccp_finalize_taira_epoch_boundary_test_fixture_v1(&publication_block(
        None,
        Some(assertion(checkpoint, false, 9)),
        true,
    ));
    let transaction = assertion(checkpoint, true, 3);
    let proof = evidence(&parent, transaction.clone(), true);
    let network = sccp_taira_finality_network_id_v1();
    foreign.proof().finality_artifact.verify().unwrap();
    assert!(
        verify_sorafs_publication_v1(
            &network,
            &foreign.proof().finality_artifact,
            &transaction,
            &proof
        )
        .is_err()
    );
    let wrong_floor = assertion(&foreign.proof().finality_artifact, true, 3);
    let wrong_floor_proof = evidence(&parent, wrong_floor.clone(), true);
    assert!(
        verify_sorafs_publication_v1(&network, checkpoint, &wrong_floor, &wrong_floor_proof)
            .is_err()
    );
}
