//! Actual BLS evidence against original genesis execution and Worker publications.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};

fn chain() -> CertifiedTestChain {
    CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap()
}

fn conflict(chain: &CertifiedTestChain, height: u64) -> Evidence {
    Evidence::ConflictingCertificates(
        chain.commit_qc(
            height,
            Hash32([0x31; 32]),
            Hash32([0x32; 32]),
            false,
            Signers::Quorum,
        ),
        chain.commit_qc(
            height,
            Hash32([0x33; 32]),
            Hash32([0x34; 32]),
            false,
            Signers::LastThree,
        ),
    )
}

#[test]
fn original_genesis_authenticates_current_height_report_and_exact_signer_intersection() {
    let chain = chain();
    let evidence = conflict(&chain, 2);
    let view = chain.state().view();
    let mut reads = Vec::new();
    let verified = verify_from_state(&view, &evidence, |count, bytes| {
        reads.push((count, bytes));
        Ok(())
    })
    .unwrap();
    assert_eq!(verified.tip(), view.native_execution_tip().unwrap());
    assert_eq!(verified.instance(), chain.instance());
    assert_eq!(verified.height(), 2);
    assert!(verified.safety_violation());
    assert_eq!(
        verified
            .offenders()
            .iter()
            .map(|offender| offender.signer)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    for offender in verified.offenders() {
        assert_eq!(
            &offender.peer_id,
            &chain.validators()[offender.signer as usize].0
        );
        assert!(offender.lane_stake.is_none());
    }
    let config = view
        .world()
        .consensus_schedule()
        .ready(2)
        .unwrap()
        .height_config()
        .unwrap();
    assert_eq!(verified.epoch(), config.epoch.id);
    assert_eq!(
        verified.authority_generation(),
        config.epoch.authority_generation
    );
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].0, 1);
    assert!(reads[0].1 > 0);
}

#[test]
fn old_subject_uses_original_history_after_schedule_window_advances() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let evidence = conflict(&chain, 2);
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    assert!(view.world().consensus_schedule().ready(2).is_err());
    let mut reads = 0;
    let verified = verify_from_state(&view, &evidence, |count, _| {
        reads += count;
        Ok(())
    })
    .unwrap();
    assert_eq!(verified.height(), 2);
    assert_eq!(verified.tip().height(), 4);
    assert_eq!(
        reads, 4,
        "one reverse walk includes the tip and original genesis"
    );
}

#[test]
fn interval_traversal_charges_each_source_once_and_preserves_descending_order() {
    let mut chain = chain();
    for _ in 0..3 {
        chain.commit(Vec::new());
    }
    let view = chain.state().view();
    let mut reads = 0;
    let mut selected = Vec::new();
    view.canonical_history()
        .visit_executed_backwards(
            NonZeroUsize::new(2).unwrap(),
            NonZeroUsize::new(3).unwrap(),
            |count, _| {
                reads += count;
                Ok(())
            },
            |receipt| {
                selected.push(receipt.height());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(reads, 3);
    assert_eq!(selected, [3, 2]);
    assert!(
        view.canonical_history()
            .visit_executed_backwards(
                NonZeroUsize::new(3).unwrap(),
                NonZeroUsize::new(2).unwrap(),
                |_, _| panic!("reversed interval must not read"),
                |_| Ok(()),
            )
            .is_err()
    );
}

#[test]
fn foreign_instance_and_epoch_cannot_borrow_an_honest_historical_committee() {
    let chain = chain();
    let mut evidence = conflict(&chain, 2);
    let Evidence::ConflictingCertificates(first, _) = &mut evidence else {
        unreachable!()
    };
    first.instance.0[0] ^= 1;
    assert!(matches!(
        verify_from_state(&chain.state().view(), &evidence, |_, _| panic!(
            "foreign instance must not read"
        )),
        Err(NativeEvidenceError::Context(_))
    ));
    let mut evidence = conflict(&chain, 2);
    let Evidence::ConflictingCertificates(first, _) = &mut evidence else {
        unreachable!()
    };
    first.epoch.context.0[0] ^= 1;
    assert!(matches!(
        verify_from_state(&chain.state().view(), &evidence, |_, _| Ok(())),
        Err(NativeEvidenceError::Proof(_))
    ));
    let mut foreign = TestChainConfig::new(World::new(), 1_000);
    foreign.chain_id = "different-native-evidence-chain".into();
    let foreign = CertifiedTestChain::start(foreign).unwrap();
    assert!(matches!(
        verify_from_state(
            &foreign.state().view(),
            &conflict(&chain, 2),
            |_, _| panic!("foreign configured chain must not read")
        ),
        Err(NativeEvidenceError::Context(_))
    ));
}

#[test]
fn local_history_refusal_and_corrupt_original_tip_never_become_attribution() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let evidence = conflict(&chain, 2);
    let mut reads = 0;
    assert!(matches!(
        verify_from_state(&chain.state().view(), &evidence, |count, _| {
            reads += count;
            Err(crate::execution_attempt::ExecutionAttemptError::Deferred(ivm::error::ExecutionDeferral::CanonicalHistoryCapacity.into()))
        }),
        Err(NativeEvidenceError::History(
            crate::execution_attempt::ExecutionAttemptError::Deferred(local)
        )) if local.reason() == ivm::error::ExecutionDeferral::CanonicalHistoryCapacity
    ));
    assert_eq!(reads, 1);
    chain
        .kura()
        .corrupt_native_frame_for_test(NonZeroUsize::new(2).unwrap());
    let mut reads = 0;
    assert!(matches!(
        verify_from_state(&chain.state().view(), &evidence, |count, _| {
            reads += count;
            Ok(())
        }),
        Err(NativeEvidenceError::History(_))
    ));
    assert_eq!(reads, 1, "corruption stops at the first actual source");
}

#[test]
fn below_quorum_and_identical_certificate_values_are_rejected() {
    let chain = chain();
    let mut evidence = conflict(&chain, 2);
    let Evidence::ConflictingCertificates(first, _) = &mut evidence else {
        unreachable!()
    };
    *first = chain.commit_qc(
        2,
        Hash32([0x31; 32]),
        Hash32([0x32; 32]),
        false,
        Signers::BelowQuorum,
    );
    assert!(matches!(
        verify_from_state(&chain.state().view(), &evidence, |_, _| Ok(())),
        Err(NativeEvidenceError::Proof(_))
    ));
    let mut evidence = conflict(&chain, 2);
    let Evidence::ConflictingCertificates(first, second) = &mut evidence else {
        unreachable!()
    };
    *second = first.clone();
    assert!(matches!(
        verify_from_state(&chain.state().view(), &evidence, |_, _| Ok(())),
        Err(NativeEvidenceError::Proof(EvidenceError::NotConflicting))
    ));
}
