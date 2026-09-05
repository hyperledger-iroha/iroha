//! Canonical cross-SDK KAGEMUSHA V1 three-message fixture authority.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    kagemusha::{
        KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1, KAGEMUSHA_COMPLETE_EXCHANGE_TARGET_BYTES_V1,
        KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
        KAGEMUSHA_WIRE_VERSION_V1, KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1,
        KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1, KagemushaAcknowledgementV1,
        KagemushaCommitCertificateV1, KagemushaCommitEvidenceV1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaEncryptedCreditEnvelopeV1,
        KagemushaHardwareCredentialV1, KagemushaHardwareTerminalBodyV1, KagemushaInboxReceiptV1,
        KagemushaPaymentOutputV1, KagemushaPaymentProofV1, KagemushaPaymentRequestV1,
        KagemushaPaymentV1, KagemushaTrustedCommitTimeV1, kagemusha_ciphertext_digest_v1,
        kagemusha_credit_opening_canonical_len_v1, kagemusha_device_key_reference_v1,
        kagemusha_inbox_receipt_commitment_v1, kagemusha_liability_pool_id_v1,
        kagemusha_payment_body_digest_v1, kagemusha_prepared_transfer_digest_v1,
        validate_kagemusha_complete_exchange_shape_v1,
    },
    nexus::AxtAssetIncarnationV1,
};
use norito::json::{self, Map, Value};
use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

const SHARED_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/offline/kagemusha_v1.json"
));

struct FixtureValuesV1 {
    request: KagemushaPaymentRequestV1,
    payment: KagemushaPaymentV1,
    acknowledgement: KagemushaAcknowledgementV1,
    terminal_body: KagemushaHardwareTerminalBodyV1,
}

fn digest(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn network() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"kagemusha-v1-three-message-fixture",
    )))
}

fn asset() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("fixture domain"),
        "xor".parse().expect("fixture asset name"),
    )
}

fn incarnation(network_id: &NetworkId, asset: &AssetDefinitionId) -> AxtAssetIncarnationV1 {
    let registration =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"kagemusha-v1-registration"));
    AxtAssetIncarnationV1::derive(
        network_id,
        asset,
        &registration,
        &Hash::new(b"kagemusha-v1-registration-execution"),
        1,
    )
}

fn account(tag: u8) -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![tag; 32], Algorithm::Ed25519)
        .expect("deterministic fixture account key");
    AccountId::new(key_pair.public_key().clone())
}

fn p256_signing_key(seed: u8) -> SigningKey {
    SigningKey::from_bytes((&[seed; 32]).into()).expect("deterministic P-256 signing key")
}

fn device_public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
    KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("canonical uncompressed P-256 device key")
}

fn sign(key: &SigningKey, bytes: &[u8]) -> KagemushaDeviceSignatureV1 {
    let signature: P256Signature = key.sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
        .expect("canonical low-S P-256 signature")
}

fn x25519_public_key(seed: u8) -> [u8; 32] {
    let secret = x25519_dalek::StaticSecret::from([seed; 32]);
    x25519_dalek::PublicKey::from(&secret).to_bytes()
}

fn fixture_values_v1() -> FixtureValuesV1 {
    let receiver_key = p256_signing_key(7);
    let governance_key = p256_signing_key(8);
    let receiver_public_key = device_public_key(&receiver_key);
    let network_id = network();
    let asset = asset();
    let asset_incarnation = incarnation(&network_id, &asset);

    let mut credential = KagemushaHardwareCredentialV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credential_id: [0; 32],
        network_id,
        hardware_profile_id: digest(0x21),
        suite_id: digest(0x22),
        firmware_policy_digest: digest(0x23),
        policy_epoch: 4,
        lane_commitment: digest(0x24),
        hardware_epoch_id: digest(0x25),
        hardware_epoch_generation: 1,
        device_public_key: receiver_public_key,
        device_key_reference: kagemusha_device_key_reference_v1(&receiver_public_key),
        issued_at_ms: 1,
        expires_at_ms: 20_000,
        governance_signature: sign(&governance_key, b"credential-placeholder"),
    }
    .seal_credential_id()
    .expect("credential identity");
    credential.governance_signature = sign(
        &governance_key,
        &credential
            .canonical_signing_bytes()
            .expect("credential signing bytes"),
    );

    let mut request = KagemushaPaymentRequestV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        release_id: digest(0x26),
        network_id,
        asset: asset.clone(),
        asset_incarnation,
        scale: 4,
        liability_pool_id: kagemusha_liability_pool_id_v1(&network_id, &asset, asset_incarnation)
            .expect("liability pool"),
        recipient: account(0x27),
        amount: 7,
        recipient_encryption_key: x25519_public_key(0x28),
        hardware_credential: credential,
        request_id: digest(0x29),
        issued_at_ms: 100,
        expires_at_ms: 10_000,
        signature: sign(&receiver_key, b"request-placeholder"),
    };
    request.signature = sign(
        &receiver_key,
        &request
            .canonical_signing_bytes()
            .expect("request signing bytes"),
    );
    request.validate_shape().expect("valid signed request");

    let output = KagemushaPaymentOutputV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        request_digest: request.canonical_digest().expect("request digest"),
        amount: request.amount,
        sender_before_commitment: digest(0x71),
        sender_after_commitment: digest(0x72),
        transition_nullifier: digest(0x82),
        credit_id: [0; 32],
        ciphertext_commitment: digest(0x83),
        commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(KagemushaTrustedCommitTimeV1 {
            time_evidence_commitment: digest(0x81),
        }),
        committed_at_ms: 500,
    }
    .seal_credit_id_against(&request)
    .expect("request-bound credit identity");
    output
        .validate_shape_against(&request)
        .expect("valid direct payment output");

    let encrypted_credit = KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key: x25519_public_key(0x84),
        nonce: [0x85; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            0x86;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("credit opening length")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(request.recipient_encryption_key)
    .expect("canonical encrypted credit envelope");

    let terminal_body = KagemushaHardwareTerminalBodyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        candidate_envelope_digest: digest(0x91),
        lifecycle_binding_digest: digest(0x92),
        transition_nullifier: output.transition_nullifier,
        outbox_reservation_commitment: digest(0x93),
        commit_evidence: output.commit_evidence,
        hardware_profile_id: digest(0x94),
        policy_epoch: 6,
        private_successor_commitment: digest(0x95),
        private_journal_commitment: digest(0x96),
        private_recovery_commitment: digest(0x97),
    };
    let commit_certificate = KagemushaCommitCertificateV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        certificate_id: [0; 32],
        candidate_envelope_digest: terminal_body.candidate_envelope_digest,
        lifecycle_binding_digest: terminal_body.lifecycle_binding_digest,
        transition_nullifier: terminal_body.transition_nullifier,
        outbox_reservation_commitment: terminal_body.outbox_reservation_commitment,
        commit_evidence: terminal_body.commit_evidence,
        hardware_profile_id: terminal_body.hardware_profile_id,
        policy_epoch: terminal_body.policy_epoch,
        hardware_terminal_commitment: [0; 32],
    }
    .seal_with_terminal_body(&terminal_body)
    .expect("terminal-bound commit certificate");

    let semantic_digest =
        kagemusha_payment_body_digest_v1(&output, &encrypted_credit).expect("payment body digest");
    let proof = KagemushaPaymentProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: digest(0x41),
        ep_protocol_digest: digest(0x42),
        semantic_digest,
        candidate_envelope_digest: commit_certificate.candidate_envelope_digest,
        commit_certificate_digest: commit_certificate
            .canonical_digest()
            .expect("commit certificate digest"),
        eq_deferred_audit: digest(0x45),
        ep_deferred_audit: digest(0x46),
        eq_proof: vec![0x49; 512],
        ep_proof: vec![0x4A; 512],
        eq_history: vec![0x47; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        ep_history: vec![0x48; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    };
    let payment = KagemushaPaymentV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        output,
        encrypted_credit,
        commit_certificate,
        proof,
    };
    payment
        .validate_shape_against(&request)
        .expect("valid direct payment");

    let payment_digest = payment
        .canonical_digest_against(&request)
        .expect("payment digest");
    let inbox_receipt = KagemushaInboxReceiptV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: payment.output.credit_id,
        receipt_commitment: kagemusha_inbox_receipt_commitment_v1(
            digest(0xA1),
            request.hardware_credential.hardware_epoch_id,
            1,
            payment.output.credit_id,
            payment_digest,
        )
        .expect("durable inbox receipt commitment"),
    };
    let mut acknowledgement = KagemushaAcknowledgementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        request_digest: request.canonical_digest().expect("request digest"),
        payment_digest,
        inbox_receipt,
        signature: sign(&receiver_key, b"acknowledgement-placeholder"),
    };
    acknowledgement.signature = sign(
        &receiver_key,
        &acknowledgement
            .canonical_signing_bytes()
            .expect("acknowledgement signing bytes"),
    );
    acknowledgement
        .validate_shape_against(&request, &payment)
        .expect("valid direct acknowledgement");

    FixtureValuesV1 {
        request,
        payment,
        acknowledgement,
        terminal_body,
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn encoded_section(raw: Vec<u8>, text: Option<String>, kind: Option<u8>) -> Value {
    let mut section = Map::new();
    if let Some(kind) = kind {
        section.insert("ipm1_kind".into(), Value::from(kind));
    }
    if let Some(text) = text {
        section.insert("kgm1".into(), Value::from(text));
    }
    section.insert("norito_hex".into(), Value::from(hex::encode(&raw)));
    section.insert("raw_bytes".into(), Value::from(raw.len()));
    section.insert("sha256".into(), Value::from(hex::encode(sha256(&raw))));
    Value::Object(section)
}

fn digest_section(domain: &str, digest: [u8; 32]) -> Value {
    norito::json!({
        "domain": domain,
        "hex": (hex::encode(digest)),
    })
}

fn canonical_fixture_v1() -> Value {
    let values = fixture_values_v1();
    let request_raw = norito::encode_canonical(&values.request).expect("encode request");
    let payment_raw = norito::encode_canonical(&values.payment).expect("encode payment");
    let acknowledgement_raw =
        norito::encode_canonical(&values.acknowledgement).expect("encode acknowledgement");
    let proof_raw = norito::encode_canonical(&values.payment.proof).expect("encode proof");
    let commit_certificate_raw = norito::encode_canonical(&values.payment.commit_certificate)
        .expect("encode commit certificate");
    let terminal_body_raw =
        norito::encode_canonical(&values.terminal_body).expect("encode terminal body");

    let request_text = values.request.encode_text().expect("encode request text");
    let payment_text = values
        .payment
        .encode_text_against(&values.request)
        .expect("encode payment text");
    let acknowledgement_text = values
        .acknowledgement
        .encode_text_against(&values.request, &values.payment)
        .expect("encode acknowledgement text");
    let request_digest = values.request.canonical_digest().expect("request digest");
    let request_signing_bytes = values
        .request
        .canonical_signing_bytes()
        .expect("request signing bytes");
    let payment_digest = values
        .payment
        .canonical_digest_against(&values.request)
        .expect("payment digest");
    let payment_output_digest = values
        .payment
        .output
        .canonical_digest()
        .expect("payment output digest");
    let ciphertext_digest = kagemusha_ciphertext_digest_v1(&values.payment.encrypted_credit);
    let payment_body_digest =
        kagemusha_payment_body_digest_v1(&values.payment.output, &values.payment.encrypted_credit)
            .expect("payment body digest");
    let commit_certificate_digest = values
        .payment
        .commit_certificate
        .canonical_digest()
        .expect("commit certificate digest");
    let prepared_transfer_digest = kagemusha_prepared_transfer_digest_v1(
        &values.request,
        values.payment.output.sender_before_commitment,
        values.payment.output.sender_after_commitment,
        values.payment.output.transition_nullifier,
        values.payment.output.ciphertext_commitment,
    )
    .expect("prepared transfer digest");
    let acknowledgement_digest = sha256(&acknowledgement_raw);
    let acknowledgement_signing_bytes = values
        .acknowledgement
        .canonical_signing_bytes()
        .expect("acknowledgement signing bytes");
    let raw_bytes = validate_kagemusha_complete_exchange_shape_v1(
        &values.request,
        &values.payment,
        &values.acknowledgement,
    )
    .expect("complete exchange shape");
    let text_bytes = request_text.len() + payment_text.len() + acknowledgement_text.len();

    norito::json!({
        "fixture_version": 1,
        "protocol": "KAGEMUSHA",
        "text_prefix": "kgm1:",
        "canonical_source": "Rust iroha_data_model KAGEMUSHA three-message Norito derivation; every SDK consumes these exact bytes",
        "proof_fixture_scope": "canonical shape vectors only; structural proof bytes do not qualify a release",
        "ipm1_message_order": [
            {"kind": "request", "tag": 1},
            {"kind": "payment", "tag": 2},
            {"kind": "acknowledgement", "tag": 3},
        ],
        "payment_request": (encoded_section(request_raw.clone(), Some(request_text), Some(1))),
        "payment": (encoded_section(payment_raw.clone(), Some(payment_text), Some(2))),
        "acknowledgement": (encoded_section(
            acknowledgement_raw.clone(),
            Some(acknowledgement_text),
            Some(3),
        )),
        "payment_proof": (encoded_section(proof_raw, None, None)),
        "commit_certificate": (encoded_section(commit_certificate_raw, None, None)),
        "request_digest": (digest_section("iroha:kagemusha:v1:payment-request", request_digest)),
        "payment_digest": (digest_section("iroha:kagemusha:v1:payment", payment_digest)),
        "acknowledgement_digest": {
            "algorithm": "sha256",
            "hex": (hex::encode(acknowledgement_digest)),
        },
        "prepared_transfer_digest": (digest_section(
            "iroha:kagemusha:v1:prepared-transfer",
            prepared_transfer_digest,
        )),
        "credit_id": {
            "hex": (hex::encode(values.payment.output.credit_id)),
        },
        "identity_vectors": {
            "payment_request_digest_hex": (hex::encode(request_digest)),
            "payment_digest_hex": (hex::encode(payment_digest)),
            "payment_output_digest_hex": (hex::encode(payment_output_digest)),
            "payment_body_digest_hex": (hex::encode(payment_body_digest)),
            "ciphertext_digest_hex": (hex::encode(ciphertext_digest)),
            "commit_certificate_digest_hex": (hex::encode(commit_certificate_digest)),
            "prepared_transfer_digest_hex": (hex::encode(prepared_transfer_digest)),
            "credit_id_hex": (hex::encode(values.payment.output.credit_id)),
            "transition_nullifier_hex": (hex::encode(values.payment.output.transition_nullifier)),
            "inbox_receipt_commitment_hex": (hex::encode(
                values.acknowledgement.inbox_receipt.receipt_commitment,
            )),
            "hardware_terminal_commitment_hex": (hex::encode(
                values.payment.commit_certificate.hardware_terminal_commitment,
            )),
        },
        "semantic_transcripts": {
            "payment_request": {
                "signing_domain": "iroha:kagemusha:v1:payment-request-signing",
                "signing_bytes_hex": (hex::encode(request_signing_bytes)),
                "canonical_digest_hex": (hex::encode(request_digest)),
            },
            "payment_body": {
                "digest_domain": "iroha:kagemusha:v1:payment-body",
                "output_digest_hex": (hex::encode(payment_output_digest)),
                "ciphertext_digest_hex": (hex::encode(ciphertext_digest)),
                "canonical_digest_hex": (hex::encode(payment_body_digest)),
            },
            "prepared_transfer": {
                "digest_domain": "iroha:kagemusha:v1:prepared-transfer",
                "request_digest_hex": (hex::encode(request_digest)),
                "sender_before_commitment_hex": (hex::encode(
                    values.payment.output.sender_before_commitment,
                )),
                "sender_after_commitment_hex": (hex::encode(
                    values.payment.output.sender_after_commitment,
                )),
                "transition_nullifier_hex": (hex::encode(
                    values.payment.output.transition_nullifier,
                )),
                "ciphertext_commitment_hex": (hex::encode(
                    values.payment.output.ciphertext_commitment,
                )),
                "canonical_digest_hex": (hex::encode(prepared_transfer_digest)),
            },
            "acknowledgement": {
                "signing_domain": "iroha:kagemusha:v1:acknowledgement-signing",
                "signing_bytes_hex": (hex::encode(acknowledgement_signing_bytes)),
                "canonical_sha256_hex": (hex::encode(acknowledgement_digest)),
            },
        },
        "hardware_terminal_commitment": {
            "domain": "iroha:kagemusha:v1:hardware-terminal-body",
            "terminal_body_norito_hex": (hex::encode(terminal_body_raw)),
            "hex": (hex::encode(values.payment.commit_certificate.hardware_terminal_commitment)),
        },
        "complete_three_message": {
            "messages": ["payment_request", "payment", "acknowledgement"],
            "raw_bytes": raw_bytes,
            "raw_target_bytes": KAGEMUSHA_COMPLETE_EXCHANGE_TARGET_BYTES_V1,
            "raw_hard_cap_bytes": KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1,
            "text_bytes": text_bytes,
            "text_hard_cap_bytes": KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1,
            "within_raw_target": (raw_bytes <= KAGEMUSHA_COMPLETE_EXCHANGE_TARGET_BYTES_V1),
            "within_raw_hard_cap": (raw_bytes <= KAGEMUSHA_COMPLETE_EXCHANGE_MAX_BYTES_V1),
            "within_text_hard_cap": (text_bytes <= KAGEMUSHA_COMPLETE_TEXT_EXCHANGE_MAX_BYTES_V1),
        },
    })
}

#[test]
fn shared_kagemusha_v1_fixture_matches_rust_authority() {
    let expected = canonical_fixture_v1();
    let rendered = format!(
        "{}\n",
        json::to_string_pretty(&expected).expect("render canonical fixture")
    );
    if let Some(destination) = std::env::var_os("PRINT_KAGEMUSHA_FIXTURE_V1") {
        if destination != "1" {
            let destination = std::path::PathBuf::from(destination);
            let destination = if destination.is_absolute() {
                destination
            } else {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../..")
                    .join(destination)
            };
            std::fs::write(&destination, &rendered)
                .expect("write requested canonical KAGEMUSHA V1 fixture");
        }
        print!("{rendered}");
        return;
    }

    let actual: Value = json::from_str(SHARED_FIXTURE).expect("parse shared KAGEMUSHA fixture");
    assert_eq!(
        actual, expected,
        "regenerate fixtures/offline/kagemusha_v1.json from this Rust authority"
    );
    assert_eq!(rendered, SHARED_FIXTURE, "fixture JSON bytes are canonical");
}

#[test]
fn distinct_payments_against_one_request_are_independently_valid() {
    let first = fixture_values_v1();
    let request = &first.request;

    let mut output = first.payment.output.clone();
    output.sender_before_commitment = digest(0x73);
    output.sender_after_commitment = digest(0x74);
    output.transition_nullifier = digest(0x87);
    output.credit_id = [0; 32];
    let output = output
        .seal_credit_id_against(request)
        .expect("second request-bound credit identity");

    let encrypted_credit = KagemushaEncryptedCreditEnvelopeV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        ephemeral_x25519_public_key: x25519_public_key(0x88),
        nonce: [0x89; KAGEMUSHA_XCHACHA20POLY1305_NONCE_BYTES_V1],
        ciphertext_and_tag: vec![
            0x8A;
            kagemusha_credit_opening_canonical_len_v1()
                .expect("credit opening length")
                + KAGEMUSHA_XCHACHA20POLY1305_TAG_BYTES_V1
        ],
    }
    .canonical_bytes_against_recipient_key(request.recipient_encryption_key)
    .expect("second canonical encrypted credit envelope");

    let terminal_body = KagemushaHardwareTerminalBodyV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        candidate_envelope_digest: digest(0xA2),
        lifecycle_binding_digest: digest(0xA3),
        transition_nullifier: output.transition_nullifier,
        outbox_reservation_commitment: digest(0xA4),
        commit_evidence: output.commit_evidence,
        hardware_profile_id: digest(0xA5),
        policy_epoch: 7,
        private_successor_commitment: digest(0xA6),
        private_journal_commitment: digest(0xA7),
        private_recovery_commitment: digest(0xA8),
    };
    let commit_certificate = KagemushaCommitCertificateV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        certificate_id: [0; 32],
        candidate_envelope_digest: terminal_body.candidate_envelope_digest,
        lifecycle_binding_digest: terminal_body.lifecycle_binding_digest,
        transition_nullifier: terminal_body.transition_nullifier,
        outbox_reservation_commitment: terminal_body.outbox_reservation_commitment,
        commit_evidence: terminal_body.commit_evidence,
        hardware_profile_id: terminal_body.hardware_profile_id,
        policy_epoch: terminal_body.policy_epoch,
        hardware_terminal_commitment: [0; 32],
    }
    .seal_with_terminal_body(&terminal_body)
    .expect("second terminal-bound commit certificate");

    let mut proof = first.payment.proof.clone();
    proof.semantic_digest =
        kagemusha_payment_body_digest_v1(&output, &encrypted_credit).expect("payment body digest");
    proof.candidate_envelope_digest = commit_certificate.candidate_envelope_digest;
    proof.commit_certificate_digest = commit_certificate
        .canonical_digest()
        .expect("second commit certificate digest");
    let second_payment = KagemushaPaymentV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        output,
        encrypted_credit,
        commit_certificate,
        proof,
    };
    second_payment
        .validate_shape_against(request)
        .expect("second independent payment against the same request");

    let payment_digest = second_payment
        .canonical_digest_against(request)
        .expect("second payment digest");
    let receiver_key = p256_signing_key(7);
    let inbox_receipt = KagemushaInboxReceiptV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        credit_id: second_payment.output.credit_id,
        receipt_commitment: kagemusha_inbox_receipt_commitment_v1(
            digest(0xA9),
            request.hardware_credential.hardware_epoch_id,
            2,
            second_payment.output.credit_id,
            payment_digest,
        )
        .expect("second durable inbox receipt commitment"),
    };
    let mut second_acknowledgement = KagemushaAcknowledgementV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        request_digest: request.canonical_digest().expect("request digest"),
        payment_digest,
        inbox_receipt,
        signature: sign(&receiver_key, b"second-acknowledgement-placeholder"),
    };
    second_acknowledgement.signature = sign(
        &receiver_key,
        &second_acknowledgement
            .canonical_signing_bytes()
            .expect("second acknowledgement signing bytes"),
    );

    validate_kagemusha_complete_exchange_shape_v1(request, &first.payment, &first.acknowledgement)
        .expect("first valid payment remains accepted");
    validate_kagemusha_complete_exchange_shape_v1(
        request,
        &second_payment,
        &second_acknowledgement,
    )
    .expect("second valid payment against the same request is accepted");
    assert_ne!(
        first.payment.output.credit_id,
        second_payment.output.credit_id
    );
    assert_eq!(
        first.payment.output.request_digest,
        second_payment.output.request_digest
    );
}
