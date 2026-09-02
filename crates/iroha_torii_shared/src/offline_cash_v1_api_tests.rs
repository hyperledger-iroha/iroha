//! Focused tests for the closed Torii Offline Cash V1 surface.

use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::{BlockHeader, consensus_v2::HeightContextId},
    domain::DomainId,
    offline::{
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OfflineCashDevicePublicKeyV1,
        OfflineCashDeviceSignatureV1, OfflineCashInboxReceiptV1, OfflineCashPairedProofV1,
        OfflineCashTransferStatementV1, offline_cash_device_key_reference_v1,
        offline_cash_inbox_receipt_commitment_v1, offline_cash_liability_pool_id_v1,
    },
};
const FIXTURE_PAYMENT_RECIPIENT: &str = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53";
const FIXTURE_RECIPIENT_PUBLIC_KEY_HEX: &str = "041e18532fd4754c02f3041d9c75ceb33b83ffd81ac7ce4fe882ccb1c98bc5896ea46c311c4e2ff40dd96a3653e6e45445d32dfe486eced75c7a90c6a18881c0a3";
const FIXTURE_SENDER_PUBLIC_KEY_HEX: &str = "047135fa4fd93a09dce98bbf681b4bfcf50e7c0d6354e62afb0bff2a3429617865ed4c1f02ddb9023ee56a557e515d6a9dc66c11f220960de594334df588776724";
const FIXTURE_TOP_UP_PUBLIC_KEY_HEX: &str = "04209c317b637935dd3da1c54f63495dfb31f97d293df085710320595c9aacb83fdde4c69fc17a0c74c20cc692662f049892ba37a4ba47d2c70cd8a99986391f9b";
const FIXTURE_REQUEST_SIGNATURE_03_HEX: &str = "b7227d60ba0e0c213843f4e854daa3c6f134e5d8b59af56540956e041ff3115a0d2a93926c3ad6ac55eb2034533dd4f4c691764f0fb8685a046ed1acd1268305";
const FIXTURE_REQUEST_SIGNATURE_31_HEX: &str = "a69f5265eda0f2ccf64bca111bfcdd5582e0b51e61902d732ea9c36cd8ed461f78921df314b55dc8e0b9cbb2d4a795146d4b7239d81d4ddf8f29c523954a75a1";
const FIXTURE_ACK_SIGNATURE_HEX: &str = "b872a2a3d4cdf82fb18ae83e67c47f30ac484abd9a53b91e67e7eb7af46b2ca473ae96eba663456a595b6b0506aa73b70111dc9eca2e4e318d40fa6109c13afd";

fn network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"torii-shared-offline-cash-v1",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn fixture_public_key(encoded: &str) -> OfflineCashDevicePublicKeyV1 {
    OfflineCashDevicePublicKeyV1::from_sec1_bytes(
        &hex::decode(encoded).expect("decode fixed P-256 public key"),
    )
    .expect("canonical fixed P-256 public key")
}

fn fixture_signature(encoded: &str) -> OfflineCashDeviceSignatureV1 {
    OfflineCashDeviceSignatureV1::from_raw_bytes(
        &hex::decode(encoded).expect("decode fixed P-256 signature"),
    )
    .expect("canonical low-S P-256 signature")
}

fn payment_request_with_id(request_id: [u8; 32]) -> OfflineCashPaymentRequestV1 {
    let recipient_public_key = fixture_public_key(FIXTURE_RECIPIENT_PUBLIC_KEY_HEX);
    let network_id = network_id();
    let asset = asset();
    assert!(
        request_id == [3; 32] || request_id == [0x31; 32],
        "unexpected fixed request identity"
    );
    let signature = match request_id[0] {
        3 => fixture_signature(FIXTURE_REQUEST_SIGNATURE_03_HEX),
        0x31 => fixture_signature(FIXTURE_REQUEST_SIGNATURE_31_HEX),
        _ => unreachable!("request identity checked above"),
    };
    OfflineCashPaymentRequestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: [1; 32],
        network_id,
        liability_pool_id: offline_cash_liability_pool_id_v1(&network_id, &asset)
            .expect("liability pool"),
        asset,
        scale: 4,
        amount: 12_345,
        recipient: AccountId::parse_encoded(FIXTURE_PAYMENT_RECIPIENT)
            .expect("canonical fixed payment recipient"),
        recipient_lane_id: [2; 32],
        recipient_key_reference: offline_cash_device_key_reference_v1(&recipient_public_key),
        recipient_public_key,
        recipient_hardware_policy_id: [4; 32],
        request_id,
        issued_at_ms: 1_000,
        expires_at_ms: 61_000,
        signature,
    }
}

fn payment_request() -> OfflineCashPaymentRequestV1 {
    payment_request_with_id([3; 32])
}

fn payment(request: &OfflineCashPaymentRequestV1) -> OfflineCashPaymentV1 {
    let request_digest = request.canonical_digest().expect("request digest");
    let sender_public_key = fixture_public_key(FIXTURE_SENDER_PUBLIC_KEY_HEX);
    let statement = OfflineCashTransferStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: request.release_id,
        network_id: request.network_id,
        asset: request.asset.clone(),
        scale: request.scale,
        amount: request.amount,
        liability_pool_id: request.liability_pool_id,
        request_digest,
        credit_id: [0; 32],
        recipient_lane_id: request.recipient_lane_id,
        recipient_key_reference: request.recipient_key_reference,
        recipient_hardware_policy_id: request.recipient_hardware_policy_id,
        sender_lane_id: [5; 32],
        sender_hardware_epoch_id: [6; 32],
        sender_key_reference: offline_cash_device_key_reference_v1(&sender_public_key),
        sender_hardware_policy_id: [7; 32],
        sender_before_sequence: 41,
        sender_after_sequence: 42,
        sender_before: iroha_data_model::offline::offline_cash_pasta_state_commitment_v1(
            iroha_data_model::offline::OfflineCashPastaStateCommitmentV1 {
                eq: [5; 32],
                ep: [6; 32],
            },
        ),
        sender_after: iroha_data_model::offline::offline_cash_pasta_state_commitment_v1(
            iroha_data_model::offline::OfflineCashPastaStateCommitmentV1 {
                eq: [7; 32],
                ep: [8; 32],
            },
        ),
        credit_commitment: [8; 32],
        sender_committed_at_ms: request.issued_at_ms + 1,
        transition_digest: [0; 32],
    }
    .seal_transition([0xD7; 32])
    .expect("seal transition");
    let semantic_digest = statement.canonical_digest().expect("statement digest");
    OfflineCashPaymentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        statement,
        proof: OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: [9; 32],
            ep_protocol_digest: [10; 32],
            semantic_digest,
            guard_eq_credential_audit: [0x19; 32],
            guard_ep_credential_audit: [0x1A; 32],
            eq_deferred_audit: [0x13; 32],
            ep_deferred_audit: [0x14; 32],
            predecessor_state: iroha_data_model::offline::OfflineCashPastaStateCommitmentV1 {
                eq: [5; 32],
                ep: [6; 32],
            },
            successor_state: iroha_data_model::offline::OfflineCashPastaStateCommitmentV1 {
                eq: [7; 32],
                ep: [8; 32],
            },
            eq_proof: vec![0xA1; 128],
            ep_proof: vec![0xB2; 128],
            eq_history: vec![0xC3; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0xD4; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
        },
        encrypted_credit: vec![0xE5; 128],
        artifact_manifest_digest: [11; 32],
    }
}

fn acknowledgement(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> OfflineCashAcknowledgementV1 {
    let request_digest = request.canonical_digest().expect("request digest");
    let payment_digest = payment
        .canonical_digest_against(request)
        .expect("payment digest");
    let inbox_sequence = 73;
    let staging_hardware_epoch_id = [0xA3; 32];
    let inbox_receipt = OfflineCashInboxReceiptV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        credit_id: payment.statement.credit_id,
        staging_hardware_epoch_id,
        inbox_sequence,
        receipt_commitment: offline_cash_inbox_receipt_commitment_v1(
            request.recipient_lane_id,
            staging_hardware_epoch_id,
            inbox_sequence,
            payment.statement.credit_id,
            payment_digest,
        )
        .expect("receipt commitment"),
    };
    OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        payment_digest,
        inbox_receipt,
        acknowledged_at_ms: request.expires_at_ms + 1,
        signature: fixture_signature(FIXTURE_ACK_SIGNATURE_HEX),
    }
}

fn top_up_request() -> OfflineCashTopUpRequestV1 {
    let recipient_public_key = fixture_public_key(FIXTURE_TOP_UP_PUBLIC_KEY_HEX);
    let network_id = network_id();
    let asset = asset();
    OfflineCashTopUpRequestV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        operation_id: [0x21; 32],
        issuance_commitment: [0; 32],
        credit_id: [0; 32],
        release_id: [0x22; 32],
        network_id,
        asset: asset.clone(),
        scale: 4,
        amount: 25_000,
        liability_pool_id: offline_cash_liability_pool_id_v1(&network_id, &asset)
            .expect("liability pool"),
        payer: account(0x31),
        recipient: account(0x32),
        recipient_lane_id: [0x23; 32],
        recipient_key_reference: offline_cash_device_key_reference_v1(&recipient_public_key),
        recipient_public_key,
        recipient_hardware_policy_id: [0x25; 32],
        credit_commitment: [0x26; 32],
        encrypted_credit: vec![0x27; 96],
        artifact_manifest_digest: [0x28; 32],
    }
    .seal_identifiers()
    .expect("seal top-up identifiers")
}

fn trust_anchor() -> OfflineCashFinalityTrustAnchorV1 {
    OfflineCashFinalityTrustAnchorV1 {
        network_id: network_id(),
        block_height: 7,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"offline-cash-v1-pinned-context",
        ))),
    }
}

#[test]
fn peer_boundary_roundtrips_only_the_exact_context_bound_v1_session() {
    let request = payment_request();
    let payment = payment(&request);
    let acknowledgement = acknowledgement(&request, &payment);
    let request_bytes = norito::encode_canonical(&request).expect("encode request");
    let payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
    let acknowledgement_bytes =
        norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
    let other_request = payment_request_with_id([0x31; 32]);
    assert!(request_bytes.len() <= OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1);
    assert!(payment_bytes.len() <= OFFLINE_CASH_PAYMENT_MAX_BYTES_V1);
    assert!(acknowledgement_bytes.len() <= OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1);
    let decoded_request = decode_offline_cash_payment_request_v1(&request_bytes)
        .expect("decode exact payment request");
    let decoded_payment = decode_offline_cash_payment_v1(&payment_bytes, &decoded_request)
        .expect("decode exact payment");
    let decoded_acknowledgement = decode_offline_cash_acknowledgement_v1(
        &acknowledgement_bytes,
        &decoded_request,
        &decoded_payment,
    )
    .expect("decode exact acknowledgement");
    assert_eq!(decoded_request, request);
    assert_eq!(decoded_payment, payment);
    assert_eq!(decoded_acknowledgement, acknowledgement);

    assert!(decode_offline_cash_payment_v1(&payment_bytes, &other_request).is_err());
    assert!(
        decode_offline_cash_acknowledgement_v1(&acknowledgement_bytes, &other_request, &payment,)
            .is_err()
    );
}

#[test]
fn peer_boundary_pre_caps_every_message_and_rejects_trailing_bytes() {
    let request = payment_request();
    let payment = payment(&request);
    assert!(matches!(
        decode_offline_cash_payment_request_v1(&vec![
            0;
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 + 1
        ]),
        Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { .. })
    ));
    assert!(matches!(
        decode_offline_cash_payment_v1(&vec![0; OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1], &request,),
        Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { .. })
    ));
    assert!(matches!(
        decode_offline_cash_acknowledgement_v1(
            &vec![0; OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1],
            &request,
            &payment,
        ),
        Err(OfflineCashValidationErrorV1::EncodedSizeExceeded { .. })
    ));

    let mut encoded = norito::encode_canonical(&request).expect("encode request");
    encoded.push(0);
    assert!(decode_offline_cash_payment_request_v1(&encoded).is_err());
}

#[test]
fn payment_request_has_no_receiver_balance_or_history_shape() {
    let request = payment_request();
    let json = norito::json::to_string(&request).expect("encode request JSON");
    for forbidden in [
        "balance_head",
        "receiver_head",
        "lineage",
        "ancestry",
        "origin",
        "hop",
        "proof_depth",
        "input_count",
        "note_inventory",
        "recipient_hardware_epoch_id",
    ] {
        assert!(
            !json.contains(forbidden),
            "payment request exposed forbidden field `{forbidden}`"
        );
    }
    assert!(json.contains("recipient_lane_id"));
    assert!(json.contains("request_id"));
}

#[test]
fn readiness_is_the_exact_four_field_v1_contract() {
    let readiness = OfflineCashReadinessV1 {
        cash_handoff_capability: OFFLINE_CASH_HANDOFF_CAPABILITY_V1.to_owned(),
        wire_version: OFFLINE_CASH_WIRE_VERSION_V1,
        device_lifecycle_version: OFFLINE_CASH_DEVICE_LIFECYCLE_VERSION_V1,
        ready: true,
    };
    readiness.validate().expect("valid readiness");
    let json = norito::json::to_string(&readiness).expect("encode readiness");
    let value: norito::json::Value = norito::json::from_str(&json).expect("read readiness JSON");
    let object = value.as_object().expect("readiness object");
    assert_eq!(object.len(), 4);
    for field in [
        "cash_handoff_capability",
        "wire_version",
        "device_lifecycle_version",
        "ready",
    ] {
        assert!(
            object.contains_key(field),
            "missing readiness field `{field}`"
        );
    }

    let unknown = json.replacen('{', r#"{"extra":null,"#, 1);
    assert!(norito::json::from_str::<OfflineCashReadinessV1>(&unknown).is_err());
    let mut wrong = readiness;
    wrong.wire_version += 1;
    assert!(wrong.validate().is_err());
}

#[test]
fn top_up_keeps_distinct_payer_and_recipient_and_decodes_exactly() {
    let request = top_up_request();
    assert_ne!(request.payer, request.recipient);
    let encoded = norito::encode_canonical(&request).expect("encode top-up request");
    assert!(encoded.len() <= OFFLINE_CASH_TOP_UP_REQUEST_MAX_BYTES_V1);
    assert_eq!(
        decode_offline_cash_top_up_request_v1(&encoded).expect("decode top-up request"),
        request
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_offline_cash_top_up_request_v1(&trailing).is_err());
    assert!(matches!(
        decode_offline_cash_top_up_request_v1(&vec![
            0;
            OFFLINE_CASH_TOP_UP_REQUEST_MAX_BYTES_V1 + 1
        ]),
        Err(OfflineCashApiErrorV1::EncodedSizeExceeded { .. })
    ));
}

#[test]
fn operation_status_decoder_requires_an_external_finality_anchor() {
    let decoder: fn(
        &[u8],
        &OfflineCashFinalityTrustAnchorV1,
    ) -> Result<OfflineCashOperationStatusV1, OfflineCashApiErrorV1> =
        decode_offline_cash_operation_status_v1;
    let status = OfflineCashOperationStatusV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        operation_id: [0x41; 32],
        kind: OfflineCashOperationKindV1::TopUp,
        state: OfflineCashOperationStateV1::Pending,
        result: None,
        rejection: None,
    };
    let encoded = norito::encode_canonical(&status).expect("encode pending status");
    let unverified = decode_unverified_offline_cash_operation_status_v1(&encoded)
        .expect("bounded structural decode");
    assert_eq!(unverified.operation_id(), status.operation_id);
    assert_eq!(unverified.kind(), status.kind);
    assert_eq!(unverified.state(), status.state);
    assert_eq!(unverified.finality_anchor_hint(), None);
    let json = norito::json::to_vec(&status).expect("encode pending status JSON");
    let unverified_json = decode_unverified_offline_cash_operation_status_json_v1(&json)
        .expect("bounded structural JSON decode");
    assert_eq!(unverified_json.operation_id(), status.operation_id);
    assert_eq!(unverified_json.kind(), status.kind);
    assert_eq!(unverified_json.state(), status.state);
    assert_eq!(unverified_json.finality_anchor_hint(), None);
    assert_eq!(
        decoder(&encoded, &trust_anchor()).expect("decode anchored status"),
        status
    );
    assert_eq!(
        decode_offline_cash_operation_status_json_v1(&json, &trust_anchor())
            .expect("decode anchored JSON status"),
        status
    );
    assert!(matches!(
        decoder(
            &vec![0; OFFLINE_CASH_OPERATION_STATUS_MAX_BYTES_V1 + 1],
            &trust_anchor(),
        ),
        Err(OfflineCashApiErrorV1::EncodedSizeExceeded { .. })
    ));
    assert!(matches!(
        decode_unverified_offline_cash_operation_status_json_v1(&vec![
            0;
            OFFLINE_CASH_OPERATION_STATUS_JSON_MAX_BYTES_V1
                + 1
        ]),
        Err(OfflineCashApiErrorV1::EncodedSizeExceeded { .. })
    ));
}

#[test]
fn unverified_status_wrapper_does_not_expose_a_terminal_result() {
    let source = include_str!("offline_api.rs");
    let declaration = source
        .split_once("pub struct UnverifiedOfflineCashOperationStatusV1")
        .expect("unverified status declaration")
        .1
        .split_once("impl UnverifiedOfflineCashOperationStatusV1")
        .expect("unverified status implementation")
        .0;
    assert!(!declaration.contains("pub inner"));
    let implementation = source
        .split_once("impl UnverifiedOfflineCashOperationStatusV1")
        .expect("unverified status implementation")
        .1
        .split_once("impl OfflineCashReadinessV1")
        .expect("readiness implementation follows wrapper")
        .0;
    for forbidden in [
        "pub fn result",
        "pub fn into_inner",
        "pub fn finality_certificate",
    ] {
        assert!(
            !implementation.contains(forbidden),
            "unverified wrapper exposes `{forbidden}`"
        );
    }
    assert!(implementation.contains("pub fn finality_anchor_hint"));
    assert!(implementation.contains("pub fn verify_against"));
}

#[test]
fn public_schema_names_are_clean_v1_names() {
    assert_eq!(
        OFFLINE_CASH_TOP_UP_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.offline_cash.top_up.request"
    );
    assert_eq!(
        OFFLINE_CASH_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.offline_cash.redeem.request"
    );
    assert_eq!(
        OFFLINE_CASH_READINESS_SCHEMA_NAME_V1,
        "iroha.torii.v1.offline_cash.readiness.response"
    );
}

#[test]
fn clean_offline_cash_v1_surface_contains_no_legacy_fallbacks() {
    let source = include_str!("offline_api.rs");
    let forbidden = [
        ["Kage", "musha"].concat(),
        "receiver_lineage".to_owned(),
        "portable_offer".to_owned(),
        "provenance".to_owned(),
        "OfflineTopUpRequest".to_owned(),
        "OfflineRedeemRequest".to_owned(),
        "OfflineOperationReference".to_owned(),
        "OfflineOperationIdentity".to_owned(),
        "max_hops".to_owned(),
        "input_max".to_owned(),
        "proof_step".to_owned(),
        " V2".to_owned(),
        " V4".to_owned(),
        " V5".to_owned(),
    ];
    for retired in forbidden {
        assert!(
            !source.contains(&retired),
            "clean Offline Cash V1 surface retained `{retired}`"
        );
    }
    for required in [
        "OfflineCashTopUpRequestV1",
        "OfflineCashRedemptionRequestV1",
        "OfflineCashFinalityTrustAnchorV1",
    ] {
        assert!(source.contains(required));
    }
}
