//! Custody of the one-use direct signed original of a credential registration.
//!
//! The proof is never verified here: the reducer verifies it (see the anonymous family's
//! World tests), so these inputs carry a structurally valid placeholder proof.

use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::BlockHeader,
    governance::{
        anonymous_ballot::{
            CredentialCommitmentV1, CredentialRegistrationV1, RegistrationAuthorizationV1,
            RegistrationProofBytesV1,
        },
        types::{BodyElectionAttemptId, GovernanceAttemptId},
    },
    parameter::Parameter,
    prelude::{Level, Log},
    transaction::{FeePaymentIntent, TransactionBuilder},
};
use ivm::error::ExecutionDeferral;

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

fn input() -> RegisterParliamentCredentialV1 {
    RegisterParliamentCredentialV1 {
        governance_attempt_id: GovernanceAttemptId::new([1; 32]),
        election_attempt_id: BodyElectionAttemptId::new([2; 32]),
        registration: CredentialRegistrationV1 {
            commitment: CredentialCommitmentV1::new([3; 128]),
            authorization: RegistrationAuthorizationV1::new([4; 64]),
            proof: RegistrationProofBytesV1::new(vec![5; RegistrationProofBytesV1::LENGTH])
                .unwrap(),
        },
    }
}

fn key() -> KeyPair {
    KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519).unwrap()
}

fn signed(
    chain: &CertifiedTestChain,
    key: &KeyPair,
    instructions: impl IntoIterator<Item = InstructionBox>,
) -> SignedTransaction {
    TransactionBuilder::new(
        chain.network_id(),
        AccountId::new(key.public_key().clone()),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions)
    .sign(key.private_key())
}

fn registration(chain: &CertifiedTestChain, key: &KeyPair) -> SignedTransaction {
    signed(chain, key, [InstructionBox::from(input())])
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

fn with_tx(chain: &CertifiedTestChain, height: u64, f: impl FnOnce(&mut StateTransaction<'_, '_>)) {
    let header = BlockHeader::new(height.try_into().unwrap(), None, None, 2_000, 0);
    let mut block = chain.state().block(header);
    let mut tx = block.transaction();
    f(&mut tx);
    // Deliberately never apply this disposable overlay.
}

#[tokio::test]
async fn the_original_yields_its_source_exactly_once() {
    let chain = test_chain(true);
    let original = registration(&chain, &key());
    with_tx(&chain, 20, |tx| {
        arm(tx, &original, true);
        let source = tx.consume_direct_parliament_registration().unwrap();
        assert_eq!(
            source,
            ParliamentCredentialSourceV1 {
                height: 20,
                input_index: 3,
                instruction_index: 0,
                entrypoint_hash: original.hash_as_entrypoint(),
            }
        );
        assert!(
            matches!(
                tx.consume_direct_parliament_registration(),
                Err(RegistrationSourceError::Rejected(_))
            ),
            "one-use original consumed"
        );
    });
}

#[tokio::test]
async fn nested_cleared_or_moved_execution_has_no_original() {
    let chain = test_chain(true);
    let original = registration(&chain, &key());
    for direct in [false, true] {
        with_tx(&chain, 20, |tx| {
            arm(tx, &original, direct);
            if direct {
                tx.clear_direct_parliament_candidate_testing();
            }
            assert!(
                matches!(
                    tx.consume_direct_parliament_registration(),
                    Err(RegistrationSourceError::Rejected(_))
                ),
                "nested or cleared authority refuses"
            );
        });
    }
    for changed_coordinate in 0..4 {
        with_tx(&chain, 20, |tx| {
            arm(tx, &original, true);
            match changed_coordinate {
                0 => tx.current_entrypoint_index = Some(4),
                1 => tx.current_network_entrypoint_hash = None,
                2 => tx.current_tx_hash = None,
                _ => tx.active_trigger_execution_depth = 1,
            }
            assert!(matches!(
                tx.consume_direct_parliament_registration(),
                Err(RegistrationSourceError::Rejected(
                    "registration original source changed"
                ))
            ));
        });
    }
}

#[tokio::test]
async fn only_the_sole_registration_of_a_direct_input_of_this_network_is_armed() {
    let chain = test_chain(true);
    let key = key();
    let batch = signed(
        &chain,
        &key,
        [InstructionBox::from(input()), InstructionBox::from(input())],
    );
    let other_instruction = signed(
        &chain,
        &key,
        [InstructionBox::from(Log::new(
            Level::INFO,
            "not a registration".to_owned(),
        ))],
    );
    let foreign = test_chain(false);
    let foreign_original = registration(&foreign, &key);
    for transaction in [&batch, &other_instruction, &foreign_original] {
        with_tx(&chain, 20, |tx| {
            arm(tx, transaction, true);
            assert!(
                tx.direct_parliament_registration.is_none(),
                "a batch, another instruction or another network arms nothing"
            );
            assert!(matches!(
                tx.consume_direct_parliament_registration(),
                Err(RegistrationSourceError::Rejected(
                    "registration needs its one-use direct signed original"
                ))
            ));
        });
    }
}

#[tokio::test]
async fn a_network_without_the_candidate_selection_refuses_the_original() {
    let unselected = test_chain(false);
    let original = registration(&unselected, &key());
    with_tx(&unselected, 20, |tx| {
        arm(tx, &original, true);
        assert!(matches!(
            &tx.direct_parliament_registration,
            Some(Err(RegistrationSourceError::Rejected(
                "candidate registration was not selected by genesis"
            )))
        ));
        assert!(matches!(
            tx.consume_direct_parliament_registration(),
            Err(RegistrationSourceError::Rejected(
                "candidate registration was not selected by genesis"
            ))
        ));
    });
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
        RegistrationSourceError::Policy(ExecutionAttemptError::Rejected(_))
    ));
    assert!(
        error.into_deferral().is_err(),
        "a malformed policy is a verdict"
    );
    assert!(require_candidate_policy(None).is_err());
    let exact = ParliamentCandidateTestingPolicyV1::CANDIDATE_V1
        .to_custom_parameter()
        .unwrap();
    assert!(require_candidate_policy(Some(&exact)).is_ok());
}

#[test]
fn only_a_policy_the_node_could_not_read_defers() {
    let local = RegistrationSourceError::Policy(ExecutionAttemptError::Deferred(
        ExecutionDeferral::VerifierArtifactsUnavailable.into(),
    ));
    assert!(local.into_deferral().is_ok());
    assert!(
        RegistrationSourceError::Rejected("refused")
            .into_deferral()
            .is_err()
    );
}
