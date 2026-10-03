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
    // Preserve the authenticated lane-write opening's structural consistency. This
    // attack changes a well-formed execution result which only the successor can bind.
    result.execution.world_state_root = Hash::new(b"unsigned genesis result replacement");
    let changed = crate::block::reserve_block_for_tests().initialize(
        original.as_ref().clone().with_commit_certificate(Some(
            CommitCertificate::from_untrusted_parts(
                Vec::new(),
                Vec::new(),
                result.preimage().unwrap(),
                Vec::new(),
            ),
        )),
    );
    assert_eq!(changed.hash(), original.hash());
    let mut prefix = CertifiedPrefix::new(&id, chain.network_id(), changed).unwrap();
    assert!(matches!(
        prefix.push(frame(&chain, 2)),
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            ChainReadError::Discontinuous { height: 2 }
        ))
    ));
    let mut foreign = CertifiedPrefix::new(
        &ChainId::from("foreign-instance"),
        chain.network_id(),
        original,
    )
    .unwrap();
    assert!(matches!(
        foreign.push(frame(&chain, 2)),
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            ChainReadError::WrongInstance { height: 2 }
        ))
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
        Err(crate::execution_attempt::ExecutionAttemptError::Rejected(
            ChainReadError::Certificate { .. }
        ))
    ));
    let (boundary, genesis) = prefix.push(original).unwrap().into_parts();
    assert!(genesis.is_none());
    assert!(boundary.header().unwrap().attest);
    assert!(boundary.commitment().schedule.boundary.is_some());
    assert!(boundary.commit_qc().unwrap().attestation_witness.is_some());
}

/// Shape reuse binds the complete context, while each subsequent QC remains independently
/// mandatory. Failed shape or signature checks never advance the streamed cursor.
#[test]
fn warmed_epoch_shape_rejects_substituted_context_and_still_checks_each_qc() {
    let (chain, _) = chain();
    let id = ChainId::from("sumeragi-certified-test-chain");
    let mut prefix = CertifiedPrefix::new(&id, chain.network_id(), frame(&chain, 1)).unwrap();
    prefix.push(frame(&chain, 2)).unwrap();
    let original = frame(&chain, 3);
    for mutate_keys in [false, true] {
        let changed = with_parts(&original, |_, _, preimage| {
            let mut commitment = ExecutionResultCommitment::decode(preimage).unwrap();
            if mutate_keys {
                commitment.schedule.current.authority.validators[0].eq_proof_public_key = [0; 32];
            } else {
                commitment.schedule.current.leader_seed = [0; 32];
            }
            // Keep the compact wire's equality requirement intact, so this is a real decoded
            // same-epoch-number substitution rather than a serialization refusal.
            let epoch = commitment.schedule.current.clone();
            for slot in [
                &mut commitment.schedule.next,
                &mut commitment.schedule.after_next,
            ] {
                let schedule::ScheduledSlot::Ready(config) = slot else {
                    panic!("permissioned fixture has ready slots");
                };
                config.epoch = epoch.clone();
            }
            *preimage = commitment.preimage().unwrap();
        });
        assert!(matches!(
            prefix.push(changed),
            Err(ExecutionAttemptError::Rejected(ChainReadError::Malformed {
                height: 3,
                ..
            }))
        ));
        assert_eq!(prefix.prefix.tip.height(), 2);
    }
    let forged = with_parts(&original, |_, qc, _| qc.agg_sig.0[5] ^= 1);
    assert!(matches!(
        prefix.push(forged),
        Err(ExecutionAttemptError::Rejected(
            ChainReadError::Certificate { height: 3, .. }
        ))
    ));
    assert_eq!(prefix.prefix.tip.height(), 2);
    prefix
        .push(original)
        .expect("unchanged original height can still verify");
    prefix.push(frame(&chain, 4)).unwrap();
}

/// Reusing immutable epoch shape does not reuse any positive durable-certificate verdict,
/// either when this reader restarts its prefix or when a fresh view creates another reader.
#[test]
fn warmed_reader_rechecks_durable_prefix_and_fresh_view_after_body_removal() {
    let (chain, _) = chain();
    let view = chain.state().view();
    let reader = CertifiedChain::new(&view).unwrap();
    reader.certified(2).unwrap();
    let original = frame(&chain, 3);
    let forged = with_parts(&original, |_, qc, _| qc.agg_sig.0[5] ^= 1);
    assert!(matches!(
        reader.check_certificate(forged, 3),
        Err(ExecutionAttemptError::Rejected(
            ChainReadError::Certificate { height: 3, .. }
        ))
    ));
    reader.certified(3).unwrap();
    chain
        .kura()
        .corrupt_canonical_body_for_testing(NonZeroUsize::new(2).unwrap())
        .unwrap();
    assert!(reader.certified(3).is_err());
    let fresh = chain.state().view();
    assert!(CertifiedChain::new(&fresh).unwrap().certified(3).is_err());
}

/// A standalone structural read always owns a fresh scope and rejects the same malformed
/// original bytes as a warmed scope; neither form establishes finality by itself.
#[test]
fn standalone_and_scoped_frame_reads_agree_without_skipping_shape_checks() {
    let (chain, _) = chain();
    let original = frame(&chain, 3);
    let mut validation = EpochValidationScope::new();
    read_frame_with_validation(frame(&chain, 2), 2, &mut validation).unwrap();
    let fresh = read_frame(original.clone(), 3).unwrap();
    let reused = read_frame_with_validation(original.clone(), 3, &mut validation).unwrap();
    assert_eq!(fresh.commitment(), reused.commitment());
    assert_eq!(fresh.id(), reused.id());
    let changed = with_parts(&original, |_, _, preimage| {
        let mut commitment = ExecutionResultCommitment::decode(preimage).unwrap();
        commitment.schedule.height += 1;
        *preimage = commitment.preimage().unwrap();
    });
    let fresh_error = read_frame(changed.clone(), 3).unwrap_err();
    let reused_error = read_frame_with_validation(changed, 3, &mut validation).unwrap_err();
    assert_eq!(fresh_error, reused_error);
    assert!(matches!(
        fresh_error,
        ExecutionAttemptError::Rejected(ChainReadError::Malformed { height: 3, .. })
    ));
}
