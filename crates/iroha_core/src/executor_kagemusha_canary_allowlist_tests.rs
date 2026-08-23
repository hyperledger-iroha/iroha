fn canary_allowlist_digest(label: &[u8]) -> iroha_data_model::offline::KagemushaExactBytesDigestV1 {
    iroha_data_model::offline::KagemushaExactBytesDigestV1::from_bytes(label)
        .expect("non-empty canary allowlist identity")
}

fn canary_allowlist_permit() -> iroha_data_model::offline::KagemushaV4TairaCanaryPermitV1 {
    use iroha_data_model::offline::{
        KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA,
        KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA, KagemushaV4PromotionBindingV1,
        KagemushaV4TairaCanaryAuthorizationBodyV1, KagemushaV4TairaCanaryPermitV1,
    };
    let controller = KeyPair::from_seed(vec![0xC1; 32], Algorithm::Ed25519);
    let body = KagemushaV4TairaCanaryAuthorizationBodyV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_AUTHORIZATION_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        binding: KagemushaV4PromotionBindingV1 {
            promotion_controller: controller.public_key().clone(),
            promotion_reservation: canary_allowlist_digest(b"allowlist reservation"),
            promotion_id: [0xC2; 32],
            network_id: executor_test_network_id(b"allowlist canary network"),
            reviewed_source_closure_descriptor_sha256: [0xC3; 32],
            manifest_sha256: [0xC4; 32],
            release_record_sha256: [0xC5; 32],
            release_policy_source: canary_allowlist_digest(b"allowlist release policy"),
            device_attestation_policy_norito: canary_allowlist_digest(b"allowlist device policy"),
            signed_genesis: canary_allowlist_digest(b"allowlist genesis"),
            catalog_consensus_policy_digest: [0xC6; 32],
            execution_policy_hash: Hash::new(b"allowlist execution policy"),
        },
        activation_expectations_artifact: canary_allowlist_digest(b"allowlist expectations"),
        activation_finality_receipt: canary_allowlist_digest(b"allowlist receipt"),
        canary_authority: ALICE_ID.clone(),
        canonical_torii_origin: "https://taira.example".to_owned(),
        authorized_at_unix_ms: 1,
        expires_at_unix_ms: 2,
        expires_at_height: core::num::NonZeroU64::new(2).expect("non-zero expiry"),
    };
    let signature =
        iroha_crypto::SignatureOf::try_from_hash(controller.private_key(), body.signing_hash())
            .expect("controller signs allowlist permit");
    KagemushaV4TairaCanaryPermitV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_PERMIT_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        body,
        signature,
    }
}

#[test]
fn initial_executor_admits_both_exact_canary_steps_to_core_validation() {
    use iroha_data_model::{
        isi::offline::{AuthorizeKagemushaTairaCanaryV4, RecordKagemushaTairaCanaryV4},
        offline::{
            KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA,
            KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA, KagemushaV4TairaCanaryReservationBodyV1,
            KagemushaV4TairaCanaryReservationV1,
        },
        transaction::SignedTransaction,
    };
    let permit = canary_allowlist_permit();
    let controller = KeyPair::from_seed(vec![0xC1; 32], Algorithm::Ed25519);
    let reservation_body = KagemushaV4TairaCanaryReservationBodyV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        permit: permit.clone(),
        canary_transaction_intent: HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::new(
            b"allowlist canary intent",
        )),
        canary_transaction_wire: canary_allowlist_digest(b"allowlist canary wire"),
        canary_entrypoint_hash: Hash::new(b"allowlist canary entrypoint"),
    };
    let reservation_signature = iroha_crypto::SignatureOf::try_from_hash(
        controller.private_key(),
        reservation_body.signing_hash(),
    )
    .expect("controller signs allowlist reservation");
    let reservation = KagemushaV4TairaCanaryReservationV1 {
        schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA.to_owned(),
        version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
        body: reservation_body,
        signature: reservation_signature,
    };
    for instruction in [
        InstructionBox::from(AuthorizeKagemushaTairaCanaryV4::new(reservation)),
        InstructionBox::from(RecordKagemushaTairaCanaryV4::new(permit)),
    ] {
        assert!(
            super::initial_native_instruction_is_explicitly_admitted(&instruction),
            "exact canary steps must reach Core signature and marker validation",
        );
    }
}
