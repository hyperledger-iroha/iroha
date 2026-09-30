//! Native evidence admission, original state binding and penalty publication.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, Signers, TestChainConfig},
};
use iroha_data_model::parameter::{
    Parameter,
    system::{SumeragiConsensusMode, SumeragiNposParameters},
};
use iroha_sumeragi::{
    message::{Vote, VoteKind},
    types::{Hash32, Signature},
};

fn chain() -> CertifiedTestChain {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    config.consensus_mode = SumeragiConsensusMode::Npos;
    config.genesis_parameters.push(Parameter::Custom(
        SumeragiNposParameters {
            slashing_delay_blocks: 2,
            ..SumeragiNposParameters::default()
        }
        .into_custom_parameter(),
    ));
    CertifiedTestChain::start(config).unwrap()
}
fn conflict(chain: &CertifiedTestChain, height: u64) -> NativeEvidence {
    NativeEvidence::ConflictingCertificates(
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
fn canonical_key_and_overflow_safe_byte_accounting() {
    let chain = chain();
    let native = conflict(&chain, 2);
    let evidence = Evidence::from_native(&native).unwrap();
    let NativeEvidence::ConflictingCertificates(a, b) = native else {
        unreachable!()
    };
    let reversed = Evidence::from_native(&NativeEvidence::ConflictingCertificates(b, a)).unwrap();
    assert_eq!(evidence, reversed);
    assert_eq!(evidence_key(&evidence), evidence_key(&reversed));
    assert_eq!(
        evidence_encoded_len(&evidence),
        evidence.native_frame().len()
    );
    assert_eq!(checked_evidence_byte_sum(2, [3, 4], 9), Some(9));
    assert_eq!(checked_evidence_byte_sum(2, [3, 5], 9), None);
    assert_eq!(checked_evidence_byte_sum(usize::MAX, [1], usize::MAX), None);
}
#[test]
fn original_native_history_authenticates_every_certificate_intersection_signer() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let evidence = Evidence::from_native(&conflict(&chain, 2)).unwrap();
    let view = chain.state().view();
    let admissions = validate_admissions(&view, 3, &[evidence.clone()]).unwrap();
    let attribution = admissions[0].attribution();
    assert_eq!(attribution.height, 2);
    assert!(attribution.safety_violation);
    assert_eq!(
        attribution
            .offenders
            .iter()
            .map(|offender| offender.signer)
            .collect::<Vec<_>>(),
        [1, 2]
    );
    for offender in &attribution.offenders {
        assert_eq!(
            offender.peer_id,
            chain.validators()[offender.signer as usize].0
        );
    }
    assert!(validate_admissions(&view, 2, &[evidence.clone()]).is_err());
    assert!(validate_admissions(&view, 3, &[evidence.clone(), evidence]).is_err());
}
#[test]
fn observer_pool_is_charged_and_refusal_preserves_original_source() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let evidence = conflict(&chain, 2);
    let state = chain.state();
    let original_tip = state.view().native_execution_tip();
    let budget = state.evidence_preparation_budget();
    let occupation = budget.try_reserve_bytes(budget.limit_bytes()).unwrap();
    assert!(observe(state, &evidence).is_err());
    assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
    assert_eq!(state.view().native_execution_tip(), original_tip);
    assert!(
        state
            .view()
            .world()
            .consensus_evidence()
            .iter()
            .next()
            .is_none()
    );
    drop(occupation);
    assert!(observe(state, &evidence).unwrap());
    let retained = budget.reserved_bytes();
    assert!(retained > 0);
    assert!(!observe(state, &evidence).unwrap());
    assert_eq!(budget.reserved_bytes(), retained);
    let selected = pending_evidence_admissions_from_view(state, 3, &state.view());
    assert_eq!(selected.len(), 1);
    assert!(
        state
            .view()
            .world()
            .consensus_evidence()
            .iter()
            .next()
            .is_none()
    );
}

#[test]
fn pending_selection_uses_the_original_parent_even_after_successor_admission() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let native = conflict(&chain, 2);
    let evidence = Evidence::from_native(&native).unwrap();
    let key = evidence_key(&evidence);
    let state = std::sync::Arc::clone(chain.state());
    assert!(observe(&state, &native).unwrap());
    let parent = state.view();
    assert_eq!(parent.height(), 2);
    chain.commit(Vec::new());
    assert!(
        state
            .view()
            .world()
            .consensus_evidence()
            .get(&key)
            .is_some()
    );
    assert!(parent.world().consensus_evidence().get(&key).is_none());
    let selected = pending_evidence_admissions_from_view(&state, 3, &parent);
    assert_eq!(
        selected,
        [evidence],
        "selection belongs to the captured parent"
    );
}
#[test]
fn original_carrier_admits_then_finality_terminalizes_exact_parent_evidence() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let evidence = conflict(&chain, 2);
    let key = evidence_key(&Evidence::from_native(&evidence).unwrap());
    observe(chain.state(), &evidence).unwrap();
    chain.commit(Vec::new());
    {
        let view = chain.state().view();
        let record = view.world().consensus_evidence().get(&key).unwrap();
        assert_eq!(record.recorded_at_height, 3);
        assert_eq!(record.penalty_status, EvidencePenaltyStatus::Pending);
        assert_eq!(record.attribution.offenders.len(), 2);
        validate_persisted_records(&view).unwrap();
        assert!(validate_admissions(&view, 4, &[record.evidence.clone()]).is_err());
    }
    chain.commit(Vec::new());
    chain.commit(Vec::new());
    let view = chain.state().view();
    assert_eq!(
        view.world()
            .consensus_evidence()
            .get(&key)
            .unwrap()
            .penalty_status,
        EvidencePenaltyStatus::Applied { height: 5 }
    );
    validate_persisted_records(&view).unwrap();
}
#[test]
fn pristine_token_rejects_an_identical_foreign_state_without_any_effect() {
    let first = chain();
    let second = chain();
    let proposal = first.proposal(None, Vec::new());
    let token = prepare(
        first.state(),
        &proposal,
        first.state().state_view_generation(),
    )
    .unwrap();
    let mut overlay = second.state().block(proposal.header());
    assert!(
        token
            .apply(
                &mut overlay,
                &proposal,
                second.state(),
                second.state().state_view_generation()
            )
            .is_err()
    );
    drop(overlay);
    assert_eq!(second.state().view().height(), 1);
}
#[test]
fn altered_signature_instance_and_epoch_never_enter_local_observations() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let native = conflict(&chain, 2);
    let NativeEvidence::ConflictingCertificates(first, second) = native else {
        unreachable!()
    };
    let mut changed = first.clone();
    changed.epoch.epoch += 1;
    assert!(
        observe(
            chain.state(),
            &NativeEvidence::ConflictingCertificates(changed, second.clone())
        )
        .is_err()
    );
    let mut changed = first.clone();
    changed.instance = Hash32([0xFF; 32]);
    assert!(
        observe(
            chain.state(),
            &NativeEvidence::ConflictingCertificates(changed, second.clone())
        )
        .is_err()
    );
    let mut changed = first;
    changed.agg_sig.0[0] ^= 1;
    assert!(
        observe(
            chain.state(),
            &NativeEvidence::ConflictingCertificates(changed, second)
        )
        .is_err()
    );
    assert_eq!(
        chain.state().evidence_preparation_budget().reserved_bytes(),
        0
    );
    assert!(
        chain
            .state()
            .view()
            .world()
            .consensus_evidence()
            .iter()
            .next()
            .is_none()
    );
}
#[test]
fn same_block_proof_cannot_authorize_a_penalty() {
    let mut chain = chain();
    chain.commit(Vec::new());
    let proof = Evidence::from_native(&conflict(&chain, 2)).unwrap();
    let admissions = validate_admissions(&chain.state().view(), 3, &[proof]).unwrap();
    let actions = [NposPenaltyAction::MarkConsensusEvidenceApplied(
        iroha_data_model::consensus::NposMarkConsensusEvidenceAppliedAction {
            evidence_key: admissions[0].key(),
            height: 3,
        },
    )];
    assert!(validate_admission_penalty_separation(&admissions, &actions).is_err());
    assert!(validate_admission_penalty_separation(&admissions, &[]).is_ok());
}

fn vote_pair(chain: &CertifiedTestChain, signer: u32, view_number: u64) -> NativeEvidence {
    use iroha_crypto::{Algorithm, KeyPair};
    let view = chain.state().view();
    let epoch = view
        .world()
        .consensus_schedule()
        .ready(2)
        .unwrap()
        .height_config()
        .unwrap()
        .epoch
        .id;
    let key = [0xC1, 0xC2, 0xC3, 0xC4]
        .into_iter()
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .find(|key| key.public_key() == chain.validators()[signer as usize].0.public_key())
        .unwrap();
    let vote = |byte| {
        let mut vote = Vote {
            kind: VoteKind::Prepare,
            instance: chain.instance(),
            epoch,
            height: 2,
            view: view_number,
            block_hash: Hash32([byte; 32]),
            result: Hash32([0x72; 32]),
            attest: false,
            signer,
            sig: Signature([0; iroha_sumeragi::types::SIGNATURE_LEN]),
            attestation: None,
        };
        vote.sig = Signature(
            iroha_crypto::Signature::new(key.private_key(), &vote.preimage())
                .payload()
                .try_into()
                .unwrap(),
        );
        vote
    };
    NativeEvidence::VoteEquivocation(vote(0x71), vote(0x73))
}
#[test]
fn signer_indices_are_canonical_and_do_not_rotate_with_view() {
    let chain = chain();
    for view_number in [0, u64::MAX] {
        let evidence = vote_pair(&chain, 2, view_number);
        let verified = super::super::evidence_history::verify_from_state(
            &chain.state().view(),
            &evidence,
            |_, _| Ok(()),
        )
        .unwrap();
        assert_eq!(
            verified.offenders(),
            &[(2, chain.validators()[2].0.clone())]
        );
    }
}
#[test]
fn npos_mode_does_not_remap_equal_vote_evidence_signer_indices() {
    let chain = chain();
    let proof = vote_pair(&chain, 1, 47);
    let verified =
        super::super::evidence_history::verify_from_state(&chain.state().view(), &proof, |_, _| {
            Ok(())
        })
        .unwrap();
    assert_eq!(
        verified.offenders(),
        &[(1, chain.validators()[1].0.clone())]
    );
}
#[test]
fn original_signer_requires_matching_height_and_in_range_roster() {
    let chain = chain();
    let NativeEvidence::VoteEquivocation(first, second) = vote_pair(&chain, 1, 0) else {
        unreachable!()
    };
    let mut wrong_height = first.clone();
    wrong_height.height += 1;
    assert!(
        super::super::evidence_history::verify_from_state(
            &chain.state().view(),
            &NativeEvidence::VoteEquivocation(wrong_height, second.clone()),
            |_, _| Ok(())
        )
        .is_err()
    );
    let mut missing_signer = first;
    missing_signer.signer = 4;
    assert!(
        super::super::evidence_history::verify_from_state(
            &chain.state().view(),
            &NativeEvidence::VoteEquivocation(missing_signer, second),
            |_, _| Ok(())
        )
        .is_err()
    );
}
