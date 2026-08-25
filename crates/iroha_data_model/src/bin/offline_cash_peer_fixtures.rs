//! Regenerate the authoritative Offline Cash V1 peer-transport fixture.
//!
//! Run with
//! `cargo iroha-fast -- run --locked --offline -p iroha_data_model --features dev-tools,test-fixtures --bin offline_cash_peer_fixtures`
//! to refresh `fixtures/offline/offline_cash_peer_transport_v1.json`. Pass
//! `--check` to fail when the checked-in fixture differs from the values emitted
//! by the current Rust data model and canonical peer adapter.

use std::{any::type_name, env, error::Error, fs, io::Cursor, path::Path};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hex::encode;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    offline::{
        KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, KagemushaValidationError,
        OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1,
        OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1, OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1,
        OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1, OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1,
        OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
        OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1,
        OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1, OFFLINE_CASH_SESSION_MAX_BYTES_V1,
        OFFLINE_CASH_SESSION_TARGET_BYTES_V1, OFFLINE_CASH_TEXT_PREFIX_V1,
        OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1, OFFLINE_CASH_WIRE_VERSION_V1,
        OfflineCashAcknowledgementV1, OfflineCashIpaLineageV1, OfflineCashPairedProofV1,
        OfflineCashPaymentRequestV1, OfflineCashPaymentV1, OfflineCashPeerAdapterV1,
        OfflineCashRecursivePairBindingV1, OfflineCashTransferResultV1,
        OfflineCashTransferStatementV1, offline_cash_receiver_key_reference_v1,
        validate_offline_cash_session_v1,
    },
};
use norito::{
    NoritoSerialize,
    core::{Compression, Header, header_flags},
    json::{self, Value},
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/offline/offline_cash_peer_transport_v1.json"
);
const REQUEST_RUST_TYPE: &str =
    "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentRequestV1";
const PAYMENT_RUST_TYPE: &str = "iroha_data_model::offline::offline_cash_v1::OfflineCashPaymentV1";
const ACKNOWLEDGEMENT_RUST_TYPE: &str =
    "iroha_data_model::offline::offline_cash_v1::OfflineCashAcknowledgementV1";
const NATIVE_TEXT_SCHEMA_VERSION: u16 = 0x0100;
// Canonical non-identity Pasta generator encodings from the exact curve
// implementation used by Core's Offline Cash verifier. These fixtures prove
// wire interoperability only; they are not terminally decided proof lineage.
const EQ_FOLDED_GENERATOR_V1: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x21, 0xeb, 0x46, 0x8c, 0xdd, 0xa8, 0x94, 0x09, 0xfc, 0x98, 0x46, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
];
const EP_FOLDED_GENERATOR_V1: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0xed, 0x30, 0x2d, 0x99, 0x1b, 0xf9, 0x4c, 0x09, 0xfc, 0x98, 0x46, 0x22,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40,
];

fn main() -> Result<(), Box<dyn Error>> {
    let check_only = env::args().any(|argument| argument == "--check");
    let fixture = build_fixture()?;
    write_fixture(FIXTURE_PATH, &fixture, check_only)
}

fn build_fixture() -> Result<Value, Box<dyn Error>> {
    let request = fixture_request()?;
    let payment = fixture_payment(&request)?;
    let acknowledgement = fixture_acknowledgement(&request, &payment)?;
    let adapter = OfflineCashPeerAdapterV1;

    request.validate()?;
    payment.validate_against(&request)?;
    acknowledgement.validate_against(&request, &payment)?;

    let request_text = adapter.encode_payment_request(&request)?;
    let payment_text = adapter.encode_payment(&request, &payment)?;
    let acknowledgement_text =
        adapter.encode_acknowledgement(&request, &payment, &acknowledgement)?;

    let request_fixture = build_message_fixture::<OfflineCashPaymentRequestV1>(
        "receiver_payment_request",
        "receive_request",
        1,
        REQUEST_RUST_TYPE,
        OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
        &request,
        &request_text,
        8,
    )?;
    let payment_fixture = build_message_fixture::<OfflineCashPaymentV1>(
        "sender_payment",
        "payment",
        2,
        PAYMENT_RUST_TYPE,
        OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
        &payment,
        &payment_text,
        8,
    )?;
    let acknowledgement_fixture = build_message_fixture::<OfflineCashAcknowledgementV1>(
        "receiver_acknowledgement_after_persist",
        "acknowledgement",
        3,
        ACKNOWLEDGEMENT_RUST_TYPE,
        OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1,
        &acknowledgement,
        &acknowledgement_text,
        0,
    )?;

    let raw_session_bytes = validate_offline_cash_session_v1(&request, &payment, &acknowledgement)?;
    let fixture_raw_session_bytes =
        request_fixture.raw_bytes + payment_fixture.raw_bytes + acknowledgement_fixture.raw_bytes;
    if raw_session_bytes != fixture_raw_session_bytes {
        return Err(format!(
            "session byte count mismatch: validator={raw_session_bytes}, fixture={fixture_raw_session_bytes}"
        )
        .into());
    }
    let text_session_bytes = request_fixture.text_bytes
        + payment_fixture.text_bytes
        + acknowledgement_fixture.text_bytes;
    if text_session_bytes > OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1 {
        return Err(format!(
            "fixture text session is {text_session_bytes} bytes, maximum is {OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1}"
        )
        .into());
    }

    let decoded_request = adapter.decode_payment_request(&request_text)?;
    let decoded_payment = adapter.decode_payment(&decoded_request, &payment_text)?;
    let decoded_acknowledgement = adapter.decode_acknowledgement(
        &decoded_request,
        &decoded_payment,
        &acknowledgement_text,
    )?;
    if decoded_request != request
        || decoded_payment != payment
        || decoded_acknowledgement != acknowledgement
    {
        return Err("typed peer-adapter roundtrip changed fixture values".into());
    }

    let request_digest = request.canonical_digest()?;
    let payment_digest = payment.canonical_digest_against(&request)?;
    let request_text_max = maximum_text_bytes(OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1);
    let payment_text_max = maximum_text_bytes(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1);
    let acknowledgement_text_max = maximum_text_bytes(OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1);
    if (request_text_max, payment_text_max, acknowledgement_text_max) != (1_029, 10_587, 347) {
        return Err("Offline Cash V1 per-message text limits changed unexpectedly".into());
    }

    Ok(norito::json!({
        "schema": "iroha.offline-cash.peer-transport.v1",
        "fixture_version": 1,
        "purpose": "Cross-SDK canonical kgm2 and profile-3 transport interoperability; deterministic proof blobs exercise bounded adapter admission and are not production proving evidence.",
        "source": "Rust OfflineCashPeerAdapterV1 over canonical Norito",
        "semantic_validation": "All three messages pass the native Offline Cash V1 typed adapter and complete-session binding checks.",
        "native_bridge_abi": 22,
        "transport": {
            "iroha_peer_wire_profile": 3,
            "native_text_schema_version": (u64::from(NATIVE_TEXT_SCHEMA_VERSION)),
            "text_prefix": (OFFLINE_CASH_TEXT_PREFIX_V1),
            "text_encoding": "UTF-8 kgm2 prefix plus unpadded canonical Base64URL",
            "base64url_padding": false,
            "ordered_stages": [
                "receiver_payment_request",
                "sender_payment",
                "receiver_acknowledgement_after_persist"
            ],
        },
        "limits": {
            "payment_request_raw_max_bytes": (OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1 as u64),
            "payment_raw_max_bytes": (OFFLINE_CASH_PAYMENT_MAX_BYTES_V1 as u64),
            "acknowledgement_raw_max_bytes": (OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1 as u64),
            "payment_request_text_max_bytes": (request_text_max as u64),
            "payment_text_max_bytes": (payment_text_max as u64),
            "acknowledgement_text_max_bytes": (acknowledgement_text_max as u64),
            "raw_session_target_bytes": (OFFLINE_CASH_SESSION_TARGET_BYTES_V1 as u64),
            "raw_session_max_bytes": (OFFLINE_CASH_SESSION_MAX_BYTES_V1 as u64),
            "text_session_max_bytes": (OFFLINE_CASH_TEXT_SESSION_MAX_BYTES_V1 as u64),
            "paired_proof_target_bytes": (OFFLINE_CASH_PAIRED_PROOF_TARGET_BYTES_V1 as u64),
            "paired_proof_max_bytes": (OFFLINE_CASH_PAIRED_PROOF_MAX_BYTES_V1 as u64),
            "parity_proof_max_bytes": (OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1 as u64),
            "recursive_pair_binding_bytes": (OFFLINE_CASH_RECURSIVE_PAIR_BINDING_ENCODED_BYTES_V1 as u64),
            "recursive_pair_binding_public_bytes": (OFFLINE_CASH_RECURSIVE_PAIR_BINDING_PUBLIC_BYTES_V1 as u64),
            "carried_lineage_bytes_per_parity": (OFFLINE_CASH_IPA_LINEAGE_ENCODED_BYTES_V1 as u64),
            "carried_lineage_crypto_bytes_per_parity": (OFFLINE_CASH_IPA_LINEAGE_CRYPTO_BYTES_V1 as u64),
            "carried_lineage_instance_cells_per_parity": (OFFLINE_CASH_IPA_LINEAGE_INSTANCE_CELLS_V1 as u64),
            "encrypted_credit_max_bytes": (OFFLINE_CASH_ENCRYPTED_CREDIT_MAX_BYTES_V1 as u64),
        },
        "session": {
            "wire_version": (u64::from(OFFLINE_CASH_WIRE_VERSION_V1)),
            "raw_norito_bytes": (raw_session_bytes as u64),
            "kgm2_text_bytes": (text_session_bytes as u64),
            "release_id_hex": (encode(request.release_id)),
            "request_digest_hex": (encode(request_digest)),
            "payment_digest_hex": (encode(payment_digest)),
            "receiver_balance_commitment_after_persist_hex": (
                encode(acknowledgement.receiver_balance_commitment)
            ),
        },
        "messages": {
            "payment_request": (request_fixture.value),
            "payment": (payment_fixture.value),
            "acknowledgement": (acknowledgement_fixture.value),
        },
    }))
}

struct MessageFixture {
    value: Value,
    raw_bytes: usize,
    text_bytes: usize,
}

#[allow(clippy::too_many_arguments)]
fn build_message_fixture<T: NoritoSerialize>(
    stage: &'static str,
    peer_payload_kind: &'static str,
    kind_id: u16,
    expected_rust_type: &'static str,
    maximum_raw_bytes: usize,
    value: &T,
    peer_text: &str,
    expected_padding_bytes: usize,
) -> Result<MessageFixture, Box<dyn Error>> {
    let rust_type = type_name::<T>();
    if rust_type != expected_rust_type {
        return Err(format!(
            "fixture type name changed: expected {expected_rust_type}, found {rust_type}"
        )
        .into());
    }

    let raw = norito::encode_canonical(value)?;
    if raw.len() > maximum_raw_bytes {
        return Err(format!(
            "{stage} is {} raw bytes, maximum is {maximum_raw_bytes}",
            raw.len()
        )
        .into());
    }
    let canonical_text = format!(
        "{OFFLINE_CASH_TEXT_PREFIX_V1}{}",
        URL_SAFE_NO_PAD.encode(&raw)
    );
    if canonical_text != peer_text {
        return Err(format!("{stage} adapter text is not the canonical raw archive").into());
    }
    let maximum_text_bytes = maximum_text_bytes(maximum_raw_bytes);
    if peer_text.len() > maximum_text_bytes {
        return Err(format!(
            "{stage} is {} text bytes, maximum is {maximum_text_bytes}",
            peer_text.len()
        )
        .into());
    }

    let header = Header::read(Cursor::new(&raw))?;
    let schema_hash = T::schema_hash();
    if header.schema != schema_hash {
        return Err(format!("{stage} Norito schema hash does not match its Rust type").into());
    }
    if header.compression != Compression::None {
        return Err(format!("{stage} canonical archive is compressed").into());
    }
    if header.flags != header_flags::COMPACT_LEN {
        return Err(format!(
            "{stage} canonical flags are {:#04x}, expected compact-length only ({:#04x})",
            header.flags,
            header_flags::COMPACT_LEN
        )
        .into());
    }
    let payload_bytes = usize::try_from(header.length)?;
    let padding_bytes = raw
        .len()
        .checked_sub(Header::SIZE)
        .and_then(|framed_payload| framed_payload.checked_sub(payload_bytes))
        .ok_or_else(|| format!("{stage} Norito frame length is inconsistent"))?;
    if padding_bytes != expected_padding_bytes {
        return Err(format!(
            "{stage} canonical padding changed: expected {expected_padding_bytes}, found {padding_bytes}"
        )
        .into());
    }
    if raw[Header::SIZE..Header::SIZE + padding_bytes]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(format!("{stage} Norito alignment padding is non-zero").into());
    }
    let payload = &raw[Header::SIZE + padding_bytes..];
    if payload.len() != payload_bytes {
        return Err(format!("{stage} Norito payload length is inconsistent").into());
    }

    let value = norito::json!({
        "stage": (stage),
        "semantic_valid": true,
        "iroha_peer_wire_profile": 3,
        "native_text_schema_version": (u64::from(NATIVE_TEXT_SCHEMA_VERSION)),
        "peer_payload_kind": (peer_payload_kind),
        "payload_kind_id": (u64::from(kind_id)),
        "rust_type": (rust_type),
        "raw_norito_bytes": (raw.len() as u64),
        "raw_norito_sha256_hex": (sha256_hex(&raw)),
        "raw_norito_hex": (encode(&raw)),
        "kgm2_text_bytes": (peer_text.len() as u64),
        "kgm2_text_sha256_hex": (sha256_hex(peer_text.as_bytes())),
        "kgm2_text": (peer_text),
        "maximum_raw_norito_bytes": (maximum_raw_bytes as u64),
        "maximum_kgm2_text_bytes": (maximum_text_bytes as u64),
        "norito_header": {
            "header_bytes": (Header::SIZE as u64),
            "header_hex": (encode(&raw[..Header::SIZE])),
            "magic_ascii": "NRT0",
            "major_version": (u64::from(header.major)),
            "minor_version": (u64::from(header.minor)),
            "rust_type_name_schema": (rust_type),
            "schema_hash_hex": (encode(schema_hash)),
            "compression": "none",
            "compression_id": (header.compression as u8 as u64),
            "payload_bytes": (payload_bytes as u64),
            "payload_sha256_hex": (sha256_hex(payload)),
            "checksum_crc64_xz_hex": (format!("{:016x}", header.checksum)),
            "flags": (u64::from(header.flags)),
            "flags_hex": (format!("{:02x}", header.flags)),
            "enabled_flags": ["compact_len"],
            "padding_bytes": (padding_bytes as u64),
            "padding_must_be_zero": true,
        },
    });

    Ok(MessageFixture {
        value,
        raw_bytes: raw.len(),
        text_bytes: peer_text.len(),
    })
}

fn fixture_asset() -> Result<AssetDefinitionId, Box<dyn Error>> {
    Ok(AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal")?,
        "xor".parse()?,
    ))
}

fn fixture_account() -> AccountId {
    AccountId::new(
        KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    )
}

fn fixture_network_id() -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"offline-cash-v1",
    )))
}

fn fixture_signing_key() -> Result<SigningKey, Box<dyn Error>> {
    Ok(SigningKey::from_bytes((&[7_u8; 32]).into())?)
}

fn fixture_encryption_public_key() -> [u8; 32] {
    [
        0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e, 0xf7,
        0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e, 0xaa, 0x9b,
        0x4e, 0x6a,
    ]
}

fn sign(key: &SigningKey, bytes: &[u8]) -> Result<KagemushaDeviceSignatureV2, Box<dyn Error>> {
    let signature: Signature = key.sign(bytes);
    let signature = signature.normalize_s().unwrap_or(signature);
    Ok(KagemushaDeviceSignatureV2::from_raw_bytes(
        signature.to_bytes().as_ref(),
    )?)
}

fn fixture_request() -> Result<OfflineCashPaymentRequestV1, Box<dyn Error>> {
    let signing_key = fixture_signing_key()?;
    let encoded = signing_key.verifying_key().to_encoded_point(false);
    let public_key = KagemushaDevicePublicKeyV2::from_sec1_bytes(encoded.as_bytes())?;
    let encryption_public_key = fixture_encryption_public_key();
    let mut request = OfflineCashPaymentRequestV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: [1; 32],
        network_id: fixture_network_id(),
        asset: fixture_asset()?,
        scale: 4,
        amount: 12_345,
        recipient: fixture_account(),
        receiver_balance_commitment: [2; 32],
        recipient_key_reference: offline_cash_receiver_key_reference_v1(
            &public_key,
            encryption_public_key,
        ),
        recipient_encryption_public_key: encryption_public_key,
        receiver_public_key: public_key,
        request_id: [3; 32],
        issued_at_ms: 1_000,
        expires_at_ms: 61_000,
        hardware_policy_id: [4; 32],
        signature: sign(&signing_key, b"placeholder")?,
    };
    request.signature = sign(&signing_key, &request.canonical_signing_bytes()?)?;
    Ok(request)
}

fn fixture_payment(
    request: &OfflineCashPaymentRequestV1,
) -> Result<OfflineCashPaymentV1, Box<dyn Error>> {
    let request_digest = request.canonical_digest()?;
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
    .seal_transition()?;
    let transfer = OfflineCashTransferResultV1::from_statement_against(&statement, request)?;
    Ok(OfflineCashPaymentV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        transfer,
        proof: OfflineCashPairedProofV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            eq_proof: vec![0xA1; 128],
            ep_proof: vec![0xB2; 128],
            eq_carried_lineage: fixture_lineage(1, EQ_FOLDED_GENERATOR_V1)?,
            ep_carried_lineage: fixture_lineage(17, EP_FOLDED_GENERATOR_V1)?,
            recursive_pair_binding: OfflineCashRecursivePairBindingV1::new_state(
                [0xC3; 32],
                [0xD4; 32],
                &OfflineCashRecursivePairBindingV1::new_guard_bundle([0xA1; 32], [0xB2; 32])?,
            )?,
        },
        encrypted_credit: vec![0xE5; 128],
    })
}

fn fixture_lineage(
    first_challenge: u64,
    folded_generator: [u8; 32],
) -> Result<OfflineCashIpaLineageV1, KagemushaValidationError> {
    OfflineCashIpaLineageV1::new(
        std::array::from_fn(|index| {
            let mut encoded = [0_u8; 32];
            let challenge = first_challenge
                .checked_add(u64::try_from(index).expect("lineage index fits u64"))
                .expect("fixture challenge does not overflow");
            encoded[..8].copy_from_slice(&challenge.to_le_bytes());
            encoded
        }),
        folded_generator,
    )
}

fn fixture_acknowledgement(
    request: &OfflineCashPaymentRequestV1,
    payment: &OfflineCashPaymentV1,
) -> Result<OfflineCashAcknowledgementV1, Box<dyn Error>> {
    let signing_key = fixture_signing_key()?;
    let mut acknowledgement = OfflineCashAcknowledgementV1 {
        version: OFFLINE_CASH_WIRE_VERSION_V1,
        release_id: request.release_id,
        request_digest: request.canonical_digest()?,
        payment_digest: payment.canonical_digest_against(request)?,
        receiver_balance_commitment: [11; 32],
        acknowledged_at_ms: request.issued_at_ms + 1,
        signature: sign(&signing_key, b"placeholder")?,
    };
    acknowledgement.signature = sign(&signing_key, &acknowledgement.canonical_signing_bytes()?)?;
    Ok(acknowledgement)
}

fn maximum_text_bytes(raw_bytes: usize) -> usize {
    let base64url_bytes = raw_bytes / 3 * 4
        + match raw_bytes % 3 {
            0 => 0,
            1 => 2,
            _ => 3,
        };
    OFFLINE_CASH_TEXT_PREFIX_V1.len() + base64url_bytes
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode(Sha256::digest(bytes))
}

fn write_fixture(path: &str, value: &Value, check_only: bool) -> Result<(), Box<dyn Error>> {
    let rendered = json::to_string_pretty(value)?;
    if check_only {
        let existing = fs::read_to_string(path)?;
        if existing.trim() != rendered.trim() {
            return Err(format!(
                "fixture {path} is stale; run cargo iroha-fast -- run --locked --offline -p iroha_data_model --features dev-tools,test-fixtures --bin offline_cash_peer_fixtures"
            )
            .into());
        }
        return Ok(());
    }
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{rendered}\n"))?;
    Ok(())
}
