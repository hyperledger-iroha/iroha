//! Signerless representation and its stateless boundaries, without proof-validity claims:
//! acceptance has no stateless verdict, no lane carries a ballot, the instruction executor
//! never runs one and it spends no ordinary gas. Proof-bearing admission and execution are
//! tested with genuine ballots in `queue::anonymous_ballots` and
//! `governance::parliament_anonymous::ballot_input`.

use super::*;
use crate::queue::TransactionRoutingView as _;
use crate::sumeragi::lanes::{TransactionCheck as _, global::StatelessChecks};
use iroha_data_model::governance::anonymous_ballot::{
    AdmissionNonceV1, AdmissionStampV1, AnonymousBallotEnvelopeV1, AnonymousBallotV1,
    BallotChoiceV1, BallotProofBytesV1, BallotScopeV1, CanonicalAnonymousBallotEnvelopeV1,
    NullifierV1, ProofProfileIdV1, VoteAuthorizationV1,
};

/// Canonical structural input only: these public proof bytes and nonce are not valid evidence.
pub fn entrypoint() -> TransactionEntrypoint {
    let envelope = AnonymousBallotEnvelopeV1 {
        network_id: crate::unit_test_support::synthetic_network_id("anonymous representation"),
        ballot: AnonymousBallotV1 {
            scope: BallotScopeV1::new([0x44; 64]),
            vote: BallotChoiceV1::Aye,
            nullifier: NullifierV1::new([7; 128]),
            auth_v: VoteAuthorizationV1::new([0x55; 64]),
            profile_id: ProofProfileIdV1::V1,
            stamp: AdmissionStampV1 {
                nonce: AdmissionNonceV1::new([0x66; 32]),
            },
            proof: BallotProofBytesV1::new(vec![0x6b; BallotProofBytesV1::LENGTH]).unwrap(),
        },
    };
    TransactionEntrypoint::AnonymousBallot(
        CanonicalAnonymousBallotEnvelopeV1::from_envelope(&envelope).unwrap(),
    )
}

#[test]
fn signerless_representation_has_no_signed_authority_or_metadata_cache() {
    let entry = entrypoint();
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(&entry));
    assert!(accepted.authority_opt().is_none());
    assert!(accepted.external().is_none());
    assert!(accepted.metadata().is_none());
    assert!(accepted.time_to_live().is_none());
    assert!(accepted.prepared_metadata().is_none());
    assert!(accepted.stateless_cache_metadata().is_none());
    assert!(accepted.signed_bytes().is_none());
    assert!(exact_signed_transaction_hash(&entry).is_none());
    assert!(accepted.is_anonymous_ballot());
    assert_eq!(accepted.hash_as_entrypoint(), entry.hash());
    assert_eq!(
        accepted.entrypoint_bytes().as_slice(),
        norito::encode_canonical(&entry).unwrap()
    );
    let TransactionEntrypoint::AnonymousBallot(carrier) = &entry else {
        unreachable!()
    };
    assert_eq!(
        accepted.encoded_len(),
        norito::encode_canonical(carrier).unwrap().len()
    );
}

#[test]
fn signerless_acceptance_has_no_stateless_verdict_and_no_lane_carries_a_ballot() {
    let entry = entrypoint();
    let network = crate::unit_test_support::synthetic_network_id("anonymous representation");
    // Even this structurally valid but unproved ballot is accepted statelessly: its board
    // decides it, at queue admission and at execution, never a stateless rejection that a
    // block could commit.
    for signature_prechecked in [false, true] {
        let accepted = AcceptedTransaction::accept_borrowed_entrypoint_at_time(
            &entry,
            &network,
            Duration::ZERO,
            TransactionParameters::default(),
            &iroha_data_model::crypto_admission_policy::CryptoAdmissionPolicyV1::new(vec![
                iroha_crypto::Algorithm::Ed25519,
            ])
            .expect("explicit signerless structural fixture policy"),
            Duration::ZERO,
            signature_prechecked,
        )
        .expect("a ballot has no stateless verdict");
        assert_eq!(accepted.entrypoint(), &entry);
        assert!(accepted.is_anonymous_ballot());
    }
    // A lane block that carries one is refused with a verdict.
    assert!(matches!(
        StatelessChecks::new(network).check(&entry, 0),
        Err(ExecutionAttemptError::Rejected(reason))
            if reason == "an anonymous ballot executes only on the global chain"
    ));
    // The instruction executor never runs one.
    assert_eq!(
        anonymous_ballot_outside_its_owner().reason(),
        ivm::error::ExecutionDeferral::LocalInvariantViolation
    );
    // It pays no fee and spends no ordinary gas: the per-block ballot budget bounds it.
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Borrowed(&entry));
    assert!(matches!(
        crate::queue::Queue::compute_proposal_gas_cost(
            &accepted,
            &iroha_config::parameters::actual::ConfidentialGas {
                proof_base: 1,
                per_public_input: 1,
                per_proof_byte: 1,
                per_nullifier: 1,
                per_commitment: 1,
            },
        ),
        Ok(0)
    ));
}

#[test]
fn the_ballot_entrypoint_frame_is_bounded_by_its_wire_cap() {
    use iroha_data_model::governance::anonymous_ballot::ANONYMOUS_BALLOT_ENTRYPOINT_WIRE_MAX_BYTES_V1 as E_WIRE;
    AcceptedTransaction::anonymous_ballot_wire_within_cap(E_WIRE).expect("the cap itself fits");
    // One byte more is a completed limit verdict, never a local refusal.
    let refusal = AcceptedTransaction::anonymous_ballot_wire_within_cap(E_WIRE + 1)
        .expect_err("one byte above E_wire");
    assert!(
        matches!(
            &refusal,
            AcceptTransactionFail::TransactionLimit(limit)
                if limit.reason
                    == "anonymous-ballot entrypoint wire has 263169 encoded bytes; at most 263168 are allowed"
        ),
        "{refusal:?}"
    );
    assert!(matches!(
        execution_attempt_from_admission_failure(refusal),
        ExecutionAttemptError::Rejected(TransactionRejectionReason::LimitCheck(_))
    ));
    // A genuine carrier is far below the cap, so acceptance still completes for it.
    assert!(AcceptedTransaction::entrypoint_encoded_len(&entrypoint()) <= E_WIRE);
}
