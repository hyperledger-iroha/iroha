//! Exact retained root replay semantics and original-pool admission descriptors.
use super::*;

#[test]
fn admission_owner_preserves_root_authority_and_refunds_exact_original_backing() {
    let _epoch = crossbeam_epoch::pin();
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let proof = Evidence::from_native(&super::super::tests::conflict(&chain, 2)).unwrap();
    let state = chain.state();
    let generation = state.state_view_generation();
    let budget = state.evidence_preparation_budget();
    let baseline = budget.reserved_bytes();
    budget.set_limit_bytes(baseline);
    assert!(matches!(
        AdmissionRead::capture(
            state,
            &state.view(),
            generation,
            3,
            std::slice::from_ref(&proof)
        ),
        Err(EvidenceAdmissionError::Preparation(_))
    ));
    assert_eq!(budget.reserved_bytes(), baseline);
    budget.set_limit_bytes(1 << 30);
    let mut read = AdmissionRead::capture(
        state,
        &state.view(),
        generation,
        3,
        std::slice::from_ref(&proof),
    )
    .unwrap();
    assert!(read.matches(generation, 3, std::slice::from_ref(&proof)));
    assert!(!read.matches(generation + 2, 3, std::slice::from_ref(&proof)));
    assert!(!read.matches(generation, 4, std::slice::from_ref(&proof)));
    assert!(!read.matches(generation, 3, &[]));
    let captured = budget.reserved_bytes();
    assert!(captured > baseline);
    read.complete().unwrap();
    let admitted = read.finish().unwrap();
    assert!(admitted.belongs_to(budget));
    assert_eq!(admitted.as_slice()[0].key(), evidence_key(&proof));
    assert_eq!(
        admitted.as_slice()[0].attribution.scope,
        iroha_data_model::block::consensus::EvidenceScope::Root
    );
    assert_eq!(admitted.as_slice()[0].attribution.height, 2);
    assert_eq!(
        admitted.as_slice()[0]
            .attribution
            .offenders
            .iter()
            .map(|offender| offender.signer)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        admitted.as_slice()[0]
            .attribution
            .offenders
            .iter()
            .all(|offender| offender.lane_stake.is_none())
    );
    drop(admitted);
    assert_eq!(budget.reserved_bytes(), baseline);
}

#[test]
fn admission_owner_retains_exact_key_and_original_signer_replay_fences() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let native = super::super::tests::conflict(&chain, 2);
    let proof = Evidence::from_native(&native).unwrap();
    let other = Evidence::from_native(&super::super::tests::vote_pair(&chain, 1, 4)).unwrap();
    observe(chain.state(), &native).unwrap();
    chain.commit(Vec::new());
    let state = chain.state();
    let generation = state.state_view_generation();
    assert!(
        matches!(AdmissionRead::capture(state, &state.view(), generation, 4, &[proof]), Err(EvidenceAdmissionError::Invalid(reason)) if reason == "evidence is already committed")
    );
    let mut read = AdmissionRead::capture(state, &state.view(), generation, 4, &[other]).unwrap();
    assert!(
        matches!(read.complete(), Err(EvidenceAdmissionError::Invalid(reason)) if reason.contains("original signer"))
    );
}

#[test]
fn admission_owner_cannot_capture_an_identical_foreign_state_view_or_odd_generation() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let mut other = super::super::tests::chain();
    other.commit(Vec::new());
    let proof = Evidence::from_native(&super::super::tests::conflict(&chain, 2)).unwrap();
    let state = chain.state();
    let generation = state.state_view_generation();
    assert!(
        matches!(AdmissionRead::capture(state, &other.state().view(), generation, 3, std::slice::from_ref(&proof)), Err(EvidenceAdmissionError::Source(error)) if error.kind() == std::io::ErrorKind::InvalidInput)
    );
    assert!(matches!(
        AdmissionRead::capture(state, &state.view(), generation | 1, 3, &[proof]),
        Err(EvidenceAdmissionError::Preparation(
            EvidencePreparationError::OriginalHistoryPending
        ))
    ));
}

#[test]
fn state_admission_cache_refuses_contention_without_blocking_empty_carriers() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let proof = Evidence::from_native(&super::super::tests::conflict(&chain, 2)).unwrap();
    let state = chain.state();
    let generation = state.state_view_generation();
    let baseline = state.evidence_preparation_budget().reserved_bytes();
    let held = state.native_evidence_admission.lock();
    assert!(matches!(
        prepare_admissions(state, generation, 3, std::slice::from_ref(&proof)),
        Err(EvidenceAdmissionError::Preparation(
            EvidencePreparationError::OriginalHistoryPending
        ))
    ));
    let empty = prepare_admissions(state, generation, 3, &[]).unwrap();
    assert!(empty.as_slice().is_empty());
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        baseline
    );
    drop(held);
    let admitted = prepare_admissions(state, generation, 3, &[proof]).unwrap();
    assert_eq!(admitted.as_slice().len(), 1);
    assert!(state.native_evidence_admission.lock().pending.is_none());
    drop(admitted);
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        baseline
    );
}

#[test]
fn replay_fence_funds_actual_native_roster_above_global_geometry() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let verified = crate::sumeragi::evidence_history::verify_from_state(
        &chain.state().view(),
        &super::super::tests::conflict(&chain, 2),
        |_, _| Ok(()),
    )
    .unwrap();
    let mut claim = verified.into_attribution();
    claim.offenders = (0..33_u32)
        .map(|signer| {
            let pair = iroha_crypto::KeyPair::from_seed(
                (signer + 1).to_le_bytes().to_vec(),
                iroha_crypto::Algorithm::BlsNormal,
            );
            EvidenceOffender {
                signer,
                peer_id: iroha_model_base::peer::PeerId::new(pair.public_key().clone()),
                lane_stake: None,
            }
        })
        .collect();
    let budget = AllocationBudget::new(33 * std::mem::size_of::<OriginalSigner>());
    let mut backing = ChargedBuffer::new(33, &budget).unwrap();
    let fence = ReplayFence::new(
        Hash::new(b"geometry-only replay fence"),
        true,
        &claim,
        &mut backing,
    )
    .unwrap();
    assert_eq!(fence.count, 33);
    assert!(fence.shares(&claim, backing.as_slice()).unwrap());
    assert_eq!(
        budget.reserved_bytes(),
        33 * std::mem::size_of::<OriginalSigner>()
    );
    let mut missing = claim.clone();
    missing.offenders.clear();
    assert!(!fence.shares(&missing, backing.as_slice()).unwrap());
    drop(backing);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn retained_admission_classifies_only_typed_local_refusal_as_retryable() {
    use iroha_data_model::query::error::QueryExecutionFail;
    use std::io::ErrorKind;
    for kind in [ErrorKind::WouldBlock, ErrorKind::Interrupted] {
        assert!(retryable(&EvidenceAdmissionError::Source(kind.into())));
    }
    for kind in [
        ErrorKind::NotFound,
        ErrorKind::InvalidData,
        ErrorKind::PermissionDenied,
    ] {
        assert!(
            !retryable(&EvidenceAdmissionError::Source(kind.into())),
            "{kind:?}"
        );
    }
    assert!(retryable(&EvidenceAdmissionError::History(
        QueryExecutionFail::GasBudgetExceeded
    )));
    assert!(!retryable(&EvidenceAdmissionError::History(
        QueryExecutionFail::Conversion("bad source".into())
    )));
    assert!(retryable(
        &EvidencePreparationError::OriginalHistoryPending.into()
    ));
    assert!(!retryable(&EvidencePreparationError::Invariant.into()));
    assert!(!retryable(
        &EvidencePreparationError::Admission(iroha_allocation::AllocationRefusal::DemandOverflow)
            .into()
    ));
    assert!(retryable(
        &EvidencePreparationError::Admission(iroha_allocation::AllocationRefusal::ExceedsLimit {
            requested_bytes: 2,
            limit_bytes: 1
        })
        .into()
    ));
}
