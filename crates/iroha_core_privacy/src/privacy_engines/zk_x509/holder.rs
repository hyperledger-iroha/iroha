//! Holder-owned inputs and complete native X.509 credential proving.
//!
//! The wallet retains its certificate subject signing key and supplies only the
//! fresh ownership signature, certificate chain, CRL, membership path and private
//! attribute openings. Creating a witness checks its bounded shape; proving still
//! performs canonical decoding, authoritative-state and reference-relation checks,
//! constructs the complete MAIN/CA proof and independently verifies the result.
//!
//! [`ZkX509HolderAttemptV1`] owns the exact statement and witness, checks signed
//! deadlines before proving, and returns an opaque pending proof. Consuming release
//! rechecks current authenticated State, time and network before exposing bytes.
//!
//! The prover consumes the witness and clears its owned serialization on every
//! return or unwind. Copies retained by a caller remain that caller's responsibility.
//! An authorized proving host receives the private witness: this local API does not
//! establish enrollment, an encrypted channel, replay protection or host revocation.

mod lifecycle;
pub use lifecycle::{
    ZkX509HolderAttemptErrorV1, ZkX509HolderAttemptV1, ZkX509PendingCredentialProofV1,
};

pub use super::{
    codec::{ZkX509AttributeOpeningV1, ZkX509WitnessCodecErrorV1, ZkX509WitnessV1},
    engine::{ZkX509EngineErrorV1, verify_zk_x509_credential_proof_v1},
    errors::ZkX509RelationErrorV1,
};
use crate::privacy_state::PrivacyZkX509AuthoritativeStateV1;
use iroha_data_model::privacy::{IrohaZkX509StarkP256StatementV1, PrivacyConsensusLimitsV1};
use rand::TryCryptoRng;
use zeroize::Zeroizing;

/// Encode the exact public message for a wallet's P-256/SHA-256 signer.
///
/// Pass these bytes directly to a message-signing API such as `SHA256withECDSA`.
/// It applies SHA-256 once. Normalize the returned ECDSA `s` to its low half and
/// supply its fixed 64-byte `r || s` representation to the witness constructor.
/// A prehash-signing API instead consumes [`ownership_challenge_v1`], which is
/// exactly SHA-256 of this message. Neither function accepts the subject key.
///
/// # Errors
///
/// Rejects a statement or account that cannot be encoded canonically, or a
/// framing allocation failure. These public bytes do not authenticate the
/// statement's governance records or authorize a transaction on their own.
pub fn ownership_signing_message_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<Vec<u8>, ZkX509RelationErrorV1> {
    super::relation::derive_ownership_signing_message_v1(statement)
}

/// Derive the exact digest the wallet's certificate subject key must sign.
///
/// Sign this digest using P-256 prehash signing, normalize `s` to its low half,
/// and pass the fixed 64-byte `r || s` signature into the witness constructor.
/// For a message-signing API that hashes internally, use
/// [`ownership_signing_message_v1`] instead; signing this digest as a message
/// would hash it twice and produce an invalid ownership signature.
/// This is the same statement/account/challenge/transaction binding checked by
/// the native relation and constrained by the complete credential proof.
///
/// # Errors
///
/// Rejects a statement or account that cannot be encoded in its canonical form.
/// This derivation does not authenticate the statement's governance records.
pub fn ownership_challenge_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<[u8; 32], ZkX509RelationErrorV1> {
    super::relation::derive_ownership_challenge_digest_v1(statement)
}

/// Produce and self-verify one canonical credential proof, consuming private inputs.
///
/// The signing key never enters this API. The wallet must create the fresh ownership
/// signature for this exact statement before constructing `witness`. The caller
/// supplies authenticated state and trusted chain time; a proving host cannot select
/// its own governance records or relax the proof's presentation window.
///
/// # Errors
///
/// Returns the native engine's exact witness, state, relation, entropy, resource or
/// proof-construction failure. No partial proof is returned. The caller retains
/// ownership of `rng`; failed construction may consume a prefix of its entropy.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_credential_v1_with_rng<R: TryCryptoRng>(
    statement: &IrohaZkX509StarkP256StatementV1,
    authoritative_state: &PrivacyZkX509AuthoritativeStateV1,
    trusted_block_timestamp_ms: u64,
    consensus_limits: &PrivacyConsensusLimitsV1,
    genesis_hash: [u8; 32],
    witness: ZkX509WitnessV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509EngineErrorV1> {
    let encoded_witness = Zeroizing::new(witness.encode_v1()?);
    super::engine::prove_zk_x509_credential_proof_v1_with_rng(
        statement,
        authoritative_state,
        trusted_block_timestamp_ms,
        consensus_limits,
        genesis_hash,
        &encoded_witness,
        rng,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct PreflightEntropy {
        requests: usize,
    }

    impl rand::TryRngCore for PreflightEntropy {
        type Error = &'static str;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            self.requests += 1;
            Err("preflight must not request entropy")
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.requests += 1;
            Err("preflight must not request entropy")
        }

        fn try_fill_bytes(&mut self, _destination: &mut [u8]) -> Result<(), Self::Error> {
            self.requests += 1;
            Err("preflight must not request entropy")
        }
    }

    impl rand::TryCryptoRng for PreflightEntropy {}

    #[test]
    fn ownership_digest_matches_wallet_signature_and_binds_fresh_challenge() {
        use p256::ecdsa::{
            Signature, VerifyingKey,
            signature::{Verifier as _, hazmat::PrehashVerifier as _},
        };
        use sha2::{Digest as _, Sha256};

        let fixture = super::super::relation::release_fixture::build_zk_x509_reference_fixture_v1()
            .expect("canonical signed credential fixture");
        let certificate = super::super::certificate::parse_certificate_v1(
            &fixture.witness.certificate_chain_der[0],
        )
        .expect("signed leaf certificate");
        let key = VerifyingKey::from_sec1_bytes(certificate.public_key).expect("P-256 subject key");
        let signature = Signature::from_slice(&fixture.witness.wallet_ownership_signature_rs)
            .expect("canonical ownership signature");
        let digest = ownership_challenge_v1(&fixture.statement).expect("ownership digest");
        let message = ownership_signing_message_v1(&fixture.statement).expect("signing message");
        assert_eq!(<[u8; 32]>::from(Sha256::digest(&message)), digest);
        key.verify(&message, &signature)
            .expect("existing wallet prehash signature verifies over the public message");
        key.verify_prehash(&digest, &signature)
            .expect("wallet signature matches the public holder API");
        let mut variants = Vec::new();
        let mut changed = fixture.statement.clone();
        let mut challenge = *changed.wallet_challenge.as_bytes();
        challenge[0] ^= 1;
        changed.wallet_challenge = iroha_data_model::privacy::PrivacyChallengeV1::new(challenge);
        variants.push(("fresh wallet challenge", changed));
        let mut changed = fixture.statement.clone();
        changed.context.transaction_intent_digest =
            iroha_data_model::privacy::PrivacyTransactionIntentDigestV1::new([0x77; 32]);
        variants.push(("transaction intent", changed));
        let mut changed = fixture.statement.clone();
        let other_wallet =
            iroha_crypto::KeyPair::try_from_seed(vec![0x62; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("deterministic second wallet");
        changed.wallet_account =
            iroha_data_model::account::AccountId::new(other_wallet.public_key().clone());
        variants.push(("wallet account", changed));
        let mut changed = fixture.statement.clone();
        let mut genesis = *changed.context.network_id.as_bytes();
        genesis[0] ^= 1;
        changed.context.network_id = iroha_data_model::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(genesis)),
        );
        variants.push(("network", changed));
        let mut changed = fixture.statement.clone();
        changed.trust_anchor_record_epoch += 1;
        variants.push(("governance revision", changed));
        let mut changed = fixture.statement.clone();
        changed.presentation_not_before_unix_seconds += 1;
        variants.push(("public window", changed));
        for (field, changed) in variants {
            let changed_digest =
                ownership_challenge_v1(&changed).expect("changed ownership digest");
            assert_ne!(digest, changed_digest, "{field} must be bound");
            let changed_message =
                ownership_signing_message_v1(&changed).expect("changed ownership message");
            assert_ne!(message, changed_message, "{field} message must be bound");
            assert_eq!(
                <[u8; 32]>::from(Sha256::digest(&changed_message)),
                changed_digest
            );
            assert!(
                key.verify(&changed_message, &signature).is_err(),
                "{field} message substitution"
            );
            assert!(
                key.verify_prehash(&changed_digest, &signature).is_err(),
                "{field} substitution"
            );
        }
    }

    #[test]
    fn message_and_prehash_signers_bind_the_same_certificate_ownership() {
        use p256::ecdsa::{
            Signature, SigningKey,
            signature::{
                Signer as _, Verifier as _,
                hazmat::{PrehashSigner as _, PrehashVerifier as _},
            },
        };

        let mut fixture =
            super::super::relation::release_fixture::build_zk_x509_reference_fixture_v1()
                .expect("canonical signed credential fixture");
        // Public deterministic test-fixture key; production holders retain their key.
        let mut scalar = [0u8; 32];
        scalar[31] = 2;
        let signer = SigningKey::from_slice(&scalar).expect("fixture leaf signing key");
        let key = signer.verifying_key();
        let certificate = super::super::certificate::parse_certificate_v1(
            &fixture.witness.certificate_chain_der[0],
        )
        .expect("signed leaf certificate");
        assert_eq!(
            key.to_encoded_point(false).as_bytes(),
            certificate.public_key
        );
        let message = ownership_signing_message_v1(&fixture.statement).expect("signing message");
        let digest = ownership_challenge_v1(&fixture.statement).expect("ownership digest");
        let message_signature: Signature = signer.sign(&message);
        let message_signature = message_signature.normalize_s().unwrap_or(message_signature);
        let prehash_signature: Signature = signer.sign_prehash(&digest).expect("prehash signature");
        let prehash_signature = prehash_signature.normalize_s().unwrap_or(prehash_signature);
        key.verify_prehash(&digest, &message_signature)
            .expect("message signer hashes the canonical message exactly once");
        key.verify(&message, &prehash_signature)
            .expect("prehash signer signs the same public message");

        let trust_anchor = fixture.authoritative_state.trust_anchor();
        let crl = fixture.authoritative_state.crl_record();
        let governance = super::super::relation::ZkX509GovernanceV1 {
            trust_anchor: &trust_anchor,
            certificate_policy: fixture.authoritative_state.certificate_policy(),
            crl: &crl,
        };
        for signature in [message_signature, prehash_signature] {
            fixture.witness.wallet_ownership_signature_rs = signature.to_bytes().into();
            super::super::relation::validate_reference_relation_v1(
                &fixture.statement,
                governance,
                &fixture.witness,
            )
            .expect("complete native relation accepts both signing API forms");
        }

        let double_hashed: Signature = signer.sign(&digest);
        let double_hashed = double_hashed.normalize_s().unwrap_or(double_hashed);
        assert!(key.verify_prehash(&digest, &double_hashed).is_err());
        assert!(key.verify(&message, &double_hashed).is_err());
        fixture.witness.wallet_ownership_signature_rs = double_hashed.to_bytes().into();
        assert_eq!(
            super::super::relation::validate_reference_relation_v1(
                &fixture.statement,
                governance,
                &fixture.witness,
            ),
            Err(ZkX509RelationErrorV1::InvalidWalletOwnership)
        );
    }

    #[test]
    fn owned_holder_prover_preserves_genesis_preflight_before_entropy() {
        let fixture = super::super::relation::release_fixture::build_zk_x509_reference_fixture_v1()
            .expect("canonical signed credential fixture");
        let mut rng = PreflightEntropy::default();
        let mut wrong_genesis = *fixture.statement.context.network_id.as_bytes();
        wrong_genesis[0] ^= 1;
        assert_eq!(
            verify_zk_x509_credential_proof_v1(
                &fixture.statement,
                &fixture.authoritative_state,
                wrong_genesis,
                &[],
            ),
            Err(ZkX509EngineErrorV1::CredentialProof(
                super::super::credential_stark::ZkX509CredentialProofErrorV1::InvalidStatement
            ))
        );
        let error = prove_credential_v1_with_rng(
            &fixture.statement,
            &fixture.authoritative_state,
            fixture.statement.presentation_not_before_unix_seconds * 1_000,
            &PrivacyConsensusLimitsV1::taira_default(),
            wrong_genesis,
            fixture.witness,
            &mut rng,
        )
        .expect_err("wrong network must fail before the real prover consumes entropy");
        assert_eq!(
            error,
            ZkX509EngineErrorV1::CredentialProof(
                super::super::credential_stark::ZkX509CredentialProofErrorV1::InvalidStatement
            )
        );
        assert_eq!(rng.requests, 0);
    }
}
