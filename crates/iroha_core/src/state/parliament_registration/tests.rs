//! Real wallet proof and original-signature custody; no synthetic governance certificate.

use super::*;
use crate::{
    governance::parliament::{
        ParliamentAttemptStateV1, tests::credential_registration_invitation_fixture,
    },
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    governance::anonymous_ballot::RegistrationProofBytesV1,
    isi::OpaqueInstructionV1,
    parameter::Parameter,
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use iroha_parliament_ballot::wallet::{CredentialHandleV1, prepare_registration_v1};

fn test_chain(selected: bool) -> CertifiedTestChain {
    let mut config = TestChainConfig::new(World::new(), 1_000);
    if selected {
        config.genesis_parameters.push(Parameter::Custom(
            ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
                .to_custom_parameter()
                .unwrap(),
        ));
    }
    CertifiedTestChain::start(config).unwrap()
}

fn signed(
    chain: &CertifiedTestChain,
    key: &KeyPair,
    input: &RegisterParliamentCredentialV1,
) -> SignedTransaction {
    signed_frame(chain, key, norito::encode_canonical(input).unwrap())
}

fn signed_frame(chain: &CertifiedTestChain, key: &KeyPair, frame: Vec<u8>) -> SignedTransaction {
    let instruction: InstructionBox =
        OpaqueInstructionV1::new(RegisterParliamentCredentialV1::WIRE_ID, frame)
            .unwrap()
            .into();
    TransactionBuilder::new(
        chain.network_id(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction])
    .sign(key.private_key())
}

fn arm(tx: &mut StateTransaction<'_, '_>, signed: &SignedTransaction, direct: bool) {
    tx.current_tx_hash = Some(signed.hash());
    tx.tx_call_hash = Some(Hash::from(signed.hash_as_entrypoint()));
    tx.current_network_entrypoint_hash = Some(signed.hash_as_entrypoint());
    tx.current_entrypoint_index = Some(3);
    let Executable::Instructions(instructions) = signed.instructions() else {
        unreachable!()
    };
    tx.arm_direct_parliament_registration(signed, signed.authority(), &instructions[0], 0, direct);
}

fn with_tx(
    chain: &CertifiedTestChain,
    attempt: &ParliamentAttemptStateV1,
    height: u64,
    f: impl FnOnce(&mut StateTransaction<'_, '_>),
) {
    // The fixture is a validated reducer invitation, not an enactment or ledger receipt.
    let header = BlockHeader::new(height.try_into().unwrap(), None, None, 2_000, 0);
    let mut block = chain.state().block(header);
    let mut tx = block.transaction();
    tx.world.put_parliament_attempt(attempt.clone()).unwrap();
    f(&mut tx);
    // Deliberately never apply this disposable overlay.
}

#[tokio::test]
async fn registration_preparation_verifies_real_proof_and_preserves_original_custody() {
    let chain = test_chain(true);
    let (attempt, election_id, key) = credential_registration_invitation_fixture();
    let authority = AccountId::new(key.public_key().clone());
    let assignment = attempt
        .check_invitation_response(attempt.attempt().id, election_id, &authority, 20)
        .unwrap();
    let body = BallotBodyContextV1 {
        network_id: chain.network_id(),
        governance_attempt_id: attempt.attempt().id,
        election_attempt_id: election_id,
        body_role: attempt
            .election(&election_id)
            .unwrap()
            .attempt()
            .request
            .body,
    };
    let credential = CredentialHandleV1::import_v1(body, &[0x53; 48], None).unwrap();
    let registration = prepare_registration_v1(&credential, &authority, &assignment)
        .expect("real candidate registration relation, not a verifier mock");
    assert_eq!(registration.proof.as_bytes().len(), 160_148);
    let input = RegisterParliamentCredentialV1 {
        governance_attempt_id: attempt.attempt().id,
        election_attempt_id: election_id,
        registration,
    };
    let original = signed(&chain, &key, &input);
    original.verify_signature().unwrap();
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &original, true);
        let prepared = tx
            .prepare_candidate_parliament_registration(&original)
            .unwrap();
        assert_eq!(prepared.assignment, assignment);
        assert_eq!(prepared.commitment, input.registration.commitment);
        assert_eq!(prepared.authorization, input.registration.authorization);
        assert_eq!(prepared.original.input_index, 3);
        assert_eq!(
            prepared.original.signed_bytes,
            HashOf::try_new(&original).unwrap()
        );
        assert!(
            core::mem::size_of_val(&prepared) < 1024,
            "proof is not retained in the compact record"
        );
        drop(prepared);
        assert!(
            tx.world
                .parliament_attempts
                .get(&attempt.attempt().id)
                .unwrap()
                .election(&election_id)
                .unwrap()
                .accepted_assignments()
                .is_empty()
        );
        assert!(
            matches!(
                tx.prepare_candidate_parliament_registration(&original),
                Err(RegistrationPreparationError::Rejected(_))
            ),
            "one-use original consumed"
        );
    });

    // A changed authorization keeps the same intent identifier. Only the complete
    // signed native encoding, not hash()/hash_as_entrypoint(), detects substitution.
    let other_key = KeyPair::try_from_seed(vec![0x75; 32], Algorithm::Ed25519).unwrap();
    let other = signed(&chain, &other_key, &input);
    let mut changed_signature = original.clone();
    changed_signature.set_signature(other.signature().clone());
    assert_eq!(original.hash(), changed_signature.hash());
    assert_ne!(
        HashOf::try_new(&original).unwrap(),
        HashOf::try_new(&changed_signature).unwrap()
    );
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &original, true);
        assert!(matches!(
            tx.prepare_candidate_parliament_registration(&changed_signature),
            Err(RegistrationPreparationError::Rejected(
                "registration original source changed"
            ))
        ));
    });

    for direct in [false, true] {
        with_tx(&chain, &attempt, 20, |tx| {
            arm(tx, &original, direct);
            if direct {
                tx.clear_direct_parliament_candidate_testing();
            }
            assert!(
                matches!(
                    tx.prepare_candidate_parliament_registration(&original),
                    Err(RegistrationPreparationError::Rejected(_))
                ),
                "nested or cleared authority refuses"
            );
        });
    }
    for changed_coordinate in 0..3 {
        with_tx(&chain, &attempt, 20, |tx| {
            arm(tx, &original, true);
            match changed_coordinate {
                0 => tx.current_entrypoint_index = Some(4),
                1 => tx.current_network_entrypoint_hash = None,
                _ => tx.active_trigger_execution_depth = 1,
            }
            assert!(matches!(
                tx.prepare_candidate_parliament_registration(&original),
                Err(RegistrationPreparationError::Rejected(_))
            ));
        });
    }
    for height in [19, 30] {
        with_tx(&chain, &attempt, height, |tx| {
            arm(tx, &original, true);
            assert!(matches!(
                tx.prepare_candidate_parliament_registration(&original),
                Err(RegistrationPreparationError::Rejected(
                    "registration invitation is not currently admissible"
                ))
            ));
        });
    }
    let mut declined = attempt.clone();
    declined
        .record_invitation_response(attempt.attempt().id, election_id, &authority, false, 20)
        .unwrap();
    with_tx(&chain, &declined, 20, |tx| {
        arm(tx, &original, true);
        assert!(
            matches!(
                tx.prepare_candidate_parliament_registration(&original),
                Err(RegistrationPreparationError::Rejected(_))
            ),
            "recorded response cannot be replaced"
        );
    });

    // Proof and authorization mutations are signed originals themselves, but fail
    // the actual registration verifier against State's unchanged body and member.
    for mutate_proof in [false, true] {
        let mut invalid = input.clone();
        if mutate_proof {
            let mut bytes = invalid.registration.proof.as_bytes().to_vec();
            bytes[120] ^= 1;
            invalid.registration.proof = RegistrationProofBytesV1::new(bytes).unwrap();
        } else {
            invalid.registration.authorization = RegistrationAuthorizationV1::new([0xA1; 64]);
        }
        let signed = signed(&chain, &key, &invalid);
        with_tx(&chain, &attempt, 20, |tx| {
            arm(tx, &signed, true);
            assert!(matches!(
                tx.prepare_candidate_parliament_registration(&signed),
                Err(RegistrationPreparationError::Proof(
                    RegistrationErrorV1::Rejected
                ))
            ));
        });
    }
    let other_member_key = (0x90..=0x92)
        .map(|tag| KeyPair::try_from_seed(vec![tag; 32], Algorithm::Ed25519).unwrap())
        .find(|candidate| candidate.public_key() != key.public_key())
        .unwrap();
    let other_member = signed(&chain, &other_member_key, &input);
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &other_member, true);
        assert!(
            matches!(
                tx.prepare_candidate_parliament_registration(&other_member),
                Err(RegistrationPreparationError::Proof(
                    RegistrationErrorV1::Rejected
                ))
            ),
            "the other real invitation derives its own authority and assignment"
        );
    });
    let unselected = test_chain(false);
    let unselected_original = signed(&unselected, &key, &input);
    with_tx(&unselected, &attempt, 20, |tx| {
        arm(tx, &unselected_original, true);
        assert!(matches!(
            &tx.direct_parliament_registration,
            Some(Err(RegistrationPreparationError::Rejected(
                "candidate registration was not selected by genesis"
            )))
        ));
        assert!(matches!(
            tx.prepare_candidate_parliament_registration(&unselected_original),
            Err(RegistrationPreparationError::Rejected(
                "candidate registration was not selected by genesis"
            ))
        ));
    });
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &unselected_original, true);
        assert!(
            matches!(
                tx.prepare_candidate_parliament_registration(&unselected_original),
                Err(RegistrationPreparationError::Rejected(_))
            ),
            "foreign network cannot arm source custody"
        );
    });
    let oversized = signed_frame(
        &chain,
        &key,
        vec![0; RegisterParliamentCredentialV1::MAX_FRAME_BYTES + 1],
    );
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &oversized, true);
        assert!(matches!(
            &tx.direct_parliament_registration,
            Some(Err(RegistrationPreparationError::Rejected(
                "registration original exceeds its fixed input bound"
            )))
        ));
        assert!(
            matches!(
                tx.prepare_candidate_parliament_registration(&oversized),
                Err(RegistrationPreparationError::Rejected(_))
            ),
            "oversized source has no full-envelope custody hash"
        );
    });
    let Executable::Instructions(original_instructions) = original.instructions() else {
        unreachable!()
    };
    let batch = TransactionBuilder::new(
        chain.network_id(),
        authority.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([
        original_instructions[0].clone(),
        original_instructions[0].clone(),
    ])
    .sign(key.private_key());
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &batch, true);
        assert!(
            tx.direct_parliament_registration.is_none(),
            "batch needs a shared paid original owner, not repeated full-envelope hashes"
        );
    });
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &original, true);
        let pool = tx
            .world
            .sccp_committee_generations
            .allocation_budget()
            .clone();
        let occupied = pool
            .try_reserve_bytes(pool.limit_bytes() - pool.reserved_bytes())
            .unwrap();
        let error = tx
            .prepare_candidate_parliament_registration(&original)
            .err()
            .unwrap();
        assert!(matches!(error, RegistrationPreparationError::Allocation(_)));
        assert!(error.is_local());
        drop(occupied);
        assert!(
            tx.world
                .parliament_attempts
                .get(&attempt.attempt().id)
                .unwrap()
                .election(&election_id)
                .unwrap()
                .accepted_assignments()
                .is_empty()
        );
    });
    let mut trailing = norito::encode_canonical(&input).unwrap();
    trailing.push(0);
    let noncanonical = signed_frame(&chain, &key, trailing);
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &noncanonical, true);
        assert!(matches!(
            tx.prepare_candidate_parliament_registration(&noncanonical),
            Err(RegistrationPreparationError::Decode(
                ExecutionAttemptError::Rejected(_)
            ))
        ));
    });
    with_tx(&chain, &attempt, 20, |tx| {
        arm(tx, &original, true);
        // The proof is byte-oriented, so a sequence-element ceiling need not
        // constrain it. Retain a real original physical decode owner whose
        // allocation allowance cannot admit the otherwise valid frame.
        let physical = norito::core::DecodeBudgetContext::try_new_owned(
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 32),
            tx.world.sccp_committee_generations.allocation_budget(),
        )
        .unwrap();
        let result = physical.with(|| {
            tx.prepare_candidate_parliament_registration(&original)
                .map(drop)
        });
        assert!(
            result.as_ref().is_err_and(|error| error.is_local()),
            "original physical decode refusal must remain local: {result:?}"
        );
        assert!(
            matches!(
                result,
                Err(RegistrationPreparationError::Decode(
                    ExecutionAttemptError::Deferred(_)
                ))
            ),
            "original inherited decode inability remains local, not an invalid registration"
        );
    });
    assert!(!iroha_data_model::governance::anonymous_ballot::parliament_ballot_qualified_v1());
}

#[test]
fn registration_policy_precondition_refuses_malformed_selected_policy_without_default() {
    let malformed = CustomParameter::new(
        ParliamentCandidateTestingPolicyV1::parameter_id().clone(),
        iroha_primitives::json::Json::new(17_u64),
    );
    let error = require_candidate_policy(Some(&malformed)).unwrap_err();
    assert!(matches!(
        error,
        RegistrationPreparationError::Decode(ExecutionAttemptError::Rejected(_))
    ));
    assert!(!error.is_local());
    assert!(require_candidate_policy(None).is_err());
    let exact = ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
        .to_custom_parameter()
        .unwrap();
    assert!(require_candidate_policy(Some(&exact)).is_ok());
}
