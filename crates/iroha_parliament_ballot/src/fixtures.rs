//! Reproducible ballots for the tests of crates that integrate this one (feature
//! `test-fixtures`).
//!
//! A genuine ballot needs a fresh proving tape and a stamp of at least 28 bits, about `2^28`
//! SHA3-512 evaluations: neither is reproducible, and the search is far too slow for a debug
//! test. A test of the node's State, queue or API therefore proves with a **fixed public
//! tape** and pins the stamp nonce it mined once for exactly those bytes. Nothing else is
//! relaxed: the proof is the candidate row's real proof of the real statement, and the node
//! verifies the pinned stamp at the frozen difficulty on every run.
//!
//! **Never enable this feature in a wallet, a node or any other shipped build.** A tape that
//! is fixed or reused reveals the credential secret it proves with; the wallet path
//! ([`crate::wallet`]) draws every tape from the operating system and has no entry that
//! takes one. These functions exist only so that a dependent crate's tests can reproduce
//! ballot bytes for a context they derive themselves; this crate's own tests keep their
//! fixtures in `test_support`.

use iroha_data_model::governance::anonymous_ballot::{
    AdmissionNonceV1, AdmissionStampV1, AnonymousBallotAuthorizationV1,
    AnonymousBallotCodecErrorV1, AnonymousBallotEnvelopeV1, AnonymousBallotV1, BallotChoiceV1,
    BallotProofBytesV1, CanonicalAnonymousBallotEnvelopeV1, ProofProfileIdV1,
};
use iroha_parliament_ballot_proof::{
    proving::{ProvingTape, TAPE_BYTES},
    vote,
};

use crate::{
    PROOF_ROW_V1,
    context::ScopedBallotContextV1,
    inputs::ProofValueV1,
    wallet::{CREDENTIAL_SECRET_BYTES, CredentialHandleV1, WalletErrorV1},
};

/// The authorization (every field of a ballot but its stamp) of `vote` in `context`'s scope
/// by the credential `secret`, proved with the fixed public `tape`.
///
/// The same arguments always give the same bytes, so a stamp nonce mined for them stays
/// valid until one of them changes.
///
/// # Errors
/// [`WalletErrorV1::NotOnSealedList`] for a credential the body did not seal, and input or
/// prover errors ([`WalletErrorV1::Prove`] when this tape exhausts its counter range:
/// choose another).
pub fn fixture_authorization_v1(
    context: &ScopedBallotContextV1,
    secret: &[u8; CREDENTIAL_SECRET_BYTES],
    vote: BallotChoiceV1,
    tape: &[u8; TAPE_BYTES],
) -> Result<AnonymousBallotAuthorizationV1, WalletErrorV1> {
    let handle = CredentialHandleV1::import_v1(*context.body_context(), secret, None)?;
    let position = handle.position(context)?;
    let statement = handle.statement(context, vote)?;
    let proof = vote::prove(
        PROOF_ROW_V1,
        &statement,
        handle.secret(),
        position,
        ProvingTape::from_bytes(tape),
    )?;
    Ok(AnonymousBallotAuthorizationV1 {
        network_id: *context.network_id(),
        scope: *context.scope(),
        vote,
        nullifier: ProofValueV1::from_proof(&statement.nullifier),
        auth_v: ProofValueV1::from_proof(&statement.auth_v),
        profile_id: ProofProfileIdV1::V1,
        proof: BallotProofBytesV1::new(proof)?,
    })
}

/// The canonical envelope of `authorization` stamped with `nonce`. The stamp is not checked
/// here: the node decides it, as for any submission.
///
/// # Errors
/// [`AnonymousBallotCodecErrorV1`] if the envelope cannot be framed.
pub fn fixture_envelope_v1(
    authorization: &AnonymousBallotAuthorizationV1,
    nonce: [u8; AdmissionNonceV1::LENGTH],
) -> Result<CanonicalAnonymousBallotEnvelopeV1, AnonymousBallotCodecErrorV1> {
    CanonicalAnonymousBallotEnvelopeV1::from_envelope(&AnonymousBallotEnvelopeV1 {
        network_id: authorization.network_id,
        ballot: AnonymousBallotV1 {
            scope: authorization.scope,
            vote: authorization.vote,
            nullifier: authorization.nullifier,
            auth_v: authorization.auth_v,
            profile_id: authorization.profile_id,
            stamp: AdmissionStampV1 {
                nonce: AdmissionNonceV1::new(nonce),
            },
            proof: authorization.proof.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ballot::verify_ballot_proof_v1,
        test_support::{
            AYE_0, FIXTURE_BALLOTS, fixture_authorization, fixture_bytes, scoped_context,
            secret_bytes, tape,
        },
    };

    #[test]
    fn a_fixed_tape_reproduces_this_crates_own_pinned_fixture() {
        let context = scoped_context();
        let ballot = &FIXTURE_BALLOTS[AYE_0];
        let authorization = fixture_authorization_v1(
            &context,
            &secret_bytes(ballot.member),
            ballot.vote,
            &tape(ballot.tape),
        )
        .expect("a sealed credential proves");
        // The wallet-independent fixture of `test_support` is byte-identical, so the nonce
        // pinned for it stamps this envelope too.
        assert_eq!(authorization, fixture_authorization(AYE_0));
        let envelope = fixture_envelope_v1(&authorization, ballot.nonce).expect("canonical");
        assert_eq!(envelope.as_bytes(), fixture_bytes(AYE_0));
        verify_ballot_proof_v1(&context, &envelope.envelope().expect("decodes"))
            .expect("the proof verifies");
    }

    #[test]
    fn a_credential_outside_the_sealed_body_proves_nothing() {
        let context = scoped_context();
        assert!(matches!(
            fixture_authorization_v1(&context, &[0x7f; 48], BallotChoiceV1::Aye, &tape(0x10)),
            Err(WalletErrorV1::NotOnSealedList)
        ));
    }
}
