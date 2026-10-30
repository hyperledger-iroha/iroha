//! Exact signed Core dispatch/refusal and genuine post-admission State transition coverage.

use super::*;
use crate::executor::attachment_custody::{
    AttachmentCustody,
    x509::{
        X509Error,
        tests::{planned, request as request_fixture},
    },
};
use crate::privacy_engines::zk_x509::relation::release_fixture::{
    ZkX509ReleaseFixtureV1, build_zk_x509_network_release_fixture_v1,
};
use iroha_data_model::{
    isi::privacy::SubmitPrivacyZkX509CertificateV1,
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        signed_attachments::*,
    },
};

fn source(request: &SubmitPrivacyZkX509CertificateV1, proof: Vec<u8>) -> SignedTransaction {
    let mut action = planned(
        request,
        ActionInvocationBindingV1::Direct {
            instruction_index: 0,
        },
        ActionAuthorityBindingV1::TransactionAuthority,
    );
    action.attachments[0].bytes = proof;
    let mut plan = SignedAttachmentPlanV1::new();
    plan.push_action(action);
    TransactionBuilder::new(
        request.statement.context.network_id,
        ALICE_ID.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.clone()])
    .with_attachment_plan(plan)
    .unwrap()
    .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
}

#[test]
fn direct_x509_requires_original_owner_and_complete_security_construction() {
    let state = governance_execution_state();
    let request = request_fixture(*state.network_id_ref(), ALICE_ID.clone());
    let signed = source(&request, vec![0xab; 128]);
    let mut block = state.block(governance_execution_header(&state));
    let mut transaction = block.transaction();
    let before = transaction.privacy_budget_for_testing();
    let plain = request
        .clone()
        .execute(&ALICE_ID, &mut transaction)
        .unwrap_err();
    assert!(
        plain
            .to_string()
            .contains("original signed attachment owner")
    );
    crate::state::seed_committed_transaction_context(
        &mut transaction,
        &TransactionEntrypoint::External(signed.clone()),
        0,
    );
    let iroha_data_model::transaction::Executable::Instructions(instructions) =
        signed.instructions()
    else {
        panic!("direct source")
    };
    let mut custody = AttachmentCustody::new(&signed).unwrap();
    let error = crate::executor::Executor::Initial
        .execute_original_direct_instruction(
            &mut transaction,
            &ALICE_ID,
            &instructions[0],
            0,
            Some(&mut custody),
        )
        .unwrap_err();
    let unavailable = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V1)
        .expect_err("the joint security reduction is incomplete");
    assert!(matches!(
        unavailable,
        crate::privacy_profiles::CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete { .. }
    ));
    assert!(
        error.to_string().contains(&unavailable.to_string()),
        "{error}"
    );
    assert!(custody.finish().is_err());
    assert_eq!(transaction.privacy_budget_for_testing(), before);
    assert!(transaction.world.privacy_nullifiers.iter().next().is_none());
}

// Each block scope is separate to retain the existing debug stack discipline.
#[inline(never)]
fn install_anchor_policy(state: &State, fixture: &ZkX509ReleaseFixtureV1) {
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT));
    let mut tx = block.transaction();
    grant_governance(&mut tx);
    RegisterPrivacyZkX509TrustAnchorV1::new(fixture.authoritative_state.trust_anchor())
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    tx.apply();
    let mut tx = block.transaction();
    RegisterPrivacyZkX509CertificatePolicyV1::new(
        fixture.authoritative_state.certificate_policy().clone(),
    )
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
}
#[inline(never)]
fn install_crl(state: &State, fixture: &ZkX509ReleaseFixtureV1) {
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 1));
    let mut tx = block.transaction();
    RegisterPrivacyZkX509CrlV1::new(fixture.authoritative_state.crl_record())
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
    tx.apply();
    block.commit_world_overlay_for_testing().unwrap();
}
fn execute_component(
    tx: &mut StateTransaction<'_, '_>,
    signed: &SignedTransaction,
    request: &SubmitPrivacyZkX509CertificateV1,
    snapshot: PrivacyCompiledProfileSnapshotV1,
) -> Result<(), X509Error> {
    crate::state::seed_committed_transaction_context(
        tx,
        &TransactionEntrypoint::External(signed.clone()),
        0,
    );
    let limits = tx.world.privacy_consensus_policy.get().current_limits;
    let budget = tx.execution_budget();
    let mut custody = AttachmentCustody::new(signed).unwrap();
    custody.direct(0, &ALICE_ID, |invocation| {
        invocation.with_x509_action(
            request,
            &limits,
            tx.world
                .parameters
                .get()
                .transaction()
                .max_decompressed_bytes()
                .get(),
            &budget,
            |binding, proof| {
                super::super::x509_submission::execute_x509_after_admission(
                    request,
                    &binding,
                    proof,
                    &ALICE_ID,
                    tx,
                    snapshot,
                    snapshot.protocol_limits,
                )
                .map_err(X509Error::from)
            },
        )
    })?;
    custody.finish()?;
    Ok(())
}

/// This real native component control supplies the exact compiled profile after
/// the native admission boundary, as existing protocol controls do.
/// It is not full Exact12 or network qualification. Current readiness is mandatory.
#[test]
#[ignore = "genuine full X509 proof and Core apply; requires current native readiness"]
fn genuine_core_attached_x509_after_admission_is_atomic_and_replay_safe() {
    std::thread::Builder::new().name("x509-attached-core-acceptance".into()).stack_size(32 * 1024 * 1024).spawn(|| {
        use rand::SeedableRng as _;
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V1).expect("current native readiness is required, never skipped");
        let snapshot = profile.snapshot();
        let mut public = crate::privacy_engines::zk_x509::relation::release_fixture::reference_statement_context_v1();
        public.network_id = test_network_id(); public.action_index = 0;
        public.parameter_id = profile.parameter_id; public.parameter_digest = profile.parameter_digest;
        public.verifier_digest = profile.verifier_digest; public.statement_schema_digest = profile.statement_schema_digest;
        public.engine_manifest_digest = profile.engine_manifest_digest;
        public.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
        let now = 1_800_000_000_000;
        let draft = build_zk_x509_network_release_fixture_v1(public, now, ALICE_ID.clone()).unwrap();
        let request = SubmitPrivacyZkX509CertificateV1::new(draft.statement, ActionAttachmentRefV1 { action_index: 0, slot_index: 0 });
        let planned = source(&request, vec![0xab]);
        let materialized = request.materialize_statement_v1(planned.attachments().unwrap().action(0).unwrap(), planned.ordered_intent_digest_v1().unwrap(), &PrivacyConsensusLimitsV1::taira_default()).unwrap();
        let fixture = build_zk_x509_network_release_fixture_v1(materialized.context, now, ALICE_ID.clone()).unwrap();
        assert_eq!(fixture.statement, materialized);
        let encoded = zeroize::Zeroizing::new(fixture.witness.encode_v1().unwrap());
        let proof = crate::privacy_engines::zk_x509::engine::prove_zk_x509_credential_proof_v1_with_rng(&fixture.statement, &fixture.authoritative_state, now, &PrivacyConsensusLimitsV1::taira_default(), TEST_GENESIS_HASH, &encoded, &mut rand::rngs::StdRng::seed_from_u64(0x4933)).unwrap();
        // Preserve every independent payload field when replacing generated proof bytes.
        let mut payload = planned.payload().clone();
        payload.attachments.as_mut().unwrap().set_occurrence_bytes(0, proof).unwrap();
        payload.validate_signed_attachments_v1().unwrap();
        let signed = TransactionBuilder::from_payload(payload).unwrap().try_sign(iroha_test_samples::ALICE_KEYPAIR.private_key()).unwrap();
        assert_eq!(signed.ordered_intent_digest_v1().unwrap(), planned.ordered_intent_digest_v1().unwrap());
        let state = zk_ace_test_state();
        install_anchor_policy(&state, &fixture); install_crl(&state, &fixture);
        let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 2));
        {
            let mut tx = block.transaction();
            execute_component(&mut tx, &signed, &request, snapshot).unwrap();
            assert_eq!(tx.world.privacy_nullifiers.iter().count(), 1);
            assert_eq!(tx.privacy_budget_for_testing().0, 1);
            // Dropping an unapplied transaction rolls back its verified nullifier and budget.
        }
        {
            let mut tx = block.transaction();
            assert_eq!(tx.world.privacy_nullifiers.iter().count(), 0);
            assert_eq!(tx.privacy_budget_for_testing().0, 0);
            execute_component(&mut tx, &signed, &request, snapshot).unwrap();
            tx.apply();
        }
        {
            let mut tx = block.transaction();
            let before: Vec<_> = tx.world.privacy_nullifiers.iter().map(|(key, value)| (norito::encode_canonical(key).unwrap(), norito::encode_canonical(value).unwrap())).collect();
            let budget = tx.privacy_budget_for_testing();
            let error = execute_component(&mut tx, &signed, &request, snapshot).unwrap_err();
            assert!(error.to_string().contains("DuplicateCertificateNullifier"), "{error}");
            let after: Vec<_> = tx.world.privacy_nullifiers.iter().map(|(key, value)| (norito::encode_canonical(key).unwrap(), norito::encode_canonical(value).unwrap())).collect();
            assert_eq!(before, after); assert_eq!(tx.privacy_budget_for_testing(), budget);
        }
    }).unwrap().join().unwrap();
}
