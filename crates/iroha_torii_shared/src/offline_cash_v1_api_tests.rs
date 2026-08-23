use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    offline::{
        KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2,
        OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OfflineCashPairedProofV1,
        OfflineCashTransferStatementV1, offline_cash_receiver_key_reference_v1,
    },
};

const FIXTURE_PUBLIC_KEY_HEX: &str = "041e18532fd4754c02f3041d9c75ceb33b83ffd81ac7ce4fe882ccb1c98bc5896ea46c311c4e2ff40dd96a3653e6e45445d32dfe486eced75c7a90c6a18881c0a3";
const FIXTURE_REQUEST_SIGNATURE_HEX: &str = "710623ea92107972f5941bb99b4a8a0befb12a00e6c2a8c21e14fa98204dfc54042c045b5eeedfb52c9482cbafb4a44a95f8472039dbffd67658a749701848e0";
const FIXTURE_OTHER_REQUEST_SIGNATURE_HEX: &str = "d08f44598c21854fb9bd0ad67cbbbf5be69f7e5bbdde7fc17b1912e5288c0fbc5f4b1735e1488ee5c5f704846307a8972634e2b8535a5ecd86b1dab27af6bdef";
const FIXTURE_ACKNOWLEDGEMENT_SIGNATURE_HEX: &str = "e0aa7ce4bc306377b2a70b28e1f6fd9e269d7eefd1051f7c99d4e203c30c4d405aa182e83eefc8fe327fdc53d5c101b97e2ecdc1af5c430f9f7fe9ffab6862b5";

const LEGACY_REQUEST_V2_HEX: &str = include_str!(
    "../../connect_norito_bridge/tests/fixtures/offline_recipient_payment_request_v2.hex"
);
const LEGACY_PEER_PAYMENT_V4_HEX: &str =
    include_str!("../../connect_norito_bridge/tests/fixtures/offline_peer_payment_v4.hex");

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

fn account() -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn fixture_signature(hex: &str) -> KagemushaDeviceSignatureV2 {
    KagemushaDeviceSignatureV2::from_raw_bytes(
        &hex::decode(hex).expect("decode fixed P-256 signature"),
    )
    .expect("fixed canonical low-S signature")
}

fn decode_hex_fixture(encoded: &str) -> Vec<u8> {
    let compact: String = encoded
        .chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect();
    hex::decode(compact).expect("decode canonical hex fixture")
}

fn request_with_id(request_id: [u8; 32]) -> OfflineCashPaymentRequestV1 {
    let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(
        &hex::decode(FIXTURE_PUBLIC_KEY_HEX).expect("decode fixed P-256 public key"),
    )
    .expect("canonical public key");
    let signature = if request_id == [3; 32] {
        fixture_signature(FIXTURE_REQUEST_SIGNATURE_HEX)
    } else if request_id == [0x31; 32] {
        fixture_signature(FIXTURE_OTHER_REQUEST_SIGNATURE_HEX)
    } else {
        panic!("missing fixed request signature")
    };
    OfflineCashPaymentRequestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: [1; 32],
        network_id: network_id(),
        asset: asset(),
        scale: 4,
        amount: 12_345,
        recipient: account(),
        receiver_balance_commitment: [2; 32],
        recipient_key_reference: offline_cash_receiver_key_reference_v1(&public_key),
        receiver_public_key: public_key,
        request_id,
        issued_at_ms: 1_000,
        expires_at_ms: 61_000,
        hardware_policy_id: [4; 32],
        signature,
    }
}

fn request() -> OfflineCashPaymentRequestV1 {
    request_with_id([3; 32])
}

fn payment(request: &OfflineCashPaymentRequestV1) -> OfflineCashPaymentV1 {
    let request_digest = request.canonical_digest().expect("request digest");
    let statement = OfflineCashTransferStatementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: request.release_id,
        network_id: request.network_id,
        asset: request.asset.clone(),
        scale: request.scale,
        amount: request.amount,
        request_digest,
        sender_before: [5; 32],
        sender_after: [6; 32],
        receiver_before: request.receiver_balance_commitment,
        credit_commitment: [7; 32],
        transition_digest: [0; 32],
    }
    .seal_transition()
    .expect("seal transition");
    let semantic_digest = statement.canonical_digest().expect("statement digest");
    OfflineCashPaymentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        request_digest,
        statement,
        proof: OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_protocol_digest: [8; 32],
            ep_protocol_digest: [9; 32],
            semantic_digest,
            eq_proof: vec![0xA1; 128],
            ep_proof: vec![0xB2; 128],
            eq_history: vec![0xC3; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0xD4; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
        },
        encrypted_credit: vec![0xE5; 128],
        artifact_manifest_digest: [10; 32],
    }
}

fn acknowledgement(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> OfflineCashAcknowledgementV1 {
    OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: request.release_id,
        request_digest: request.canonical_digest().expect("request digest"),
        payment_digest: payment
            .canonical_digest_against(request)
            .expect("payment digest"),
        receiver_balance_commitment: [11; 32],
        acknowledged_at_ms: request.issued_at_ms + 1,
        signature: fixture_signature(FIXTURE_ACKNOWLEDGEMENT_SIGNATURE_HEX),
    }
}

fn assert_size_error(error: &KagemushaValidationError, actual: usize, max: usize) {
    assert!(
        matches!(
            error,
            KagemushaValidationError::EncodedSizeExceeded {
                actual: rejected_actual,
                max: rejected_max,
            } if *rejected_actual == actual && *rejected_max == max
        ),
        "unexpected size rejection: {error}"
    );
}

#[test]
fn public_boundary_roundtrips_only_the_exact_context_bound_v1_session() {
    let request = request();
    let payment = payment(&request);
    let acknowledgement = acknowledgement(&request, &payment);
    let request_bytes = norito::encode_canonical(&request).expect("encode request");
    let payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
    let acknowledgement_bytes =
        norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");

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

    let other_request = request_with_id([0x31; 32]);
    assert!(decode_offline_cash_payment_v1(&payment_bytes, &other_request).is_err());
    assert!(
        decode_offline_cash_acknowledgement_v1(&acknowledgement_bytes, &other_request, &payment,)
            .is_err()
    );
}

#[test]
fn public_boundary_pre_caps_every_message_before_decode() {
    let request = request();
    let payment = payment(&request);
    assert_size_error(
        &decode_offline_cash_payment_request_v1(&vec![
            0;
            OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 + 1
        ])
        .expect_err("oversized request must fail"),
        OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 + 1,
        OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
    );
    assert_size_error(
        &decode_offline_cash_payment_v1(&vec![0; OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1], &request)
            .expect_err("oversized payment must fail"),
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 + 1,
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
    );
    assert_size_error(
        &decode_offline_cash_acknowledgement_v1(
            &vec![0; OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1],
            &request,
            &payment,
        )
        .expect_err("oversized acknowledgement must fail"),
        OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1,
        OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
    );
}

#[test]
fn public_boundary_rejects_trailing_and_legacy_v4_v5_messages() {
    let request = request();
    let payment = payment(&request);
    let acknowledgement = acknowledgement(&request, &payment);

    let mut request_bytes = norito::encode_canonical(&request).expect("encode request");
    request_bytes.push(0);
    assert!(decode_offline_cash_payment_request_v1(&request_bytes).is_err());
    let mut payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
    payment_bytes.push(0);
    assert!(decode_offline_cash_payment_v1(&payment_bytes, &request).is_err());
    let mut acknowledgement_bytes =
        norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
    acknowledgement_bytes.push(0);
    assert!(
        decode_offline_cash_acknowledgement_v1(&acknowledgement_bytes, &request, &payment,)
            .is_err()
    );

    for legacy_version in [4, 5] {
        let mut legacy_request = request.clone();
        legacy_request.version = legacy_version;
        let legacy_request_bytes =
            norito::encode_canonical(&legacy_request).expect("encode legacy-version request");
        assert!(decode_offline_cash_payment_request_v1(&legacy_request_bytes).is_err());

        let mut legacy_payment = payment.clone();
        legacy_payment.version = legacy_version;
        let legacy_payment_bytes =
            norito::encode_canonical(&legacy_payment).expect("encode legacy-version payment");
        assert!(decode_offline_cash_payment_v1(&legacy_payment_bytes, &request).is_err());

        let mut legacy_acknowledgement = acknowledgement;
        legacy_acknowledgement.version = legacy_version;
        let legacy_acknowledgement_bytes = norito::encode_canonical(&legacy_acknowledgement)
            .expect("encode legacy-version acknowledgement");
        assert!(
            decode_offline_cash_acknowledgement_v1(
                &legacy_acknowledgement_bytes,
                &request,
                &payment,
            )
            .is_err()
        );
    }

    let actual_legacy_request = decode_hex_fixture(LEGACY_REQUEST_V2_HEX);
    assert!(actual_legacy_request.len() <= OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1);
    assert!(decode_offline_cash_payment_request_v1(&actual_legacy_request).is_err());
    let actual_legacy_payment = decode_hex_fixture(LEGACY_PEER_PAYMENT_V4_HEX);
    assert!(actual_legacy_payment.len() > OFFLINE_CASH_PAYMENT_MAX_BYTES_V1);
    assert!(decode_offline_cash_payment_v1(&actual_legacy_payment, &request).is_err());
    assert!(
        decode_offline_cash_acknowledgement_v1(&actual_legacy_payment, &request, &payment).is_err()
    );
}

#[test]
fn source_contract_has_no_generic_or_legacy_decode_fallback() {
    let source = include_str!("offline_api.rs");
    let request_delegate = source
        .split_once("pub fn decode_offline_cash_payment_request_v1")
        .expect("request delegate")
        .1
        .split_once("/// Decode one exact first-release sender payment")
        .expect("payment delegate follows request")
        .0;
    let payment_delegate = source
        .split_once("pub fn decode_offline_cash_payment_v1")
        .expect("payment delegate")
        .1
        .split_once("/// Decode one exact first-release acknowledgement")
        .expect("acknowledgement delegate follows payment")
        .0;
    let acknowledgement_delegate = source
        .split_once("pub fn decode_offline_cash_acknowledgement_v1")
        .expect("acknowledgement delegate")
        .1
        .split_once("/// Stable public Norito schema name")
        .expect("legacy API declarations follow acknowledgement")
        .0;
    assert!(
        request_delegate.contains("OfflineCashPaymentRequestV1::decode_canonical_exact(bytes)")
    );
    assert!(
        payment_delegate
            .contains("OfflineCashPaymentV1::decode_canonical_exact_against(bytes, request)")
    );
    assert!(acknowledgement_delegate.contains(
        "OfflineCashAcknowledgementV1::decode_canonical_exact_against(bytes, request, payment)"
    ));
    for delegate in [request_delegate, payment_delegate, acknowledgement_delegate] {
        for forbidden in [
            "decode_from_bytes",
            "decode_canonical_with_limits",
            "OfflineTopUpRequest",
            "OfflineRedeemRequest",
            "KagemushaRecursiveSpend",
        ] {
            assert!(
                !delegate.contains(forbidden),
                "public V1 delegate contains forbidden fallback `{forbidden}`"
            );
        }
    }
}
