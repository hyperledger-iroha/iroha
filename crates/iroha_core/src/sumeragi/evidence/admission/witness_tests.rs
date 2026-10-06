//! Original signed conflicts retain untrusted witness attachments under their finite pool.
use super::*;
use crate::sumeragi::test_chain::{CertifiedTestChain, Signers};
use iroha_sumeragi::{message::ResultWitness, types::Hash32};

fn witnessed_conflict(chain: &CertifiedTestChain) -> Evidence {
    let committed = chain.committed(2);
    let witness = ResultWitness::from_untrusted(
        committed
            .block()
            .commit_certificate()
            .expect("actual certified original execution")
            .result_preimage()
            .to_vec(),
    )
    .unwrap();
    let mut first = chain.commit_qc(
        2,
        Hash32([0x31; 32]),
        committed.result(),
        true,
        Signers::Quorum,
    );
    let mut second = chain.commit_qc(
        2,
        Hash32([0x32; 32]),
        committed.result(),
        true,
        Signers::LastThree,
    );
    // Conflicting values prove safety through exact BLS signatures alone. Untrusted
    // attachments are retained evidence bytes, never native application authority.
    first.attestation_witness = Some(witness.clone());
    second.attestation_witness = Some(witness);
    Evidence::from_native(&NativeEvidence::ConflictingCertificates(first, second)).unwrap()
}
#[test]
fn retained_native_evidence_witnesses_belong_to_original_preparation_pool() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let proof = witnessed_conflict(&chain);
    let original = proof.native_frame().to_vec();
    let state = chain.state();
    let generation = state.state_view_generation();
    let budget = state.evidence_preparation_budget();
    let epoch = crossbeam_epoch::pin();
    let baseline = budget.reserved_bytes();
    let view = state.view();
    let mut read =
        AdmissionRead::capture(state, &view, generation, 3, std::slice::from_ref(&proof))
            .expect("real quorum signatures; attachments grant no native authority");
    drop(view);
    read.complete().unwrap();
    let candidate = &read.candidates.as_slice()[0];
    let NativeEvidence::ConflictingCertificates(first, second) = &candidate.native else {
        panic!("original certificate pair")
    };
    for certificate in [first, second] {
        assert!(
            certificate
                .attestation_witness
                .as_ref()
                .unwrap()
                .admitted_to(budget),
            "retained decoded proof witness is not funded by its original preparation pool"
        );
    }
    assert_eq!(candidate.frame.as_slice(), original);
    let admitted = read.finish().unwrap();
    assert!(admitted.belongs_to(budget));
    assert!(admitted.as_slice()[0].attribution().safety_violation);
    assert_eq!(proof.native_frame(), original);
    drop(admitted);
    assert_eq!(budget.reserved_bytes(), baseline);
    drop(epoch);
}

#[test]
fn witness_control_refusal_refunds_failed_capture_and_retries_original_source() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let proof = witnessed_conflict(&chain);
    let original = proof.native_frame().to_vec();
    let state = chain.state();
    let generation = state.state_view_generation();
    let budget = state.evidence_preparation_budget();
    let epoch = crossbeam_epoch::pin();
    let baseline = budget.reserved_bytes();
    let previous_limit = budget.limit_bytes();
    let native = proof.decode_native().unwrap();
    let NativeEvidence::ConflictingCertificates(first, _) = &native else {
        panic!("original signed pair")
    };
    let witness_len = first.attestation_witness.as_ref().unwrap().as_slice().len();
    let control = iroha_allocation::ChargedShared::<ChargedBuffer<u8>>::allocation_layout().size();
    // Empty retained history: exactly one candidate, one output, the frame and two witnesses.
    // The last witness backing fits, but its shared control does not. No estimated charge.
    let before_witness = std::mem::size_of::<Candidate>()
        + std::mem::size_of::<super::super::AdmittedEvidence>()
        + original.len();
    budget.set_limit_bytes(baseline + before_witness + 2 * witness_len + control);
    let view = state.view();
    let failure =
        match AdmissionRead::capture(state, &view, generation, 3, std::slice::from_ref(&proof)) {
            Ok(_) => panic!("second witness control must be refused"),
            Err(error) => error,
        };
    let EvidenceAdmissionError::Preparation(EvidencePreparationError::Admission(
        iroha_allocation::AllocationRefusal::Capacity {
            requested_bytes, ..
        },
    )) = &failure
    else {
        panic!("exact original-pool refusal: {failure:?}")
    };
    assert_eq!(*requested_bytes, control);
    assert!(retryable(&failure));
    assert_eq!(
        budget.reserved_bytes(),
        baseline,
        "failed capture retains no decoded progress"
    );
    assert_eq!(proof.native_frame(), original);
    assert_eq!(state.state_view_generation(), generation);
    budget.set_limit_bytes(previous_limit);
    let mut retry =
        AdmissionRead::capture(state, &view, generation, 3, std::slice::from_ref(&proof)).unwrap();
    drop(view);
    retry.complete().unwrap();
    assert!(
        retry.candidates.as_slice()[0]
            .native
            .result_witnesses_admitted_to(budget)
    );
    let admitted = retry.finish().unwrap();
    assert!(admitted.as_slice()[0].attribution().safety_violation);
    drop(admitted);
    assert_eq!(budget.reserved_bytes(), baseline);
    drop(epoch);
}

#[test]
fn funded_witnesses_do_not_authorize_invalid_original_signatures() {
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let proof = witnessed_conflict(&chain);
    let mut native = proof.decode_native().unwrap();
    let NativeEvidence::ConflictingCertificates(first, _) = &mut native else {
        panic!("original signed pair")
    };
    first.agg_sig.0[0] ^= 1;
    let invalid = Evidence::from_native(&native).unwrap();
    let state = chain.state();
    let epoch = crossbeam_epoch::pin();
    let baseline = state.evidence_preparation_budget().reserved_bytes();
    let view = state.view();
    let rejected =
        AdmissionRead::capture(state, &view, state.state_view_generation(), 3, &[invalid]);
    assert!(matches!(rejected, Err(EvidenceAdmissionError::Invalid(_))));
    assert_eq!(
        state.evidence_preparation_budget().reserved_bytes(),
        baseline
    );
    drop(view);
    drop(epoch);
}

#[test]
fn persisted_witness_refusal_keeps_validation_cut_and_original_record_for_retry() {
    let epoch = crossbeam_epoch::pin();
    let mut chain = super::super::tests::chain();
    chain.commit(Vec::new());
    let proof = witnessed_conflict(&chain);
    let native = proof.decode_native().unwrap();
    let key = evidence_key(&proof);
    super::super::observe(chain.state(), &native).unwrap();
    chain.commit(Vec::new());
    let state = chain.state();
    let budget = state.evidence_preparation_budget();
    let baseline = budget.reserved_bytes();
    let limit = budget.limit_bytes();
    let generation = state.state_view_generation();
    let tip = state.view().native_execution_tip();
    let original = state
        .view()
        .world()
        .consensus_evidence()
        .get(&key)
        .unwrap()
        .clone();
    let NativeEvidence::ConflictingCertificates(first, _) = &native else {
        panic!("original certified pair")
    };
    let control = iroha_allocation::ChargedShared::<ChargedBuffer<u8>>::allocation_layout().size();
    budget.set_limit_bytes(baseline + first.attestation_witness.as_ref().unwrap().as_slice().len());
    for _ in 0..2 {
        let refusal = super::super::validate_persisted_records(state).unwrap_err();
        assert!(
            matches!(&refusal, EvidenceAdmissionError::Preparation(EvidencePreparationError::Admission(
            iroha_allocation::AllocationRefusal::Capacity { requested_bytes, .. }
        )) if *requested_bytes == control),
            "{refusal:?}"
        );
        assert!(retryable(&refusal));
        assert!(state.native_evidence_admission.lock().restore.is_some());
        assert_eq!(
            budget.reserved_bytes(),
            baseline,
            "temporary root decode is refunded"
        );
        assert_eq!(state.state_view_generation(), generation);
        assert_eq!(state.view().native_execution_tip(), tip);
        assert_eq!(
            state.view().world().consensus_evidence().get(&key),
            Some(&original)
        );
    }
    budget.set_limit_bytes(limit);
    super::super::validate_persisted_records(state).unwrap();
    assert!(state.native_evidence_admission.lock().restore.is_none());
    assert_eq!(budget.reserved_bytes(), baseline);
    drop(epoch);
}
