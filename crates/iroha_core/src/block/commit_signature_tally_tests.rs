//! Commit-signature tally, quorum, and admission regression tests.

use std::collections::BTreeSet;

use iroha_crypto::{Algorithm, SignatureOf};
use iroha_data_model::block::builder::BlockBuilder as DataBlockBuilder;
use nonzero_ext::nonzero;

use super::*;
use crate::{
    block::valid::commit_signature_tally,
    sumeragi::{consensus::ValidatorIndex, network_topology::Topology},
};

fn checked_block_signature(
    private_key: &iroha_crypto::PrivateKey,
    block_hash: HashOf<BlockHeader>,
) -> SignatureOf<BlockHeader> {
    SignatureOf::try_from_hash(private_key, block_hash).expect("test block signing should succeed")
}

#[cfg(feature = "bls")]
#[test]
fn commit_signature_tally_dedups_and_counts_set_b() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_validator = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_set_b = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_validator.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
        PeerId::new(kp_set_b.public_key().clone()),
    ]);

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let signatures = BTreeSet::from([
        BlockSignature::new(0, checked_block_signature(kp_leader.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_validator.private_key(), hash)),
        BlockSignature::new(2, checked_block_signature(kp_proxy.private_key(), hash)),
        BlockSignature::new(3, checked_block_signature(kp_set_b.private_key(), hash)),
    ]);
    let block = DataBlockBuilder::new(header).build(signatures);

    let tally = commit_signature_tally(&block, &topology);
    assert_eq!(tally.present, 4);
    assert_eq!(tally.counted, 4);
    assert_eq!(tally.set_b_signatures, 1);
}

#[cfg(feature = "bls")]
#[test]
fn is_commit_rejects_duplicate_signer_index() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_dup = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let signatures = BTreeSet::from([
        BlockSignature::new(0, checked_block_signature(kp_leader.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_proxy.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_dup.private_key(), hash)),
    ]);
    let block = DataBlockBuilder::new(header).build(signatures);

    let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
    assert!(matches!(
        err,
        SignatureVerificationError::DuplicateSignature { signer } if signer == 1
    ));
}

#[cfg(feature = "bls")]
#[test]
fn is_commit_rejects_proxy_tail_spoof() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_spoof = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let signatures = BTreeSet::from([
        BlockSignature::new(0, checked_block_signature(kp_leader.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_spoof.private_key(), hash)),
    ]);
    let block = DataBlockBuilder::new(header).build(signatures);

    let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
    assert!(
        matches!(err, SignatureVerificationError::UnknownSignature),
        "expected proxy tail spoof rejection, got {err:?}"
    );
}

#[cfg(feature = "bls")]
#[test]
fn is_commit_rejects_leader_spoof() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_spoof = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let signatures = BTreeSet::from([
        BlockSignature::new(0, checked_block_signature(kp_spoof.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_proxy.private_key(), hash)),
    ]);
    let block = DataBlockBuilder::new(header).build(signatures);

    let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
    assert!(matches!(err, SignatureVerificationError::UnknownSignature));
}

#[cfg(feature = "bls")]
#[test]
fn is_commit_rejects_set_b_spoof() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_validator = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_set_b = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_spoof = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_validator.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
        PeerId::new(kp_set_b.public_key().clone()),
    ]);

    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let signatures = BTreeSet::from([
        BlockSignature::new(0, checked_block_signature(kp_leader.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_validator.private_key(), hash)),
        BlockSignature::new(2, checked_block_signature(kp_proxy.private_key(), hash)),
        BlockSignature::new(3, checked_block_signature(kp_spoof.private_key(), hash)),
    ]);
    let block = DataBlockBuilder::new(header).build(signatures);

    let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
    assert!(matches!(err, SignatureVerificationError::UnknownSignature));
}

#[cfg(feature = "bls")]
#[test]
fn commit_with_signers_rejects_invalid_block_signature() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    // Corrupt the leader signature so the block signatures are no longer trustworthy.
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let signatures = BTreeSet::from([
        BlockSignature::new(0, checked_block_signature(kp_proxy.private_key(), hash)),
        BlockSignature::new(1, checked_block_signature(kp_proxy.private_key(), hash)),
    ]);
    let block =
        ValidBlock::new_unverified_for_tests(DataBlockBuilder::new(header).build(signatures));
    let signers = BTreeSet::from([
        ValidatorIndex::try_from(0).expect("validator index parses"),
        ValidatorIndex::try_from(1).expect("validator index parses"),
    ]);

    let result = block
        .commit_with_signers(&topology, &signers, false)
        .unpack(|_| {});
    assert!(
        result.is_err(),
        "invalid block signatures must still be rejected even when a QC signer set is present"
    );
}

#[cfg(feature = "bls")]
#[test]
fn commit_with_signers_succeeds_with_quorum_and_signatures() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    let mut block = ValidBlock::new_dummy(kp_leader.private_key());
    block.sign(&kp_proxy, &topology);
    let signers = BTreeSet::from([
        ValidatorIndex::try_from(0).expect("validator index parses"),
        ValidatorIndex::try_from(1).expect("validator index parses"),
    ]);

    let result = block
        .commit_with_signers(&topology, &signers, false)
        .unpack(|_| {});
    assert!(
        result.is_ok(),
        "quorum signatures should commit via QC signer set"
    );
}

// Tail quorum and signature-restoration tests retain their stable libtest paths.
include!("commit_signature_tail_tests.rs");
