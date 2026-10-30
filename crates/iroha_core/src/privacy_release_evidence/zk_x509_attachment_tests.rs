//! Release producer intent/attachment tests; malformed proof bytes are never verified as valid.

use super::*;
use crate::executor::attachment_custody::x509::tests::request;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

fn context() -> PrivacyReleaseTransactionContextV1 {
    PrivacyReleaseTransactionContextV1 {
        network_id: release_network_id_from_genesis_hash([0x95; 32]),
        genesis_hash: [0x95; 32],
        authority: ALICE_ID.clone(),
        creation_time: Duration::from_millis(1_800_000_000_000),
        time_to_live: Some(Duration::from_secs(60)),
        nonce: std::num::NonZeroU32::new(1),
        fee_payment: iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        metadata: Default::default(),
    }
}

#[test]
fn producer_derives_original_action_then_attaches_proof_without_rebinding() {
    let context = context();
    let independent = request(context.network_id, context.authority.clone()).statement;
    let draft = zk_x509_network_transaction_payload_v1(&context, &independent, Vec::new()).unwrap();
    let materialized = zk_x509_network_materialized_statement_v1(&draft).unwrap();
    let intent = materialized.context.transaction_intent_digest;
    assert!(!intent.is_zero());
    let malformed = vec![0x88; 1024];
    let final_payload =
        zk_x509_network_transaction_payload_v1(&context, &materialized, malformed.clone()).unwrap();
    assert_eq!(
        draft.ordered_intent_digest_v1().unwrap(),
        final_payload.ordered_intent_digest_v1().unwrap()
    );
    assert_eq!(
        zk_x509_network_materialized_statement_v1(&final_payload).unwrap(),
        materialized
    );
    let Executable::Instructions(instructions) = &final_payload.instructions else {
        panic!("direct request")
    };
    let typed = instructions[0]
        .as_any()
        .downcast_ref::<SubmitPrivacyZkX509CertificateV1>()
        .unwrap();
    assert!(typed.statement.context.transaction_intent_digest.is_zero());
    assert_eq!(
        final_payload.attachments.as_ref().unwrap().occurrences()[0].bytes,
        malformed
    );
    let signed =
        sign_zk_x509_network_payload_v1(final_payload, intent, ALICE_KEYPAIR.private_key())
            .unwrap();
    signed.verify_signature().unwrap();
    assert_eq!(
        signed
            .attachments()
            .unwrap()
            .action(0)
            .unwrap()
            .statement_digest
            .as_bytes(),
        intent.as_bytes()
    );
}

#[test]
fn producer_signer_rejects_changed_independent_intent_or_wrong_network() {
    let context = context();
    let independent = request(context.network_id, context.authority.clone()).statement;
    let payload =
        zk_x509_network_transaction_payload_v1(&context, &independent, vec![0x88; 128]).unwrap();
    let intent = zk_x509_network_materialized_statement_v1(&payload)
        .unwrap()
        .context
        .transaction_intent_digest;
    let mut changed = context.clone();
    changed.creation_time += Duration::from_millis(1);
    let changed_payload =
        zk_x509_network_transaction_payload_v1(&changed, &independent, vec![0x88; 128]).unwrap();
    assert!(matches!(
        sign_zk_x509_network_payload_v1(changed_payload, intent, ALICE_KEYPAIR.private_key()),
        Err(PrivacyReleaseEvidenceErrorClassV1::EvidenceInvariant)
    ));
    changed.genesis_hash[0] ^= 1;
    assert!(matches!(
        zk_x509_network_transaction_payload_v1(&changed, &independent, vec![0x88; 128]),
        Err(PrivacyReleaseEvidenceErrorClassV1::FixtureConstructionFailed)
    ));
}
