// Commit-signature quorum and restoration tests.
//
// Included by `block::commit_signature_tally_tests` to preserve exact libtest names.

#[cfg(feature = "bls")]
#[test]
fn commit_with_signers_accepts_quorum_without_proxy_tail_signature() {
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

    // Sign with leader + validator but omit proxy-tail signature to mirror a QC with trimmed
    // block signatures.
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let mut signatures = BTreeSet::new();
    signatures.insert(BlockSignature::new(
        0,
        checked_block_signature(kp_leader.private_key(), hash),
    ));
    signatures.insert(BlockSignature::new(
        1,
        checked_block_signature(kp_validator.private_key(), hash),
    ));
    signatures.insert(BlockSignature::new(
        3,
        checked_block_signature(kp_set_b.private_key(), hash),
    ));
    let block =
        ValidBlock::new_unverified_for_tests(DataBlockBuilder::new(header).build(signatures));
    let signers = BTreeSet::from([
        ValidatorIndex::try_from(0).expect("validator index parses"),
        ValidatorIndex::try_from(1).expect("validator index parses"),
        ValidatorIndex::try_from(2).expect("validator index parses"),
    ]);

    let result = block
        .commit_with_signers(&topology, &signers, false)
        .unpack(|_| {});
    assert!(
        result.is_ok(),
        "QC quorum should commit even when block signatures are trimmed"
    );
}

#[cfg(feature = "bls")]
#[test]
fn commit_with_signers_allows_block_signer_not_in_qc() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_validator = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_extra_validator = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_validator.public_key().clone()),
        PeerId::new(kp_extra_validator.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    // QC captured votes from leader + validator + proxy; block also carries a signature
    // from a validator that is not part of the QC signer set.
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let hash = header.hash();
    let mut signatures = BTreeSet::new();
    signatures.insert(BlockSignature::new(
        0,
        checked_block_signature(kp_leader.private_key(), hash),
    ));
    signatures.insert(BlockSignature::new(
        1,
        checked_block_signature(kp_validator.private_key(), hash),
    ));
    signatures.insert(BlockSignature::new(
        2,
        checked_block_signature(kp_extra_validator.private_key(), hash),
    ));
    signatures.insert(BlockSignature::new(
        3,
        checked_block_signature(kp_proxy.private_key(), hash),
    ));
    let block =
        ValidBlock::new_unverified_for_tests(DataBlockBuilder::new(header).build(signatures));
    let signers = BTreeSet::from([
        ValidatorIndex::try_from(0).expect("validator index parses"),
        ValidatorIndex::try_from(1).expect("validator index parses"),
        ValidatorIndex::try_from(3).expect("validator index parses"),
    ]);

    let result = block
        .commit_with_signers(&topology, &signers, false)
        .unpack(|_| {});
    assert!(
        result.is_ok(),
        "extra commit-role signatures outside the QC set should not block commit"
    );
}

#[cfg(feature = "bls")]
#[test]
fn replace_signatures_restores_previous_on_failure() {
    let kp_leader = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let kp_proxy = crate::block::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = Topology::new(vec![
        PeerId::new(kp_leader.public_key().clone()),
        PeerId::new(kp_proxy.public_key().clone()),
    ]);

    let mut vb = ValidBlock::new_dummy(kp_leader.private_key());
    vb.sign(&kp_proxy, &topology);
    let original: BTreeSet<_> = vb.as_ref().signatures().cloned().collect();
    let hash = vb.as_ref().hash();
    let mut invalid = BTreeSet::new();
    invalid.insert(BlockSignature::new(
        1,
        checked_block_signature(kp_proxy.private_key(), hash),
    ));

    let result = vb.replace_signatures(invalid, &topology).unpack(|_| {});
    assert!(matches!(
        result,
        Err(SignatureVerificationError::LeaderMissing)
    ));
    let restored: BTreeSet<_> = vb.as_ref().signatures().cloned().collect();
    assert_eq!(restored, original);
}
