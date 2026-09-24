use super::super::{
    BfvSecretKey, decode_rounded_plaintext, encrypt_bounded_noise_from_seed,
    keygen_bounded_noise_from_seed, poly_mul_mod, poly_neg_mod, ram_lfe_bfv_parameters_v1,
};
use super::*;
use crate::{Algorithm, KeyPair};

struct EightPartyFixture {
    statement: BfvEightPartyDecryptionStatementV1,
    contributions:
        [BfvEightPartySignedDecryptionContributionV1; BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1],
    signing_keys: Vec<KeyPair>,
    secrets: Vec<BfvSecretKey>,
    private_shares: Vec<Vec<u64>>,
}

fn fixture_contribution_commitment(statement_digest: Hash, index: usize, share: &[u64]) -> Hash {
    let encoded = norito::encode_canonical(&share.to_vec()).expect("encode private fixture share");
    Hash::new_from_chunks(&[
        b"bfv-eight-party-test-only-private-share-commitment",
        statement_digest.as_ref(),
        &[index as u8],
        encoded.as_slice(),
    ])
}

fn eight_party_fixture() -> EightPartyFixture {
    let params = ram_lfe_bfv_parameters_v1();
    let mut parties = Vec::new();
    let mut common_a = None;
    for index in 0..BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1 {
        let seed = format!("bfv-eight-party-independent-secret-{index}");
        let (secret, public) = keygen_bounded_noise_from_seed(&params, seed.as_bytes())
            .expect("generate independent BFV secret");
        let a = common_a.get_or_insert_with(|| public.a.clone());
        let public_key_share_b = poly_neg_mod(&params, &poly_mul_mod(&params, a, &secret.s));
        let signing_key = KeyPair::from_seed(vec![index as u8 + 1; 32], Algorithm::Ed25519);
        parties.push((signing_key, secret, public_key_share_b));
    }
    parties.sort_by(|left, right| left.0.public_key().cmp(right.0.public_key()));
    let mut aggregate_b = vec![0; usize::from(params.polynomial_degree)];
    let mut members = Vec::new();
    for (key, _, b) in &parties {
        aggregate_b = poly_add_mod(&params, &aggregate_b, b);
        members.push(BfvEightPartyDecryptionMemberV1 {
            signing_public_key: key.public_key().clone(),
            public_key_share_b: b
                .clone()
                .try_into()
                .expect("registered 64-coefficient share"),
        });
    }
    let aggregate_public_key = BfvPublicKey {
        b: aggregate_b,
        a: common_a.expect("common a"),
    };
    let mut plaintext = vec![0; usize::from(params.polynomial_degree)];
    plaintext[0] = 7;
    let ciphertext =
        encrypt_bounded_noise_from_seed(&params, &aggregate_public_key, &plaintext, &[0x52; 32])
            .expect("encrypt fixture ciphertext");
    let statement = BfvEightPartyDecryptionStatementV1 {
        version: BFV_EIGHT_PARTY_DECRYPTION_VERSION_V1,
        session_id: [0x53; 32],
        parameters: params,
        aggregate_public_key,
        ciphertext,
        members: members.try_into().expect("exact eight-member roster"),
    };
    let digest =
        bfv_eight_party_decryption_statement_digest_v1(&statement).expect("freeze statement");
    let mut contributions = Vec::new();
    let mut private_shares = Vec::new();
    for (index, (key, secret, _)) in parties.iter().enumerate() {
        let private_share = poly_mul_mod(&params, &statement.ciphertext.c1, &secret.s);
        let payload = BfvEightPartyDecryptionContributionPayloadV1 {
            version: BFV_EIGHT_PARTY_DECRYPTION_VERSION_V1,
            session_id: statement.session_id,
            statement_digest: digest,
            participant_index: index as u8,
            contribution_commitment: fixture_contribution_commitment(digest, index, &private_share),
        };
        private_shares.push(private_share);
        contributions.push(BfvEightPartySignedDecryptionContributionV1 {
            signature: SignatureOf::try_new(key.private_key(), &payload)
                .expect("independent participant signature"),
            payload,
        });
    }
    let (signing_keys, secrets): (Vec<_>, Vec<_>) = parties
        .into_iter()
        .map(|(key, secret, _)| (key, secret))
        .unzip();
    EightPartyFixture {
        statement,
        contributions: contributions.try_into().expect("exact eight contributions"),
        signing_keys,
        secrets,
        private_shares,
    }
}

#[test]
fn eight_independent_parties_authenticate_but_cannot_release_plaintext() {
    let fixture = eight_party_fixture();
    assert_eq!(
        fixture.secrets.len(),
        BFV_EIGHT_PARTY_DECRYPTION_PARTICIPANTS_V1
    );
    for (index, secret) in fixture.secrets.iter().enumerate() {
        assert!(fixture.secrets[..index].iter().all(|prior| prior != secret));
    }
    let params = &fixture.statement.parameters;
    let mut combined_shares = vec![0; usize::from(params.polynomial_degree)];
    let mut combined_secret = vec![0; usize::from(params.polynomial_degree)];
    for (share, secret) in fixture.private_shares.iter().zip(&fixture.secrets) {
        combined_shares = poly_add_mod(params, &combined_shares, share);
        combined_secret = poly_add_mod(params, &combined_secret, &secret.s);
    }
    assert_eq!(
        combined_shares,
        poly_mul_mod(params, &fixture.statement.ciphertext.c1, &combined_secret),
        "fixture must exercise eight independent BFV secret shares",
    );
    let scaled = poly_add_mod(params, &fixture.statement.ciphertext.c0, &combined_shares);
    assert_eq!(
        decode_rounded_plaintext(params, &scaled).expect("fixture decode")[0],
        7
    );
    validate_bfv_eight_party_decryption_authentication_v1(
        &fixture.statement,
        &fixture.contributions,
    )
    .expect("authenticate eight independent signers");
    assert_eq!(
        verify_bfv_eight_party_decryption_v1(&fixture.statement, &fixture.contributions),
        Err(BfvError::EightPartyShareRelationProofUnavailable),
    );
}

#[test]
fn roster_missing_replay_wrong_signer_and_late_share_mutations_fail_authentication() {
    let fixture = eight_party_fixture();
    let mut missing = fixture.contributions.to_vec();
    missing.pop();
    assert!(
        validate_bfv_eight_party_decryption_authentication_v1(&fixture.statement, &missing)
            .is_err()
    );

    let mut duplicate = fixture.contributions.clone();
    duplicate[7] = duplicate[0].clone();
    assert!(
        validate_bfv_eight_party_decryption_authentication_v1(&fixture.statement, &duplicate)
            .is_err()
    );

    let mut replay_statement = fixture.statement.clone();
    replay_statement.session_id[31] ^= 1;
    assert!(
        validate_bfv_eight_party_decryption_authentication_v1(
            &replay_statement,
            &fixture.contributions
        )
        .is_err()
    );

    let mut different_ciphertext = fixture.statement.clone();
    different_ciphertext.ciphertext.c0[0] = (different_ciphertext.ciphertext.c0[0] + 1)
        % different_ciphertext.parameters.ciphertext_modulus;
    assert!(
        validate_bfv_eight_party_decryption_authentication_v1(
            &different_ciphertext,
            &fixture.contributions,
        )
        .is_err()
    );

    let mut wrong_signer = fixture.contributions.clone();
    wrong_signer[7].signature = SignatureOf::try_new(
        fixture.signing_keys[0].private_key(),
        &wrong_signer[7].payload,
    )
    .expect("sign wrong-party declaration");
    assert!(
        validate_bfv_eight_party_decryption_authentication_v1(&fixture.statement, &wrong_signer)
            .is_err()
    );

    let mut forged_late_share = fixture.contributions.clone();
    forged_late_share[7].payload.contribution_commitment = Hash::new(b"forged late share");
    assert!(
        validate_bfv_eight_party_decryption_authentication_v1(
            &fixture.statement,
            &forged_late_share
        )
        .is_err()
    );

    let mut signed_invalid_share = fixture.contributions.clone();
    let mut invalid_private_share = fixture.private_shares[7].clone();
    invalid_private_share[63] ^= 1;
    signed_invalid_share[7].payload.contribution_commitment = fixture_contribution_commitment(
        signed_invalid_share[7].payload.statement_digest,
        7,
        &invalid_private_share,
    );
    signed_invalid_share[7].signature = SignatureOf::try_new(
        fixture.signing_keys[7].private_key(),
        &signed_invalid_share[7].payload,
    )
    .expect("malicious participant can sign its own false share");
    validate_bfv_eight_party_decryption_authentication_v1(
        &fixture.statement,
        &signed_invalid_share,
    )
    .expect("authentication alone cannot reject an algebraically false share");
    assert_eq!(
        verify_bfv_eight_party_decryption_v1(&fixture.statement, &signed_invalid_share),
        Err(BfvError::EightPartyShareRelationProofUnavailable),
    );
}

#[test]
fn invalid_roster_aggregate_and_polynomial_shapes_are_rejected() {
    let fixture = eight_party_fixture();
    let mut duplicate_key = fixture.statement.clone();
    duplicate_key.members[7].signing_public_key =
        duplicate_key.members[0].signing_public_key.clone();
    assert!(validate_bfv_eight_party_decryption_statement_v1(&duplicate_key).is_err());

    let mut wrong_aggregate = fixture.statement.clone();
    wrong_aggregate.aggregate_public_key.b[63] ^= 1;
    assert!(validate_bfv_eight_party_decryption_statement_v1(&wrong_aggregate).is_err());

    let mut zero_session = fixture.statement.clone();
    zero_session.session_id = [0; 32];
    assert!(validate_bfv_eight_party_decryption_statement_v1(&zero_session).is_err());

    let mut over_modulus = fixture.statement.clone();
    over_modulus.members[7].public_key_share_b[63] =
        fixture.statement.parameters.ciphertext_modulus;
    assert!(validate_bfv_eight_party_decryption_statement_v1(&over_modulus).is_err());
}

#[test]
fn canonical_norito_roundtrip_preserves_exact_statement_and_signatures() {
    let fixture = eight_party_fixture();
    let statement_bytes = norito::encode_canonical(&fixture.statement).expect("encode statement");
    let decoded_statement = decode_bfv_eight_party_decryption_statement_bytes_v1(&statement_bytes)
        .expect("decode bounded statement");
    let contributions_bytes =
        norito::encode_canonical(&fixture.contributions).expect("encode contributions");
    let decoded_contributions =
        decode_bfv_eight_party_decryption_contributions_bytes_v1(&contributions_bytes)
            .expect("decode bounded contributions");
    assert_eq!(decoded_statement, fixture.statement);
    assert_eq!(decoded_contributions, fixture.contributions);
    validate_bfv_eight_party_decryption_authentication_v1(
        &decoded_statement,
        &decoded_contributions,
    )
    .expect("canonical roundtrip keeps signatures and context bound");
}

#[test]
fn bounded_norito_decoders_reject_oversize_and_wrong_count_inputs() {
    let fixture = eight_party_fixture();
    let oversized = vec![0_u8; BFV_EIGHT_PARTY_STATEMENT_MAX_BYTES_V1 + 1];
    assert!(decode_bfv_eight_party_decryption_statement_bytes_v1(&oversized).is_err());

    let mut over_degree = fixture.statement.clone();
    over_degree.aggregate_public_key.b.push(0);
    let over_degree_bytes = norito::encode_canonical(&over_degree).expect("encode over-degree key");
    assert!(over_degree_bytes.len() < BFV_EIGHT_PARTY_STATEMENT_MAX_BYTES_V1);
    assert!(decode_bfv_eight_party_decryption_statement_bytes_v1(&over_degree_bytes).is_err());

    let missing = fixture.contributions[..7].to_vec();
    let missing_bytes = norito::encode_canonical(&missing).expect("encode seven contributions");
    assert!(decode_bfv_eight_party_decryption_contributions_bytes_v1(&missing_bytes).is_err());
}
