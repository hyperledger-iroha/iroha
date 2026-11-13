//! Native-history binding controls, including the genuine accepted registration exercised by
//! the anonymous family's World test.

use super::*;
use crate::{
    state::World,
    sumeragi::{
        certified_chain::CertifiedChain,
        test_chain::{CertifiedTestChain, TestChainConfig},
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    governance::{
        anonymous_ballot::{
            CredentialCommitmentV1, ParliamentCredentialSourceV1, RegistrationAuthorizationV1,
        },
        types::{BodyElectionAttemptId, GovernanceAttemptId, ParliamentBody},
    },
    prelude::{Level, Log},
    transaction::{
        SignedTransaction,
        signed::{SealedTransactionCommitmentPayload, SignedSealedTransactionCommitment},
    },
};

/// Check the actual accepted source and its rejected batch predecessor without constructing
/// another expensive registration proof. The caller is the genuine executor registration test.
pub(crate) fn assert_registration_history(
    chain: &CertifiedTestChain,
    context: &BallotBodyContextV1,
    assignment: AssignmentId,
    record: &ParliamentCredentialRecordV1,
    accepted: &SignedTransaction,
    rejected: &SignedTransaction,
) {
    let view = chain.state().view();
    let history = CertifiedChain::new(&view).unwrap();
    let original = history
        .authenticated_execution(record.source.height)
        .unwrap();
    let registration =
        resolve_credential_registration_v1(&original, context, assignment, record).unwrap();
    let expected = accepted
        .instructions()
        .explicit_instructions()
        .next()
        .unwrap()
        .as_any()
        .downcast_ref::<RegisterParliamentCredentialV1>()
        .unwrap();
    assert_eq!(registration, expected);
    let input = original
        .block()
        .network_entrypoint_at(usize::try_from(record.source.input_index).unwrap())
        .unwrap();
    let TransactionEntrypoint::External(signed) = input else {
        unreachable!()
    };
    let borrowed = signed
        .instructions()
        .explicit_instructions()
        .next()
        .unwrap()
        .as_any()
        .downcast_ref::<RegisterParliamentCredentialV1>()
        .unwrap();
    assert!(
        std::ptr::eq(registration, borrowed),
        "borrow the original proof graph"
    );
    iroha_parliament_ballot::registration::verify_registration_v1(
        context,
        signed.authority(),
        &assignment,
        &registration.registration,
    )
    .expect("the exact retrieved proof verifies again");

    for coordinate in 0..5 {
        let mut changed = *record;
        match coordinate {
            0 => changed.source.height = 0,
            1 => changed.source.height += 1,
            2 => changed.source.input_index = u64::MAX,
            3 => changed.source.instruction_index = 1,
            _ => {
                changed.source.entrypoint_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"another source input"));
            }
        }
        assert_eq!(
            resolve_credential_registration_v1(&original, context, assignment, &changed),
            Err(CredentialHistoryErrorV1::Source),
            "changed source coordinate {coordinate}"
        );
    }
    for binding in 0..6 {
        let mut changed_context = *context;
        let mut changed_assignment = assignment;
        let mut changed_record = *record;
        match binding {
            0 => {
                changed_context.network_id = NetworkId::from_genesis_hash(
                    HashOf::from_untyped_unchecked(Hash::new(b"another registration network")),
                );
            }
            1 => changed_context.governance_attempt_id = GovernanceAttemptId::new([0x91; 32]),
            2 => changed_context.election_attempt_id = BodyElectionAttemptId::new([0x92; 32]),
            3 => changed_assignment = AssignmentId::new([0x93; 32]),
            4 => changed_record.commitment = CredentialCommitmentV1::new([0x94; 128]),
            _ => changed_record.authorization = RegistrationAuthorizationV1::new([0x95; 64]),
        }
        assert_eq!(
            resolve_credential_registration_v1(
                &original,
                &changed_context,
                changed_assignment,
                &changed_record,
            ),
            Err(CredentialHistoryErrorV1::Binding),
            "changed registration binding {binding}"
        );
    }

    // A proof in a signed, finalized input that was rejected is no registration evidence.
    let refused = history
        .authenticated_execution(record.source.height - 1)
        .unwrap();
    let index = refused
        .block()
        .network_entrypoints()
        .position(|input| input.hash() == rejected.hash_as_entrypoint())
        .unwrap();
    let mut changed = *record;
    changed.source.height = refused.committed().height();
    changed.source.input_index = index as u64;
    changed.source.entrypoint_hash = rejected.hash_as_entrypoint();
    assert_eq!(
        resolve_credential_registration_v1(&refused, context, assignment, &changed),
        Err(CredentialHistoryErrorV1::NotApplied)
    );
}

#[test]
fn canonical_history_resolution_refuses_sealed_inputs_and_unrelated_signed_work() {
    let config = TestChainConfig::new(World::new(), 1_000);
    let key = config.genesis_key.clone();
    let mut chain = CertifiedTestChain::start(config).unwrap();
    let signed = chain.sign(
        &key,
        [Log::new(Level::INFO, "registration history control".into()).into()],
        1_001,
    );
    let sealed = TransactionEntrypoint::SealedCommitment(SignedSealedTransactionCommitment::sign(
        SealedTransactionCommitmentPayload::new(
            chain.network_id(),
            signed.authority().clone(),
            Hash::new(b"not a credential registration"),
            3,
            5,
            None,
        ),
        key.private_key(),
    ));
    chain.commit_entrypoints(vec![sealed, TransactionEntrypoint::External(signed)]);
    let view = chain.state().view();
    let history = CertifiedChain::new(&view).unwrap();
    let original = history.authenticated_execution(2).unwrap();
    let context = BallotBodyContextV1 {
        network_id: chain.network_id(),
        governance_attempt_id: GovernanceAttemptId::new([1; 32]),
        election_attempt_id: BodyElectionAttemptId::new([2; 32]),
        body_role: ParliamentBody::PolicyJury,
    };
    assert_eq!(original.block().network_entrypoint_count(), 2);
    assert_eq!(original.block().external_transactions().len(), 1);
    for (index, input) in original.block().network_entrypoints().enumerate() {
        let record = ParliamentCredentialRecordV1 {
            commitment: CredentialCommitmentV1::new([3; 128]),
            authorization: RegistrationAuthorizationV1::new([4; 64]),
            source: ParliamentCredentialSourceV1 {
                height: 2,
                input_index: index as u64,
                instruction_index: 0,
                entrypoint_hash: input.hash(),
            },
        };
        assert_eq!(
            resolve_credential_registration_v1(
                &original,
                &context,
                AssignmentId::new([5; 32]),
                &record,
            ),
            Err(CredentialHistoryErrorV1::Instruction),
            "complete network index {index}, not the signed-only projection"
        );
    }
}
