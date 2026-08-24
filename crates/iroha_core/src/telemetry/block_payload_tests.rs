#[test]
fn block_payload_detects_transaction_blocks() {
    let block = block_with_transactions(2);
    assert!(block_counts_as_non_empty(&block));
}
#[test]
fn block_payload_flags_genesis_without_transactions() {
    let block = empty_block(1);
    assert!(block_counts_as_non_empty(&block));
}
#[test]
fn block_payload_rejects_non_genesis_empty_block() {
    let block = empty_block(2);
    assert!(!block_counts_as_non_empty(&block));
}
fn checked_block_signature(
    private_key: &PrivateKey,
    header: &iroha_data_model::block::BlockHeader,
) -> SignatureOf<iroha_data_model::block::BlockHeader> {
    SignatureOf::try_new(private_key, header).expect("test block signing should succeed")
}
#[test]
fn block_payload_detects_da_commitment_blocks() {
    let block = block_with_da_commitments(2);
    assert!(block_counts_as_non_empty(&block));
}
#[test]
fn block_payload_detects_npos_consensus_effect_blocks() {
    use iroha_data_model::consensus::{
        NposConsensusEffects, NposMarkVrfPenaltiesAppliedAction, NposPenaltyAction,
    };
    let mut block = empty_block(2);
    block.set_npos_consensus_effects(Some(NposConsensusEffects {
        finalized_global_beacon_pulse: None,
        vrf_epoch_seals: Vec::new(),
        v2_evidence_admissions: Vec::new(),
        penalty_actions: vec![NposPenaltyAction::MarkVrfPenaltiesApplied(
            NposMarkVrfPenaltiesAppliedAction {
                epoch: 0,
                height: 2,
            },
        )],
    }));
    assert!(
        block_counts_as_non_empty(&block),
        "deterministic NPoS state effects are semantic block work"
    );
}
#[test]
fn block_payload_detects_autonomous_lane_payload_carriers() {
    use iroha_data_model::block::{
        AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1, AutonomousLanePayloadEnvelopeV1,
        BlockExecutionContextBundle,
    };
    let mut block = empty_block(6);
    let producer = PeerId::new(
        checked_keypair_with_algorithm(Algorithm::BlsNormal)
            .public_key()
            .clone(),
    );
    let envelope = AutonomousLanePayloadEnvelopeV1 {
        version: AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1,
        network_id: iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"telemetry-autonomous-genesis"),
        )),
        epoch: 4,
        lane_id: LaneId::new(3),
        dataspace_id: DataSpaceId::new(10),
        lane_incarnation: Hash::new(b"telemetry-autonomous-incarnation"),
        proposal_height: 6,
        lane_block_height: 1,
        lane_block_view: 0,
        proposal_hash: Hash::new(b"telemetry-autonomous-proposal"),
        descriptor_hash: Hash::new(b"telemetry-autonomous-descriptor"),
        payload_hash: Hash::new(b"telemetry-autonomous-payload"),
        producer,
        canonical_payload: vec![0x44, 0x50, 0x4E],
    };
    block.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_autonomous_lane_payloads(vec![envelope]),
    ));
    assert!(
        block_counts_as_non_empty(&block),
        "autonomous lane payload carriers are semantic block work"
    );
}
fn empty_block(height: u64) -> iroha_data_model::block::SignedBlock {
    use std::num::NonZeroU64;
    use iroha_data_model::block::{BlockHeader, BlockSignature};
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("height must be > 0"),
        None,
        None,
        None,
        0,
        0,
    );
    let signer = checked_keypair();
    let signature = BlockSignature::new(0, checked_block_signature(signer.private_key(), &header));
    iroha_data_model::block::SignedBlock::presigned(signature, header, Vec::new())
}
fn block_with_da_commitments(height: u64) -> iroha_data_model::block::SignedBlock {
    use std::num::NonZeroU64;
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature},
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme},
            types::{BlobDigest, RetentionPolicy, StorageTicketId},
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("height must be > 0"),
        None,
        None,
        None,
        0,
        0,
    );
    let signer = checked_keypair();
    let signature = BlockSignature::new(0, checked_block_signature(signer.private_key(), &header));
    let mut block = iroha_data_model::block::SignedBlock::presigned(signature, header, Vec::new());
    let record = DaCommitmentRecord::new(
        LaneId::new(0),
        1,
        1,
        BlobDigest::new([0x11; 32]),
        ManifestDigest::new([0x22; 32]),
        DaProofScheme::MerkleSha256,
        Hash::prehashed([0x33; 32]),
        Some(Hash::prehashed([0x55; 32])),
        RetentionPolicy::default(),
        StorageTicketId::new([0x66; 32]),
        Signature::try_from_bytes(&[0x77; 64])
            .expect("checked telemetry DA acknowledgement signature fixture"),
    );
    let bundle = DaCommitmentBundle::new(vec![record]);
    block.set_da_commitments(Some(bundle));
    block
}
fn block_with_transactions(height: u64) -> iroha_data_model::block::SignedBlock {
    use std::num::NonZeroU64;
    use iroha_data_model::{
        block::{BlockHeader, BlockSignature},
        transaction::signed::SignedTransaction,
    };
    fn dummy_transaction() -> SignedTransaction {
        let key_pair = checked_keypair();
        let authority = AccountId::new(key_pair.public_key().clone());
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"telemetry-block-payload-test-network",
            )));
        let transaction = TransactionBuilder::new(
            network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(key_pair.private_key());
        assert_eq!(transaction.network_id(), Some(&network_id));
        transaction
    }
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("height must be > 0"),
        None,
        None,
        None,
        0,
        0,
    );
    let signer = checked_keypair();
    let signature = BlockSignature::new(0, checked_block_signature(signer.private_key(), &header));
    let tx = dummy_transaction();
    iroha_data_model::block::SignedBlock::presigned(signature, header, vec![tx])
}
