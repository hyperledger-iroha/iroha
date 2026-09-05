//! Focused tests for the closed Torii KAGEMUSHA V1 surface.

use super::*;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    Level, NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::{BlockHeader, consensus_v2::HeightContextId},
    domain::DomainId,
    isi::{Log, kagemusha_v1::TopUpKagemushaV1},
    kagemusha::{
        KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1,
        KAGEMUSHA_WIRE_VERSION_V1, KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1,
        KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaEncryptedCreditEnvelopeV1,
        KagemushaHardwareCredentialV1, KagemushaMintAuthorizationV1, KagemushaPairedProofV1,
        kagemusha_credit_opening_canonical_len_v1, kagemusha_device_key_reference_v1,
        kagemusha_liability_pool_id_v1,
    },
    nexus::AxtAssetIncarnationV1,
    testing::kagemusha::KagemushaFixtureSignerV1,
    transaction::{FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder},
};

const FIXTURE_TOP_UP_PUBLIC_KEY_HEX: &str = "04209c317b637935dd3da1c54f63495dfb31f97d293df085710320595c9aacb83fdde4c69fc17a0c74c20cc692662f049892ba37a4ba47d2c70cd8a99986391f9b";

fn network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"torii-shared-kagemusha-v1",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    )
}

fn asset_incarnation(seed: u8) -> AxtAssetIncarnationV1 {
    AxtAssetIncarnationV1::try_from_bytes(*Hash::new([seed]).as_ref())
        .expect("canonical asset incarnation")
}

fn account(seed: u8) -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn fixture_public_key(encoded: &str) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        &hex::decode(encoded).expect("decode fixed P-256 public key"),
    )
    .expect("canonical fixed P-256 public key")
}

fn signing_key(seed: u8) -> KagemushaFixtureSignerV1 {
    KagemushaFixtureSignerV1::from_repeated_byte(seed)
}

fn signing_device_public_key(key: &KagemushaFixtureSignerV1) -> KagemushaDevicePublicKeyV1 {
    key.device_public_key()
}

fn sign_device(key: &KagemushaFixtureSignerV1, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
    key.sign(bytes)
}

const fn suite_id() -> [u8; 32] {
    [0x10; 32]
}

fn hardware_credential(
    network_id: NetworkId,
    lane_commitment: [u8; 32],
    device_public_key: KagemushaDevicePublicKeyV1,
    tag: u8,
) -> KagemushaHardwareCredentialV1 {
    let credential = KagemushaHardwareCredentialV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id,
        hardware_profile_id: [tag; 32],
        suite_id: suite_id(),
        firmware_policy_digest: [tag.wrapping_add(1); 32],
        policy_epoch: 1,
        lane_commitment,
        hardware_epoch_id: [tag.wrapping_add(2); 32],
        hardware_epoch_generation: 1,
        device_public_key,
        device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
        issued_at_ms: 500,
        expires_at_ms: 90_000,
        governance_signature: sign_device(&signing_key(3), b"fixture governance credential"),
    }
    .seal_credential_id()
    .expect("seal hardware credential identity");
    credential
        .validate_shape()
        .expect("valid hardware credential shape");
    credential
}

fn recipient_encryption_key(tag: u8) -> [u8; 32] {
    let mut key = [0; 32];
    key[0] = tag;
    key
}

fn encrypted_credit(recipient_key: [u8; 32], tag: u8) -> Vec<u8> {
    let mut ephemeral_x25519_public_key = [0; 32];
    ephemeral_x25519_public_key[0] = tag.wrapping_add(1);
    KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key,
        nonce: [tag; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            tag;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("credit opening length")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(recipient_key)
    .expect("canonical encrypted credit")
}

fn paired_proof(semantic_digest: [u8; 32], tag: u8) -> KagemushaPairedProofV1 {
    KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: [tag; 32],
        ep_protocol_digest: [tag.wrapping_add(1); 32],
        semantic_digest,
        guard_eq_credential_audit: [tag.wrapping_add(2); 32],
        guard_ep_credential_audit: [tag.wrapping_add(3); 32],
        eq_deferred_audit: [tag.wrapping_add(4); 32],
        ep_deferred_audit: [tag.wrapping_add(5); 32],
        eq_proof: vec![tag.wrapping_add(6); 128],
        ep_proof: vec![tag.wrapping_add(7); 128],
        eq_history: vec![tag.wrapping_add(8); KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: vec![tag.wrapping_add(9); KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    }
}

fn unchecked_payment_request_with_id(request_id: [u8; 32]) -> KagemushaPaymentRequestV1 {
    let receiver_key = signing_key(7);
    let recipient_public_key = signing_device_public_key(&receiver_key);
    let network_id = network_id();
    let asset = asset();
    let asset_incarnation = asset_incarnation(1);
    let recipient_lane_id = [2; 32];
    let mut request = KagemushaPaymentRequestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: [1; 32],
        network_id,
        asset: asset.clone(),
        asset_incarnation,
        scale: 4,
        liability_pool_id: kagemusha_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
            .expect("liability pool"),
        recipient: account(0x32),
        amount: 12_345,
        recipient_encryption_key: recipient_encryption_key(0x29),
        hardware_credential: hardware_credential(
            network_id,
            recipient_lane_id,
            recipient_public_key,
            0x41,
        ),
        request_id,
        issued_at_ms: 1_000,
        expires_at_ms: 61_000,
        signature: sign_device(&receiver_key, b"request-placeholder"),
    };
    request.signature = sign_device(
        &receiver_key,
        &request
            .canonical_signing_bytes()
            .expect("request signing bytes"),
    );
    request
}

fn payment_request_with_id(request_id: [u8; 32]) -> KagemushaPaymentRequestV1 {
    let request = unchecked_payment_request_with_id(request_id);
    request.validate_shape().expect("valid payment request");
    request
}

fn payment_request() -> KagemushaPaymentRequestV1 {
    decode_kagemusha_payment_request_v1(&fixture_message_bytes("payment_request"))
        .expect("decode Rust-owned canonical payment request")
}

fn fixture_message_bytes(name: &str) -> Vec<u8> {
    let fixture: norito::json::Value =
        norito::json::from_str(include_str!("../../../fixtures/offline/kagemusha_v1.json"))
            .expect("Rust-owned canonical KAGEMUSHA fixture JSON");
    let encoded = fixture
        .get(name)
        .and_then(|message| message.get("norito_hex"))
        .and_then(norito::json::Value::as_str)
        .expect("canonical fixture contains the exact current message kind");
    hex::decode(encoded).expect("canonical fixture Norito bytes")
}

fn peer_fixture() -> (
    KagemushaPaymentRequestV1,
    KagemushaPaymentV1,
    KagemushaAcknowledgementV1,
) {
    let request = payment_request();
    let payment = decode_kagemusha_payment_v1(&fixture_message_bytes("payment"), &request)
        .expect("decode Rust-owned committed payment");
    let acknowledgement = decode_kagemusha_acknowledgement_v1(
        &fixture_message_bytes("acknowledgement"),
        &request,
        &payment,
    )
    .expect("decode Rust-owned acknowledgement");
    (request, payment, acknowledgement)
}

fn top_up_request_for_network(network_id: NetworkId) -> KagemushaTopUpRequestV1 {
    let recipient_public_key = fixture_public_key(FIXTURE_TOP_UP_PUBLIC_KEY_HEX);
    let asset = asset();
    let asset_incarnation = asset_incarnation(1);
    let recipient_lane_id = [0x23; 32];
    let recipient_one_time_key = recipient_encryption_key(0x29);
    let request = KagemushaTopUpRequestV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: [0x21; 32],
        issuance_commitment: [0; 32],
        credit_id: [0; 32],
        release_id: [0x22; 32],
        suite_id: suite_id(),
        vk_digest: [0x24; 32],
        network_id,
        asset: asset.clone(),
        asset_incarnation,
        scale: 4,
        amount: 25_000,
        liability_pool_id: kagemusha_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
            .expect("liability pool"),
        payer: account(0x31),
        recipient: account(0x32),
        hardware_credential: hardware_credential(
            network_id,
            recipient_lane_id,
            recipient_public_key,
            0x71,
        ),
        recipient_credential_commitment: [0x25; 32],
        credit_commitment: [0x26; 32],
        recipient_one_time_key,
        encrypted_credit: encrypted_credit(recipient_one_time_key, 0x27),
        artifact_manifest_digest: [0x28; 32],
        mint_authorization: None,
    }
    .seal_identifiers()
    .expect("seal top-up identifiers");
    let statement = request
        .mint_authorization_statement()
        .expect("mint authorization statement");
    let proof = paired_proof(
        statement
            .canonical_digest()
            .expect("mint authorization semantic digest"),
        0x81,
    );
    request
        .attach_mint_authorization(KagemushaMintAuthorizationV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            statement,
            proof,
        })
        .expect("attach mint authorization")
}

fn top_up_request() -> KagemushaTopUpRequestV1 {
    top_up_request_for_network(network_id())
}

fn trust_anchor() -> KagemushaFinalityTrustAnchorV1 {
    KagemushaFinalityTrustAnchorV1 {
        network_id: network_id(),
        block_height: 7,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"kagemusha-v1-pinned-context",
        ))),
    }
}

#[test]
fn peer_boundary_roundtrips_only_the_exact_context_bound_three_message_exchange() {
    let (request, payment, acknowledgement) = peer_fixture();
    let request_bytes = norito::encode_canonical(&request).expect("encode request");
    let payment_bytes = norito::encode_canonical(&payment).expect("encode payment");
    let acknowledgement_bytes =
        norito::encode_canonical(&acknowledgement).expect("encode acknowledgement");
    let other_request = payment_request_with_id([0x31; 32]);
    assert!(request_bytes.len() <= KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1);
    assert!(payment_bytes.len() <= KAGEMUSHA_PAYMENT_MAX_BYTES_V1);
    assert!(acknowledgement_bytes.len() <= KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1);
    for (name, bytes) in [
        ("payment_request", &request_bytes),
        ("payment", &payment_bytes),
        ("acknowledgement", &acknowledgement_bytes),
    ] {
        assert_eq!(
            *bytes,
            fixture_message_bytes(name),
            "canonical {name} bytes"
        );
    }
    assert_eq!(
        validate_kagemusha_complete_exchange_v1(&request, &payment, &acknowledgement,)
            .expect("validate complete exchange"),
        request_bytes.len() + payment_bytes.len() + acknowledgement_bytes.len(),
    );

    assert!(decode_kagemusha_payment_v1(&payment_bytes, &other_request).is_err());
    assert!(
        decode_kagemusha_acknowledgement_v1(&acknowledgement_bytes, &other_request, &payment)
            .is_err()
    );
}

#[test]
fn peer_boundary_pre_caps_every_message_and_rejects_trailing_bytes() {
    let (request, payment, _) = peer_fixture();
    assert!(matches!(
        decode_kagemusha_payment_request_v1(&vec![0; KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1 + 1]),
        Err(KagemushaValidationErrorV1::EncodedSizeExceeded { .. })
    ));
    assert!(matches!(
        decode_kagemusha_payment_v1(&vec![0; KAGEMUSHA_PAYMENT_MAX_BYTES_V1 + 1], &request),
        Err(KagemushaValidationErrorV1::EncodedSizeExceeded { .. })
    ));
    assert!(matches!(
        decode_kagemusha_acknowledgement_v1(
            &vec![0; KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1 + 1],
            &request,
            &payment,
        ),
        Err(KagemushaValidationErrorV1::EncodedSizeExceeded { .. })
    ));

    let mut encoded = norito::encode_canonical(&request).expect("encode request");
    encoded.push(0);
    assert!(decode_kagemusha_payment_request_v1(&encoded).is_err());
    let mut encoded = fixture_message_bytes("payment");
    encoded.push(0);
    assert!(decode_kagemusha_payment_v1(&encoded, &request).is_err());
    let mut encoded = fixture_message_bytes("acknowledgement");
    encoded.push(0);
    assert!(decode_kagemusha_acknowledgement_v1(&encoded, &request, &payment).is_err());
}

#[test]
fn peer_payment_rejects_post_commit_proof_and_certificate_substitution() {
    let (request, payment, _) = peer_fixture();
    assert_eq!(
        payment.proof.commit_certificate_digest,
        payment
            .commit_certificate
            .canonical_digest()
            .expect("certificate digest"),
    );
    let mut substituted_payment = payment.clone();
    substituted_payment.proof.commit_certificate_digest[0] ^= 1;
    let encoded = norito::encode_canonical(&substituted_payment).expect("encode substituted proof");
    assert!(decode_kagemusha_payment_v1(&encoded, &request).is_err());
    let mut substituted_payment = payment;
    substituted_payment.commit_certificate.transition_nullifier[0] ^= 1;
    let encoded =
        norito::encode_canonical(&substituted_payment).expect("encode substituted certificate");
    assert!(decode_kagemusha_payment_v1(&encoded, &request).is_err());
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
        "recipient_one_time_key",
    ] {
        assert!(
            !json.contains(forbidden),
            "payment request exposed forbidden field `{forbidden}`"
        );
    }
    assert!(json.contains("lane_commitment"));
    assert!(json.contains("amount"));
    assert!(json.contains("recipient_encryption_key"));
    assert!(json.contains("request_id"));
}

#[test]
fn readiness_is_the_exact_four_field_v1_contract() {
    let readiness = KagemushaReadinessV1 {
        kagemusha_handoff_capability: KAGEMUSHA_HANDOFF_CAPABILITY_V1.to_owned(),
        wire_version: KAGEMUSHA_WIRE_VERSION_V1,
        device_lifecycle_version: KAGEMUSHA_DEVICE_LIFECYCLE_VERSION_V1,
        ready: true,
    };
    readiness.validate().expect("valid readiness");
    let json = norito::json::to_string(&readiness).expect("encode readiness");
    let value: norito::json::Value = norito::json::from_str(&json).expect("read readiness JSON");
    let object = value.as_object().expect("readiness object");
    assert_eq!(object.len(), 4);
    for field in [
        "kagemusha_handoff_capability",
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
    assert!(norito::json::from_str::<KagemushaReadinessV1>(&unknown).is_err());
    let mut wrong = readiness;
    wrong.wire_version += 1;
    assert!(wrong.validate().is_err());
}

#[test]
fn top_up_keeps_distinct_payer_and_recipient_and_decodes_exactly() {
    let request = top_up_request();
    assert_ne!(request.payer, request.recipient);
    let encoded = norito::encode_canonical(&request).expect("encode top-up request");
    assert!(encoded.len() <= KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1);
    assert_eq!(
        decode_kagemusha_top_up_request_v1(&encoded).expect("decode top-up request"),
        request
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_kagemusha_top_up_request_v1(&trailing).is_err());
    assert!(matches!(
        decode_kagemusha_top_up_request_v1(&vec![0; KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1 + 1]),
        Err(KagemushaApiErrorV1::EncodedSizeExceeded { .. })
    ));
}

#[test]
fn maximum_shape_top_up_request_fits_the_fixed_v1_ceiling() {
    let mut request = top_up_request();
    let proof = &mut request
        .mint_authorization
        .as_mut()
        .expect("mint authorization")
        .proof;
    proof.eq_proof = vec![0x91; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1];
    proof.ep_proof = vec![0x92; KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1];
    request.validate_shape().expect("maximum-shape top-up");

    let encoded = norito::encode_canonical(&request).expect("encode maximum-shape top-up");
    assert!(
        encoded.len() > 4 * 1024,
        "regression fixture must exceed the retired 4 KiB ceiling"
    );
    assert!(encoded.len() <= KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1);
    assert_eq!(
        decode_kagemusha_top_up_request_v1(&encoded).expect("decode maximum-shape top-up"),
        request
    );

    let payer_key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let transaction = TransactionBuilder::new(
        request.network_id,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([TopUpKagemushaV1::new(request).expect("maximum-shape instruction")])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(payer_key.private_key())
    .expect("sign maximum-shape top-up");
    let transaction_bytes = transaction
        .encode_wire_v1()
        .expect("encode maximum-shape signed transaction");
    assert!(
        transaction_bytes.len() <= KAGEMUSHA_TOP_UP_SIGNED_TRANSACTION_MIN_INGRESS_BYTES_V1,
        "the enabled-route provisioning floor must admit the maximum V1 top-up shape"
    );
}

#[test]
fn payer_signed_top_up_transaction_enforces_exact_envelope_authority() {
    let request = top_up_request();
    let payer_key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    assert_eq!(
        request.payer,
        AccountId::new(payer_key.public_key().clone())
    );
    let transaction = TransactionBuilder::new(
        request.network_id,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([TopUpKagemushaV1::new(request.clone()).expect("top-up instruction")])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(payer_key.private_key())
    .expect("payer-signed top-up");
    assert_eq!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &transaction)
            .expect("valid payer-signed top-up"),
        &request
    );

    let other_key = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
    let wrong_authority = AccountId::new(other_key.public_key().clone());
    let mismatched = TransactionBuilder::new(
        request.network_id,
        wrong_authority,
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([TopUpKagemushaV1::new(request.clone()).expect("top-up instruction")])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(other_key.private_key())
    .expect("differently authorized transaction");
    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &mismatched),
        Err(KagemushaApiErrorV1::TopUpTransactionAuthorityMismatch)
    ));
}

#[test]
fn payer_signed_top_up_rejects_wrong_network_signature_and_instruction_count() {
    let request = top_up_request();
    let payer_key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let top_up = TopUpKagemushaV1::new(request.clone()).expect("top-up instruction");
    let wrong_network =
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([
            0xA5,
        ])));
    let transaction = TransactionBuilder::new(
        wrong_network,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([top_up.clone()])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(payer_key.private_key())
    .expect("wrong-network top-up");
    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &transaction),
        Err(KagemushaApiErrorV1::TopUpTransactionWrongNetwork)
    ));

    let invalid_signature = TransactionBuilder::new(
        request.network_id,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([top_up.clone()])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .build_with_signature(Signature::from_bytes(&[]));
    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &invalid_signature),
        Err(KagemushaApiErrorV1::TopUpTransactionSignatureInvalid)
    ));

    let two_instructions = TransactionBuilder::new(
        request.network_id,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([top_up.clone(), top_up])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(payer_key.private_key())
    .expect("two-instruction transaction");
    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &two_instructions),
        Err(KagemushaApiErrorV1::TopUpTransactionShapeInvalid)
    ));

    let wrong_instruction = TransactionBuilder::new(
        request.network_id,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(Level::INFO, "not a top-up".to_owned())])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(payer_key.private_key())
    .expect("wrong-instruction transaction");
    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &wrong_instruction),
        Err(KagemushaApiErrorV1::TopUpTransactionShapeInvalid)
    ));
}

#[test]
fn payer_signed_top_up_rejects_an_embedded_request_for_another_network() {
    let expected_network = network_id();
    let embedded_network =
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new([
            0xA6,
        ])));
    let request = top_up_request_for_network(embedded_network);
    let payer_key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let transaction = TransactionBuilder::new(
        expected_network,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([TopUpKagemushaV1::new(request).expect("top-up instruction")])
    .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
    .try_sign(payer_key.private_key())
    .expect("payer-signed top-up with a foreign embedded network");

    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&expected_network, &transaction),
        Err(KagemushaApiErrorV1::TopUpTransactionWrongNetwork)
    ));
}

#[test]
fn payer_signed_top_up_rejects_ordinary_admission_intent() {
    let request = top_up_request();
    let payer_key = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let transaction = TransactionBuilder::new(
        request.network_id,
        request.payer.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([TopUpKagemushaV1::new(request.clone()).expect("top-up instruction")])
    .with_admission_intent(TransactionAdmissionIntent::Ordinary)
    .try_sign(payer_key.private_key())
    .expect("ordinary-admission top-up");
    assert!(matches!(
        validate_kagemusha_top_up_signed_transaction_v1(&request.network_id, &transaction),
        Err(KagemushaApiErrorV1::TopUpTransactionAdmissionIntentInvalid)
    ));
}

#[test]
fn operation_status_decoder_requires_an_external_finality_anchor() {
    let decoder: fn(
        &[u8],
        &KagemushaFinalityTrustAnchorV1,
    ) -> Result<KagemushaOperationStatusV1, KagemushaApiErrorV1> =
        decode_kagemusha_operation_status_v1;
    let status = KagemushaOperationStatusV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: [0x41; 32],
        kind: KagemushaOperationKindV1::TopUp,
        state: KagemushaOperationStateV1::Pending,
        result: None,
        rejection: None,
    };
    let encoded = norito::encode_canonical(&status).expect("encode pending status");
    let unverified = decode_unverified_kagemusha_operation_status_v1(&encoded)
        .expect("bounded structural decode");
    assert_eq!(unverified.operation_id(), status.operation_id);
    assert_eq!(unverified.kind(), status.kind);
    assert_eq!(unverified.state(), status.state);
    assert_eq!(unverified.finality_anchor_hint(), None);
    let json = norito::json::to_vec(&status).expect("encode pending status JSON");
    let unverified_json = decode_unverified_kagemusha_operation_status_json_v1(&json)
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
        decode_kagemusha_operation_status_json_v1(&json, &trust_anchor())
            .expect("decode anchored JSON status"),
        status
    );
    assert!(matches!(
        decoder(
            &vec![0; KAGEMUSHA_OPERATION_STATUS_MAX_BYTES_V1 + 1],
            &trust_anchor(),
        ),
        Err(KagemushaApiErrorV1::EncodedSizeExceeded { .. })
    ));
    assert!(matches!(
        decode_unverified_kagemusha_operation_status_json_v1(&vec![
            0;
            KAGEMUSHA_OPERATION_STATUS_JSON_MAX_BYTES_V1
                + 1
        ]),
        Err(KagemushaApiErrorV1::EncodedSizeExceeded { .. })
    ));
}

#[test]
fn unverified_status_wrapper_does_not_expose_a_terminal_result() {
    let source = include_str!("kagemusha_api.rs");
    let declaration = source
        .split_once("pub struct UnverifiedKagemushaOperationStatusV1")
        .expect("unverified status declaration")
        .1
        .split_once("impl UnverifiedKagemushaOperationStatusV1")
        .expect("unverified status implementation")
        .0;
    assert!(!declaration.contains("pub inner"));
    let implementation = source
        .split_once("impl UnverifiedKagemushaOperationStatusV1")
        .expect("unverified status implementation")
        .1
        .split_once("impl KagemushaReadinessV1")
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
        KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.top_up.request"
    );
    assert_eq!(
        KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.redeem.request"
    );
    assert_eq!(
        KAGEMUSHA_TOP_UP_SIGNED_TRANSACTION_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.top_up.signed_transaction"
    );
    assert_eq!(
        KAGEMUSHA_READINESS_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.readiness.response"
    );
}

#[test]
fn clean_kagemusha_v1_surface_contains_no_legacy_fallbacks() {
    let source = include_str!("kagemusha_api.rs");
    let retired_prefix = ["line", "Off"].into_iter().rev().collect::<String>();
    let forbidden = vec![
        format!("{retired_prefix}Cash"),
        "receiver_lineage".to_owned(),
        "portable_offer".to_owned(),
        "provenance".to_owned(),
        format!("{retired_prefix}TopUpRequest"),
        format!("{retired_prefix}RedeemRequest"),
        format!("{retired_prefix}OperationReference"),
        format!("{retired_prefix}OperationIdentity"),
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
            "clean KAGEMUSHA V1 surface retained `{retired}`"
        );
    }
    for required in [
        "KagemushaTopUpRequestV1",
        "KagemushaRedemptionRequestV1",
        "KagemushaFinalityTrustAnchorV1",
    ] {
        assert!(source.contains(required));
    }
}
