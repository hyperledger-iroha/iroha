//! Current-State/replay controls and the explicit genuine attached acceptance selector.

use super::*;
use crate::privacy_engines::zk_x509::{
    engine::prove_zk_x509_credential_proof_v1_with_rng,
    relation::release_fixture::{
        ZkX509ReleaseFixtureV1, build_zk_x509_network_release_fixture_v1,
        reference_statement_context_v1,
    },
};
use iroha_data_model::{
    account::AccountId,
    privacy::*,
    transaction::{FeePaymentIntent, TransactionBuilder, signed_attachments::*},
};
const NOW: u64 = 1_800_000_000_000;
const LIMITS: PrivacyConsensusLimitsV1 = PrivacyConsensusLimitsV1::taira_default();

fn fixture() -> (ZkX509ReleaseFixtureV1, PrivacyCompiledProfileSnapshotV1) {
    let sample = privacy_action_role_samples_v1()
        .unwrap()
        .into_iter()
        .find(|sample| sample.role == PrivacyActionRoleV1::ZkX509CertificatePresentation)
        .unwrap();
    let fixture = build_zk_x509_network_release_fixture_v1(
        reference_statement_context_v1(),
        NOW,
        match sample.envelope.statement {
            PrivacyStatementV1::IrohaZkX509StarkP256V1(statement) => statement.wallet_account,
            _ => unreachable!(),
        },
    )
    .unwrap();
    (fixture, sample.profile)
}
fn context<'a>(
    fixture: &'a ZkX509ReleaseFixtureV1,
    profile: &'a PrivacyCompiledProfileSnapshotV1,
) -> PrivacyVerificationContextV1<'a> {
    PrivacyVerificationContextV1 {
        profile,
        protocol_limits: &profile.protocol_limits,
        consensus_limits: &LIMITS,
        network_id: &fixture.statement.context.network_id,
        genesis_hash: *fixture.statement.context.network_id.as_bytes(),
        current_height: 2,
        expected_action_index: 0,
        block_timestamp_ms: NOW,
        pgc_state: None,
        orchard_state: None,
        proof_managed_state: None,
        zk_x509_state: Some(PrivacyZkX509VerificationStateV1 {
            authoritative_state: &fixture.authoritative_state,
            certificate_nullifier_consumed: false,
        }),
        bootle_lantern_policy: None,
        vega_issuer_record: None,
    }
}

#[test]
fn borrowed_native_boundary_rejects_missing_stale_expired_and_consumed_state_before_proof() {
    let (fixture, profile) = fixture();
    let malformed = [0xab; 128];
    let mut current = context(&fixture, &profile);
    assert!(
        matches!(
            verify_zk_x509_certificate_v1(&fixture.statement, &malformed, &current),
            Err(PrivacyVerificationErrorV1::NativeZkX509(_))
        ),
        "real current State reaches native malformed-proof rejection"
    );
    current
        .zk_x509_state
        .as_mut()
        .unwrap()
        .certificate_nullifier_consumed = true;
    assert!(
        matches!(verify_zk_x509_certificate_v1(&fixture.statement, &malformed, &current), Err(PrivacyVerificationErrorV1::ZkX509State(detail)) if detail.code == PrivacyZkX509StateFailureCodeV1::DuplicateCertificateNullifier)
    );
    current
        .zk_x509_state
        .as_mut()
        .unwrap()
        .certificate_nullifier_consumed = false;
    current.block_timestamp_ms = NOW + 301_000;
    assert!(
        matches!(verify_zk_x509_certificate_v1(&fixture.statement, &malformed, &current), Err(PrivacyVerificationErrorV1::ZkX509State(detail)) if detail.code == PrivacyZkX509StateFailureCodeV1::AuthoritativeStateMismatch)
    );
    current.block_timestamp_ms = NOW;
    let mut stale = fixture.statement.clone();
    stale.crl_record_epoch += 1;
    assert!(
        matches!(verify_zk_x509_certificate_v1(&stale, &malformed, &current), Err(PrivacyVerificationErrorV1::ZkX509State(detail)) if detail.code == PrivacyZkX509StateFailureCodeV1::AuthoritativeStateMismatch)
    );
    current.zk_x509_state = None;
    assert!(
        matches!(verify_zk_x509_certificate_v1(&fixture.statement, &malformed, &current), Err(PrivacyVerificationErrorV1::ZkX509State(detail)) if detail.code == PrivacyZkX509StateFailureCodeV1::MissingTrustedState)
    );
}

#[test]
fn attached_context_requires_exact_network_genesis_and_privacy_ordinal() {
    let (fixture, profile) = fixture();
    let mut current = context(&fixture, &profile);
    validate_privacy_execution_context_v1(&fixture.statement.context, &current).unwrap();
    current.genesis_hash = [0; 32];
    assert!(
        matches!(validate_privacy_execution_context_v1(&fixture.statement.context, &current), Err(PrivacyVerificationErrorV1::Context(detail)) if detail.code == PrivacyVerificationContextFailureCodeV1::ZeroGenesisHash)
    );
    current.genesis_hash = [0x31; 32];
    assert!(
        matches!(validate_privacy_execution_context_v1(&fixture.statement.context, &current), Err(PrivacyVerificationErrorV1::Context(detail)) if detail.code == PrivacyVerificationContextFailureCodeV1::NetworkGenesisMismatch)
    );
    current.genesis_hash = *current.network_id.as_bytes();
    current.expected_action_index = 1;
    assert!(
        matches!(validate_privacy_execution_context_v1(&fixture.statement.context, &current), Err(PrivacyVerificationErrorV1::Context(detail)) if detail.code == PrivacyVerificationContextFailureCodeV1::ActionIndexMismatch)
    );
    current.expected_action_index = 0;
    let mut foreign = fixture.statement.context;
    foreign.network_id = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(b"foreign")),
    );
    assert!(
        matches!(validate_privacy_execution_context_v1(&foreign, &current), Err(PrivacyVerificationErrorV1::Context(detail)) if detail.code == PrivacyVerificationContextFailureCodeV1::NetworkIdMismatch)
    );
}

/// Required acceptance control. It fails at the real readiness gate until the
/// current profile's native resource/expectation requirements are fulfilled.
/// No skip branch, fake proof, or alternative profile profile is permitted.
#[test]
#[ignore = "genuine full proof; requires current X509 native readiness and optimized resource validation"]
fn genuine_attached_x509_proof_acceptance_requires_current_readiness() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(genuine_attached_x509_proof_acceptance_body)
        .unwrap()
        .join()
        .unwrap();
}
fn genuine_attached_x509_proof_acceptance_body() {
    use rand::SeedableRng as _;
    let profile = crate::privacy_profiles::compiled_privacy_profile_v1(
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
    )
    .expect("current native readiness must pass; this is not a skip");
    let profile = profile.snapshot();
    let key = iroha_crypto::KeyPair::from_seed(vec![0x42; 32], iroha_crypto::Algorithm::Ed25519);
    let mut public = reference_statement_context_v1();
    public.parameter_id = profile.parameter_id;
    public.parameter_digest = profile.parameter_digest;
    public.verifier_digest = profile.verifier_digest;
    public.statement_schema_digest = profile.statement_schema_digest;
    public.engine_manifest_digest = profile.engine_manifest_digest;
    public.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
    let draft = build_zk_x509_network_release_fixture_v1(
        public,
        NOW,
        AccountId::new(key.public_key().clone()),
    )
    .unwrap();
    let request = SubmitPrivacyZkX509CertificateV1::new(
        draft.statement,
        ActionAttachmentRefV1 {
            action_index: 0,
            slot_index: 0,
        },
    );
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(PlannedActionV1 {
        operation: ActionOperationV1::NativeInstruction(
            SubmitPrivacyZkX509CertificateV1::WIRE_ID.into(),
        ),
        role: ActionRoleV1::ProofEffect,
        authority: ActionAuthorityBindingV1::TransactionAuthority,
        invocation: ActionInvocationBindingV1::Direct {
            instruction_index: 0,
        },
        public_inputs_digest: request.public_inputs_digest_v1().unwrap(),
        attachments: vec![PlannedAttachmentV1 {
            kind: AttachmentKindV1::Proof,
            role: AttachmentRoleV1::ActionProof,
            bytes: vec![0xab],
        }],
    });
    let mut payload = TransactionBuilder::new(
        public.network_id,
        request.statement.wallet_account.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.clone()])
    .with_attachment_plan(plan)
    .unwrap()
    .into_payload()
    .unwrap();
    let materialized = request
        .materialize_statement_v1(
            payload.attachments.as_ref().unwrap().action(0).unwrap(),
            payload.ordered_intent_digest_v1().unwrap(),
            &LIMITS,
        )
        .unwrap();
    let fixture = build_zk_x509_network_release_fixture_v1(
        materialized.context,
        NOW,
        materialized.wallet_account.clone(),
    )
    .unwrap();
    assert_eq!(fixture.statement, materialized);
    let encoded = zeroize::Zeroizing::new(fixture.witness.encode_v1().unwrap());
    let proof = prove_zk_x509_credential_proof_v1_with_rng(
        &fixture.statement,
        &fixture.authoritative_state,
        NOW,
        &LIMITS,
        *public.network_id.as_bytes(),
        &encoded,
        &mut rand::rngs::StdRng::seed_from_u64(0x4933),
    )
    .unwrap();
    payload
        .attachments
        .as_mut()
        .unwrap()
        .set_occurrence_bytes(0, proof.clone())
        .unwrap();
    payload.validate_signed_attachments_v1().unwrap();
    let signed = TransactionBuilder::from_payload(payload)
        .unwrap()
        .try_sign(key.private_key())
        .unwrap();
    signed.verify_signature().unwrap();
    let mut current = context(&fixture, &profile);
    let accepted =
        verify_privacy_zk_x509_attached_v1(&request, &fixture.statement, &proof, &current).unwrap();
    assert_eq!(
        accepted.protocol_id(),
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1
    );
    assert_eq!(
        accepted.encoded_action_bytes(),
        norito::canonical_frame_len(&request).unwrap() as u64 + proof.len() as u64
    );
    current
        .zk_x509_state
        .as_mut()
        .unwrap()
        .certificate_nullifier_consumed = true;
    assert!(
        matches!(verify_privacy_zk_x509_attached_v1(&request, &fixture.statement, &proof, &current), Err(PrivacyVerificationErrorV1::ZkX509State(detail)) if detail.code == PrivacyZkX509StateFailureCodeV1::DuplicateCertificateNullifier)
    );
}
