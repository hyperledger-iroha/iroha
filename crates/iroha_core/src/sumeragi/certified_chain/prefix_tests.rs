//! One-pass native prefix verification and actual successor-bound genesis execution.

use super::*;

#[test]
fn streamed_prefix_emits_genesis_execution_anchor_only_after_real_successor() {
    let (chain, _) = chain();
    let id = ChainId::from("sumeragi-certified-test-chain");
    let mut prefix = CertifiedPrefix::new(&id, chain.network_id(), frame(&chain, 1)).unwrap();
    assert_eq!(prefix.instance(), chain.instance());
    assert!(
        prefix.push(frame(&chain, 3)).is_err(),
        "a skipped frame must not advance the cursor"
    );
    for height in 2..=5 {
        let (current, genesis) = prefix.push(frame(&chain, height)).unwrap().into_parts();
        assert_eq!(current.verification(), QcVerification::Verified);
        assert_eq!(current.height(), height);
        if height == 2 {
            let genesis = genesis.expect("actual H2 authenticates Rg exactly once");
            assert_eq!(genesis.successor(), current.core_hash());
            assert_eq!(genesis.committed().height(), 1);
            assert!(genesis.committed().header().is_none());
            assert!(current.extends(genesis.committed()));
            assert_eq!(
                genesis.into_committed().result(),
                chain.committed(1).result()
            );
        } else {
            assert!(genesis.is_none());
        }
        assert!(
            prefix.push(frame(&chain, height)).is_err(),
            "replay cannot advance twice"
        );
    }
}

#[test]
fn unsigned_changed_genesis_result_cannot_be_exported_by_streamed_reader() {
    let (chain, _) = chain();
    let id = ChainId::from("sumeragi-certified-test-chain");
    let original = frame(&chain, 1);
    let certificate = original.commit_certificate().unwrap();
    let mut result = ExecutionResultCommitment::decode(certificate.result_preimage()).unwrap();
    result.execution.ordinary_writes_root = Hash::new(b"unsigned genesis result replacement");
    let changed = Arc::new(original.as_ref().clone().with_commit_certificate(Some(
        CommitCertificate::from_untrusted_parts(
            Vec::new(),
            Vec::new(),
            result.preimage().unwrap(),
            Vec::new(),
        ),
    )));
    assert_eq!(changed.hash(), original.hash());
    let mut prefix = CertifiedPrefix::new(&id, chain.network_id(), changed).unwrap();
    assert!(matches!(
        prefix.push(frame(&chain, 2)),
        Err(ChainReadError::Discontinuous { height: 2 })
    ));
    let mut foreign = CertifiedPrefix::new(
        &ChainId::from("foreign instance"),
        chain.network_id(),
        original,
    )
    .unwrap();
    assert!(matches!(
        foreign.push(frame(&chain, 2)),
        Err(ChainReadError::WrongInstance { height: 2 })
    ));
}

#[test]
fn streamed_prefix_checks_genuine_pasta_at_retained_empty_epoch_boundary() {
    let mut chain = CertifiedTestChain::npos_boundary_fixture();
    chain.commit_with(Some(10_000), Vec::new(), Signers::LastThree);
    let id = ChainId::from("sumeragi-certified-test-chain");
    let mut prefix = CertifiedPrefix::new(&id, chain.network_id(), frame(&chain, 1)).unwrap();
    for height in 2..10 {
        prefix.push(frame(&chain, height)).unwrap();
    }
    let original = frame(&chain, 10);
    let tampered = with_parts(&original, |_, qc, _| {
        qc.attestation_witness = None;
    });
    assert!(matches!(
        prefix.push(tampered),
        Err(ChainReadError::Certificate { .. })
    ));
    let (boundary, genesis) = prefix.push(original).unwrap().into_parts();
    assert!(genesis.is_none());
    assert!(boundary.header().unwrap().attest);
    assert!(boundary.commitment().schedule.boundary.is_some());
    assert!(boundary.commit_qc().unwrap().attestation_witness.is_some());
}
