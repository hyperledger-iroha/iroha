//! Complete known-public ordinary Node transport fixture; all authority/clock/proof inputs are inert.
//! The genuine test P-256/Ed request and Ed decision equations do not supply installed World roots,
//! current signed observations, globally held DATA, actual IPA proofs or any debit capability.
use crate::kagemusha::*;
use iroha_crypto::{Algorithm, KeyPair, Signature};
use sha2::{Digest as _, Sha256};

/// A complete shape-valid carrier and its sole existing shared unsigned Mint/signature fixture.
/// No conversion to a verified proof, clock, Node purpose or finalized debit exists.
pub struct KagemushaOrdinaryNodeMintCodecFixtureV1 {
    /// Full carrier DATA; independent roots and signed clocks are intentionally inert.
    pub submission: KagemushaOrdinaryNodeMintSubmissionV1,
    /// Maintained shared known-public fixture with only real test platform/account equations.
    pub mint_fixture: super::ordinary_mint::KagemushaOrdinaryMintCodecFixtureV1,
}
/// Construct the sole Model transport fixture for exact codecs and closed negative admission.
/// # Panics
/// Panics if a maintained complete model/signature or selector grammar changes.
#[must_use]
pub fn kagemusha_ordinary_node_mint_codec_fixture_v1() -> KagemushaOrdinaryNodeMintCodecFixtureV1 {
    let preparation_control_original = b"known-public inert full preparation FI control".to_vec();
    let current_control_original = b"separate inert current FI control".to_vec();
    let mint_fixture =
        super::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_with_preparation_control_v1(
            &preparation_control_original,
        )
        .unwrap();
    let request = &mint_fixture.request;
    let context = &request.authorization.statement.context;
    let topup_request_original = request.canonical_bytes().unwrap();
    let selection = KagemushaOrdinaryIncomingSelectionV1 {
        version: 1,
        lineage: context.lineage.clone(),
        operation_id: context.operation_id,
        predecessor: context.predecessor.clone(),
        source: KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256: Sha256::digest(&topup_request_original).into(),
        },
        credit_id: request.authorization.statement.credit_id,
        amount: context.amount,
        scale: context.lineage.owner.runtime.scale,
        recipient_app_credential_digest: context.recipient_app_credential_digest,
        financial_control_original_sha256: context.financial_control_original_sha256,
        clock_context_digest: context.clock_context.binding_digest().unwrap(),
    };
    selection.validate_against_topup(request).unwrap();
    let subject = KagemushaOrdinaryMintDebitDecisionV1 {
        version: 1,
        selection,
        issuer_policy_digest: kagemusha_ordinary_retail_issuer_policy_digest_v1(
            &mint_fixture.enrollment_fixture.issuer_policy,
        )
        .unwrap(),
        release_id: context.release_id,
        reserved_data_record_original_sha256: [88; 32],
        reserved_data_revision: 11,
        data_incarnation_digest: [89; 32],
        data_policy_epoch: 3,
        data_schema_epoch: 2,
        authority_context_id: [90; 32],
        authority_height: 101,
        world_root: [91; 32],
        current_financial_control_original_sha256: Sha256::digest(&current_control_original).into(),
        decision_clock_context: KagemushaOrdinaryCashClockContextV1 {
            version: 1,
            request_nonce: [93; 32],
            signed_observations_original_digest: [94; 32],
            lower_at_ms: 1000,
            upper_at_ms: 1010,
        },
        issued_at_ms: 1000,
        expires_at_ms: 1100,
    };
    let issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
    assert_eq!(
        issuer.public_key(),
        &mint_fixture
            .enrollment_fixture
            .issuer_policy
            .issuer_public_key
    );
    let signature = Signature::new(
        issuer.private_key(),
        &subject.issuer_signing_message().unwrap(),
    );
    let decision = KagemushaSignedOrdinaryMintDebitDecisionV1 { subject, signature };
    decision
        .verify_for_request(
            request,
            &decision.subject.selection,
            &mint_fixture.enrollment_fixture.issuer_policy,
        )
        .unwrap();
    let submission = KagemushaOrdinaryNodeMintSubmissionV1 {
        version: 1,
        topup_request_original,
        account_consent: mint_fixture.account_consent.clone(),
        issuer_purpose_original: vec![1],
        identity_policy_original: vec![2],
        core_enrollment_issuer_policy_original: vec![3],
        enrollment_challenge_original: vec![4],
        raw_admission_original: vec![5],
        platform_attestation_original: vec![6],
        enrollment_possession_original: vec![7],
        credential_original: vec![8],
        financial_enrollment_original: vec![9],
        preparation_integrity: None,
        decision_integrity: None,
        clock_selection_original: vec![10],
        preparation_clock_original: vec![11],
        decision_clock_original: vec![12],
        preparation_clock_parent_originals: vec![],
        decision_clock_parent_originals: vec![],
        preparation_control_original,
        debit_decision_original: decision.canonical_bytes().unwrap(),
        current_control_original,
    };
    submission.validate_shape().unwrap();
    KagemushaOrdinaryNodeMintCodecFixtureV1 {
        submission,
        mint_fixture,
    }
}
