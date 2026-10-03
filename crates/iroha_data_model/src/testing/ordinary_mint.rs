//! Known-public ordinary Mint codec and signature fixture, never a genuine Mint proof/debit grant.
//!
//! The shared enrollment reports/evidence, ciphertext/tag and paired IPA proof bytes are inert
//! model data. Only the actual P-256 app equation and Ed wallet consent signatures are genuine.
//! Production encoders/formulas are called directly; no alternate wire or accepting verifier exists.
use super::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
use crate::kagemusha::*;
use iroha_crypto::kex::{KeyExchangeScheme as _, X25519Sha256};
use iroha_crypto::{Algorithm, KeyGenOption, KeyPair, Signature};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

/// Complete unsigned `TopUpRequest` and real account consent over known-public synthetic originals.
/// This type has no conversion to a verified Mint proof, financial owner, DATA or finalized debit.
pub struct KagemushaOrdinaryMintCodecFixtureV1 {
    /// Complete neutral request with genuinely signed dedicated P-256 approval and inert IPA proof.
    pub request: KagemushaOrdinaryTopUpRequestV1,
    /// Real same single-member Ed wallet signature over the exact full account-consent message.
    pub account_consent: Signature,
    /// Full synthetic enrollment/release fixture with genuine model admission signatures/policy.
    pub enrollment_fixture: Fixture,
}
/// Build shared first-release ordinary Mint data for model/Node/Core negative admission tests.
/// Real cryptographic proof verification must reject this inert paired-proof frame.
/// # Panics
/// Panics if the maintained canonical data, typed neutral envelope or signature grammar changes.
#[must_use]
pub fn kagemusha_ordinary_mint_codec_fixture_v1() -> KagemushaOrdinaryMintCodecFixtureV1 {
    build_codec_fixture(None)
}
/// Build the same known-public synthetic Mint fixture with an exact full preparation FI-control
/// original. All context, issuance, credit and approval selectors are rederived by the maintained
/// model formulas; proof/ciphertext bytes stay inert and no verified owner is constructed.
/// # Errors
/// Rejects an empty or oversized full preparation-control original.
/// # Panics
/// Panics if the maintained fixture's sole model/signature grammar changes.
pub fn kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(
    control_original: &[u8],
) -> Result<KagemushaOrdinaryMintCodecFixtureV1, String> {
    if control_original.is_empty()
        || control_original.len() > KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
    {
        return Err("synthetic full preparation FI-control original exceeds its bound".into());
    }
    Ok(build_codec_fixture(Some(
        Sha256::digest(control_original).into(),
    )))
}
fn build_codec_fixture(
    control_original_sha256: Option<[u8; 32]>,
) -> KagemushaOrdinaryMintCodecFixtureV1 {
    let (enrollment_fixture, mut context, _) = make_context(false);
    if let Some(digest) = control_original_sha256 {
        context.financial_control_original_sha256 = digest;
    }
    let (statement, encrypted_credit) = make_statement(context);
    let request = KagemushaOrdinaryTopUpRequestV1 {
        version: 1,
        authorization: authorization(statement),
        encrypted_credit,
    };
    let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
    let account_consent = Signature::new(
        wallet.private_key(),
        &request.account_signing_message().unwrap(),
    );
    request.verify_account_signature(&account_consent).unwrap();
    KagemushaOrdinaryMintCodecFixtureV1 {
        request,
        account_consent,
        enrollment_fixture,
    }
}
fn make_context(
    apple: bool,
) -> (
    Fixture,
    KagemushaOrdinaryMintAuthorizationContextV1,
    KagemushaCreditOpeningV1,
) {
    let fixture = Fixture::with_single_member_wallet(apple, false, [19; 32]);
    let verified = fixture.verify(1000).unwrap();
    let c = verified.app_credential();
    let s = c.subject();
    let (x, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![32; 32]));
    let recipient_key = x.to_bytes();
    let owner = fixture.selection.owner.clone();
    let rt = &owner.runtime;
    let operation_id = [45; 32];
    let amount = 177;
    let opening = KagemushaCreditOpeningV1 {
        version: 1,
        credit_id: [0; 32],
        amount,
        credit_commitment_opening: [50; 32],
        recipient_binding_opening: [51; 32],
        recovery_nonce: [52; 32],
    };
    let mut context = KagemushaOrdinaryMintAuthorizationContextV1 {
        version: 1,
        operation_id,
        lineage: KagemushaOrdinaryFinancialLineageV1 {
            version: 1,
            owner: owner.clone(),
            financial_epoch_id: kagemusha_ordinary_financial_epoch_id_v1(s).unwrap(),
            financial_authority_commitment: s.financial_authority_commitment,
        },
        predecessor: KagemushaOrdinaryFinancialHeadV1 {
            state_commitment: [46; 32],
            logical_sequence: (1_u128 << 101) + 7,
            state_original_sha256: [47; 32],
        },
        release_id: s.release_id,
        suite_id: s.suite_id,
        vk_digest: [48; 32],
        artifact_manifest_digest: [49; 32],
        recipient_app_credential_digest: c.digest(),
        app_credential_profile_id: s.hardware_profile_id,
        policy_epoch: s.policy_epoch,
        amount,
        recipient_credential_commitment: kagemusha_recipient_credential_commitment_v1(
            operation_id,
            c.digest(),
            opening.recipient_binding_opening,
        )
        .unwrap(),
        credit_commitment: kagemusha_mint_credit_opening_commitment_v1(
            &rt.network_id,
            &rt.asset,
            rt.asset_incarnation,
            rt.scale,
            kagemusha_liability_pool_id_v1(&rt.network_id, &rt.asset, rt.asset_incarnation)
                .unwrap(),
            amount,
            &owner.account_id,
            recipient_key,
            opening.credit_commitment_opening,
        )
        .unwrap(),
        recipient_one_time_key: recipient_key,
        clock_context: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [53; 32],
            signed_observations_original_digest: [54; 32],
            lower_at_ms: 1000,
            upper_at_ms: 1010,
        },
        financial_control_original_sha256: [55; 32],
    };
    context.validate_shape().unwrap();
    context.validate_against_credential(c).unwrap();
    let mut opening = opening;
    opening.credit_id = context.credit_id().unwrap();
    context.validate_credit_opening(&opening).unwrap();
    // Ensure a stable valid context is returned, without claiming these metadata bytes were observed.
    context.version = 1;
    (fixture, context, opening)
}
fn make_statement(
    context: KagemushaOrdinaryMintAuthorizationContextV1,
) -> (KagemushaOrdinaryMintAuthorizationStatementV1, Vec<u8>) {
    let (x, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![33; 32]));
    let envelope = KagemushaEncryptedCreditEnvelopeV1 {
        version: 1,
        ephemeral_x25519_public_key: x.to_bytes(),
        nonce: [56; 24],
        ciphertext_and_tag: vec![
            57;
            crate::kagemusha::kagemusha_credit_opening_canonical_len_v1()
                .unwrap()
                + 16
        ],
    };
    let raw = envelope
        .canonical_bytes_against_recipient_key(context.recipient_one_time_key)
        .unwrap();
    assert_eq!(raw.len(), KAGEMUSHA_ENCRYPTED_CREDIT_CANONICAL_BYTES_V1);
    let s = KagemushaOrdinaryMintAuthorizationStatementV1 {
        version: 1,
        issuance_commitment: context.issuance_commitment().unwrap(),
        credit_id: context.credit_id().unwrap(),
        context,
        ciphertext_digest: kagemusha_ciphertext_digest_v1(&raw),
    };
    s.validate_encrypted_credit(&raw).unwrap();
    (s, raw)
}
fn approval(
    statement: &KagemushaOrdinaryMintAuthorizationStatementV1,
) -> KagemushaOrdinaryMintApprovalV1 {
    let context = &statement.context;
    let challenge = KagemushaOrdinaryMintApprovalChallengeV1 {
        version: 1,
        operation_id: context.operation_id,
        nonce: [58; 32],
        credential_digest: context.recipient_app_credential_digest,
        statement_digest: statement.binding_digest().unwrap(),
        clock_context_digest: context.clock_context.binding_digest().unwrap(),
        financial_control_original_sha256: context.financial_control_original_sha256,
        issued_at_ms: 1000,
        expires_at_ms: 1100,
    };
    let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let signature: P256Signature = key.sign(&challenge.canonical_signing_bytes().unwrap());
    KagemushaOrdinaryMintApprovalV1 {
        challenge,
        evidence: KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
            signature_der: signature.to_der().as_bytes().to_vec(),
        },
    }
}
fn authorization(
    statement: KagemushaOrdinaryMintAuthorizationStatementV1,
) -> KagemushaOrdinaryMintAuthorizationV1 {
    let approval = approval(&statement);
    // Explicitly inert proof frame data for codec/selector tests; no proof verifier is called.
    let proof = KagemushaOrdinaryMintPairedProofV1 {
        version: 1,
        eq_protocol_digest: [59; 32],
        ep_protocol_digest: [60; 32],
        statement_digest: statement.binding_digest().unwrap(),
        approval_original_digest: approval.binding_digest().unwrap(),
        eq_proof: vec![63],
        ep_proof: vec![64],
        eq_history: vec![65; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: vec![66; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    };
    KagemushaOrdinaryMintAuthorizationV1 {
        version: 1,
        statement,
        approval,
        proof,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_control_option_rederives_ids_and_real_signatures_without_a_proof_grant() {
        let full_original = b"known-public inert FI-control codec original";
        let a = kagemusha_ordinary_mint_codec_fixture_v1();
        let b = kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(full_original)
            .unwrap();
        let c = kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(
            b"other complete inert original",
        )
        .unwrap();
        assert_eq!(
            b.request
                .authorization
                .statement
                .context
                .financial_control_original_sha256,
            <[u8; 32]>::from(Sha256::digest(full_original))
        );
        assert_ne!(
            a.request.authorization.statement.credit_id,
            b.request.authorization.statement.credit_id
        );
        assert_ne!(
            b.request.authorization.statement.credit_id,
            c.request.authorization.statement.credit_id
        );
        assert_ne!(
            b.request.authorization.approval,
            c.request.authorization.approval
        );
        for fixture in [&b, &c] {
            fixture
                .request
                .verify_account_signature(&fixture.account_consent)
                .unwrap();
            let c = fixture.enrollment_fixture.verify(1000).unwrap();
            fixture
                .request
                .authorization
                .approval
                .authenticate_platform_equation(c.app_credential(), None)
                .unwrap();
            assert_eq!(fixture.request.authorization.proof.eq_proof, vec![63]);
            assert_eq!(fixture.request.authorization.proof.ep_proof, vec![64]);
        }
        assert!(kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(&[]).is_err());
        assert!(
            kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(&vec![
                1;
                KAGEMUSHA_ORDINARY_CURRENT_CONTROL_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
    }

    #[test]
    fn shared_fixture_has_actual_platform_account_equations_and_only_inert_proof_data() {
        let f = kagemusha_ordinary_mint_codec_fixture_v1();
        f.request
            .verify_account_signature(&f.account_consent)
            .unwrap();
        let enrolled = f.enrollment_fixture.verify(1000).unwrap();
        assert_eq!(
            f.request
                .authorization
                .approval
                .authenticate_platform_equation(enrolled.app_credential(), None)
                .unwrap(),
            (None, None)
        );
        assert_eq!(f.request.authorization.proof.eq_proof, vec![63]);
        assert_eq!(f.request.authorization.proof.ep_proof, vec![64]);
        let raw = f.request.canonical_bytes().unwrap();
        assert_eq!(
            KagemushaOrdinaryTopUpRequestV1::decode_canonical_exact(&raw).unwrap(),
            f.request
        );
    }
    #[test]
    fn shared_fixture_account_consent_refuses_changed_full_proof_original() {
        let mut f = kagemusha_ordinary_mint_codec_fixture_v1();
        f.request.authorization.proof.eq_proof[0] ^= 1;
        assert!(
            f.request
                .verify_account_signature(&f.account_consent)
                .is_err()
        );
    }
    #[test]
    fn shared_fixture_retains_exact_distinct_x25519_keys_and_signed_originals() {
        let fixture = kagemusha_ordinary_mint_codec_fixture_v1();
        let context = &fixture.request.authorization.statement.context;
        let (recipient, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![32; 32]));
        let (ephemeral, _) = X25519Sha256::new().keypair(KeyGenOption::UseSeed(vec![33; 32]));
        assert_eq!(context.recipient_one_time_key, recipient.to_bytes());
        let envelope =
            KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
                &fixture.request.encrypted_credit,
                context.recipient_one_time_key,
            )
            .unwrap();
        assert_eq!(envelope.ephemeral_x25519_public_key, ephemeral.to_bytes());
        assert_ne!(
            context.recipient_one_time_key,
            envelope.ephemeral_x25519_public_key
        );
        assert_eq!(
            envelope
                .canonical_bytes_against_recipient_key(context.recipient_one_time_key)
                .unwrap(),
            fixture.request.encrypted_credit
        );
        fixture
            .request
            .verify_account_signature(&fixture.account_consent)
            .unwrap();
        let mut substituted = fixture.request.clone();
        substituted
            .authorization
            .statement
            .context
            .recipient_one_time_key = ephemeral.to_bytes();
        assert!(
            substituted
                .verify_account_signature(&fixture.account_consent)
                .is_err()
        );
        let mut low_order = envelope;
        low_order.ephemeral_x25519_public_key = [0; 32];
        assert!(
            low_order
                .canonical_bytes_against_recipient_key(context.recipient_one_time_key)
                .is_err()
        );
    }
}
