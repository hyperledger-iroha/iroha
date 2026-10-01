//! Witness traversal, exact original-pool custody and retry parity for every evidence carrier.
use super::*;
use crate::message::{AttestationSignature, CommitAttestation, Defect, VoteKind, tests::*};

fn witness() -> ResultWitness {
    ResultWitness::from_untrusted(vec![0x31; 32]).unwrap()
}
fn qc() -> Qc {
    let mut qc = sample_qc(VoteKind::Commit, 0);
    qc.attestation_witness = Some(witness());
    qc
}
fn proposal() -> super::super::Proposal {
    let mut value = sample_proposal();
    value.parent_qc.as_mut().unwrap().attestation_witness = Some(witness());
    value
        .justify
        .as_mut()
        .unwrap()
        .high_pqc
        .as_mut()
        .unwrap()
        .attestation_witness = Some(witness());
    value
}
fn variants() -> [(Evidence, usize); 5] {
    let mut vote = sample_vote(VoteKind::Commit);
    vote.attestation = Some(CommitAttestation {
        witness: witness(),
        signature: AttestationSignature::try_from_slice(&[1; 16]).unwrap(),
    });
    let mut timeout = sample_timeout();
    timeout.high_pqc.as_mut().unwrap().attestation_witness = Some(witness());
    [
        (
            Evidence::ProposalEquivocation(Box::new(proposal()), Box::new(proposal())),
            4,
        ),
        (Evidence::VoteEquivocation(vote.clone(), vote), 2),
        (
            Evidence::TimeoutEquivocation(Box::new(timeout.clone()), Box::new(timeout)),
            2,
        ),
        (
            Evidence::InvalidProposal {
                proposal: Box::new(proposal()),
                defect: Defect::ParentHash,
            },
            2,
        ),
        (Evidence::ConflictingCertificates(qc(), qc()), 2),
    ]
}
fn witness_charge() -> usize {
    let pool = AllocationBudget::new(1 << 20);
    let mut value = witness();
    value.admit(&pool).unwrap();
    pool.reserved_bytes()
}

#[test]
fn every_evidence_witness_path_preserves_bytes_and_exact_shared_custody() {
    let unit = witness_charge();
    for (mut proof, count) in variants() {
        let original = proof.encode().unwrap();
        let budget = AllocationBudget::new(count * unit);
        let foreign = AllocationBudget::new(count * unit);
        assert_eq!(proof.result_witnesses().count(), count);
        assert!(!proof.result_witnesses_admitted_to(&budget));
        proof.admit_result_witnesses(&budget).unwrap();
        assert!(proof.result_witnesses_admitted_to(&budget));
        assert_eq!(budget.reserved_bytes(), count * unit);
        assert_eq!(proof.encode().unwrap(), original);
        let clone = proof.clone();
        assert_eq!(budget.reserved_bytes(), count * unit);
        for (original, cloned) in proof.result_witnesses().zip(clone.result_witnesses()) {
            assert_eq!(original.as_slice().as_ptr(), cloned.as_slice().as_ptr());
        }
        assert!(matches!(
            proof.admit_result_witnesses(&foreign),
            Err(ByteAdmissionError::ForeignBudget)
        ));
        assert_eq!(foreign.reserved_bytes(), 0);
        assert_eq!(budget.reserved_bytes(), count * unit);
        drop(proof);
        assert_eq!(budget.reserved_bytes(), count * unit);
        drop(clone);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn partial_witness_admission_retains_completed_and_pending_original_owners_for_retry() {
    let unit = witness_charge();
    let mut proof = Evidence::ConflictingCertificates(qc(), qc());
    let original = proof.encode().unwrap();
    let budget = AllocationBudget::new(unit + 32);
    let foreign = AllocationBudget::new(2 * unit);
    assert!(matches!(
        proof.admit_result_witnesses(&budget),
        Err(ByteAdmissionError::ControlAdmission(_))
    ));
    assert_eq!(budget.reserved_bytes(), unit + 32);
    let first = proof.result_witnesses().next().unwrap().as_slice().as_ptr();
    let pending = proof.result_witnesses().nth(1).unwrap().as_slice().as_ptr();
    assert!(
        proof
            .result_witnesses()
            .next()
            .unwrap()
            .admitted_to(&budget)
    );
    assert!(!proof.result_witnesses_admitted_to(&budget));
    assert!(matches!(
        proof.admit_result_witnesses(&foreign),
        Err(ByteAdmissionError::ForeignBudget)
    ));
    assert_eq!(foreign.reserved_bytes(), 0);
    assert!(
        proof
            .admit_result_witnesses(&budget)
            .unwrap_err()
            .is_local_refusal()
    );
    assert_eq!(budget.reserved_bytes(), unit + 32);
    assert_eq!(
        proof.result_witnesses().next().unwrap().as_slice().as_ptr(),
        first
    );
    assert_eq!(
        proof.result_witnesses().nth(1).unwrap().as_slice().as_ptr(),
        pending
    );
    assert_eq!(proof.encode().unwrap(), original);
    budget.set_limit_bytes(2 * unit);
    proof.admit_result_witnesses(&budget).unwrap();
    assert_eq!(budget.reserved_bytes(), 2 * unit);
    assert_eq!(
        proof.result_witnesses().nth(1).unwrap().as_slice().as_ptr(),
        pending
    );
    assert!(proof.result_witnesses_admitted_to(&budget));
    assert_eq!(
        proof.result_witnesses().next().unwrap().as_slice().as_ptr(),
        first
    );
    assert_eq!(proof.encode().unwrap(), original);
    drop(proof);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn witness_funding_does_not_synthesize_missing_attachments() {
    let mut proof = Evidence::ConflictingCertificates(
        sample_qc(VoteKind::Commit, 0),
        sample_qc(VoteKind::Commit, 1),
    );
    let original = proof.encode().unwrap();
    let budget = AllocationBudget::new(0);
    proof.admit_result_witnesses(&budget).unwrap();
    assert_eq!(proof.result_witnesses().count(), 0);
    assert!(proof.result_witnesses_admitted_to(&budget));
    assert_eq!(budget.reserved_bytes(), 0);
    assert_eq!(proof.encode().unwrap(), original);
}
