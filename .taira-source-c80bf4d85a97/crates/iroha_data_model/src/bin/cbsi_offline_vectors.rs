//! Regenerate the canonical CBSI Offline Cash interop fixture shared by the wallets.
//!
//! The app-packaged JSON shape is retained, but every canonical archive is built
//! from the current Kagemusha V2/V4 types. No deleted `OfflineNote` compatibility
//! model participates in this producer. Every account literal is encoded for
//! Taira discriminant 369 and every balance is the exact `sbd#cbsi` definition.
//!
//! Run:
//!
//! ```text
//! cargo run -p iroha_data_model --features test-fixtures,transparent_api \
//!   --bin cbsi_offline_vectors
//! ```
//!
//! Use `--check` to reject drift or `--output <path>` to write a synchronized
//! Android/iOS copy.

use std::{env, error::Error, fs, io, path::Path};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use hex::encode;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    ChainId,
    account::{AccountAddress, AccountId},
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId},
    offline::{
        KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2,
        KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4, KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2, KagemushaPastaCycleParityV1,
        KagemushaPastaCycleProofEnvelopeV4, KagemushaReceiverAcknowledgementPayloadV2,
        KagemushaReceiverAcknowledgementV2, KagemushaRecipientPaymentRequestSigningPayloadV2,
        KagemushaRecipientPaymentRequestV2, KagemushaRecursiveSpendArtifactBindingV4,
        KagemushaRecursiveSpendBranchClaimV2, KagemushaRecursiveSpendBranchV2,
        KagemushaRecursiveSpendBundleV4, KagemushaRecursiveSpendOperationVectorV4,
        KagemushaRecursiveSpendPeerSplitTransitionV4, KagemushaRecursiveSpendProofV4,
        KagemushaRecursiveSpendPublicStatementV4, KagemushaRecursiveSpendStateBoundaryV2,
        KagemushaRecursiveSpendTopUpAnchorRefV2, KagemushaRecursiveSpendTransitionV4,
        KagemushaScaledAmountV2, KagemushaSpendableNoteDescriptorV2,
        kagemusha_receiver_key_reference_v2, kagemusha_recursive_spend_lineage_root_v2,
        kagemusha_recursive_spend_verifier_key_id_v4,
    },
    proof::ProofBox,
    qr_stream::{QrPayloadKind, QrStreamEncoder, QrStreamFrameKind, QrStreamOptions},
};
use norito::{
    json::{self, Value},
    to_bytes,
};
use p256::ecdsa::{
    Signature as P256Signature, SigningKey as P256SigningKey, signature::Signer as _,
};
use sha2::{Digest, Sha256};

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/offline/cbsi_interop_contract.json"
);
const CHAIN_ID: &str = "taira-cbsi-offline-fixture";
const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
const CBSI_SBD_ASSET_ALIAS: &str = "sbd#cbsi";
const CBSI_SBD_ASSET_DEFINITION_ID: &str = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv";
const INVOICE_ID: &str = "invoice-fixture-1";
const SENDER_KEY_ID: &str = "sender-key-offline-1";
const RECIPIENT_KEY_ID: &str = "recipient-key-offline-1";
const SENDER_DEVICE_ID: &str = "sender-device-offline-1";
const RECIPIENT_DEVICE_ID: &str = "recipient-device-offline-1";
const AMOUNT: &str = "5";
const CHANGE_AMOUNT: &str = "47";
const ISSUE_AMOUNT: &str = "52";
const ASSET_SCALE: u32 = 2;
const AMOUNT_ATOMIC_UNITS: u128 = 500;
const CHANGE_ATOMIC_UNITS: u128 = 4_700;
const ISSUE_ATOMIC_UNITS: u128 = 5_200;
const GENERATED_AT_MS: u64 = 1_706_000_000_000;
const CREATED_AT_MS: u64 = 1_706_000_000_123;
const ACCEPTED_AT_MS: u64 = 1_706_000_000_333;
const DISPLAY_TTL_MS: u64 = 60_000;
const PAYMENT_TOKEN_ENVELOPE_SCHEMA: &str =
    "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV4";
const OFFLINE_BEARER_CASH_RECEIVE_PREFIX: &str = "wallet-offline-bearer-cash-receive:";
const OFFLINE_BEARER_CASH_PAYMENT_PREFIX: &str = "wallet-offline-bearer-cash-payment:";
const OFFLINE_BEARER_CASH_ACK_PREFIX: &str = "wallet-offline-bearer-cash-ack:";

#[cfg_attr(not(test), allow(dead_code))]
struct FixtureParts {
    fixture: Value,
    sender_literal: String,
    recipient_literal: String,
    source_request: KagemushaRecipientPaymentRequestV2,
    recipient_request: KagemushaRecipientPaymentRequestV2,
    payment_bundle: KagemushaRecursiveSpendBundleV4,
    acknowledgement: KagemushaReceiverAcknowledgementV2,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut check_only = false;
    let mut output = FIXTURE_PATH.to_owned();
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--check" => check_only = true,
            "--output" => {
                output = arguments.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--output requires one path")
                })?;
            }
            unsupported => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported argument: {unsupported}"),
                )
                .into());
            }
        }
    }
    let parts = build_fixture()?;
    write_fixture(&output, &parts.fixture, check_only)
}

#[allow(clippy::too_many_lines)]
fn build_fixture() -> Result<FixtureParts, Box<dyn Error>> {
    let offline_fi_key = fixed_ed25519_keypair(0x11)?;
    let sender_account_key = fixed_ed25519_keypair(0x21)?;
    let recipient_account_key = fixed_ed25519_keypair(0x22)?;
    let sender = AccountId::new(sender_account_key.public_key().clone());
    let recipient = AccountId::new(recipient_account_key.public_key().clone());
    let sender_literal = taira_account_literal(&sender)?;
    let recipient_literal = taira_account_literal(&recipient)?;

    let asset_definition = AssetDefinitionId::parse_address_literal(CBSI_SBD_ASSET_DEFINITION_ID)?;
    let asset_definition_literal = asset_definition.canonical_address();
    if asset_definition_literal != CBSI_SBD_ASSET_DEFINITION_ID {
        return Err(format!(
            "{CBSI_SBD_ASSET_ALIAS} derived {asset_definition_literal}, expected {CBSI_SBD_ASSET_DEFINITION_ID}"
        )
        .into());
    }
    let sender_asset = AssetId::new(asset_definition.clone(), sender.clone());
    let recipient_asset = AssetId::new(asset_definition.clone(), recipient.clone());
    let sender_asset_literal = taira_asset_literal(&sender_asset)?;
    let recipient_asset_literal = taira_asset_literal(&recipient_asset)?;
    let chain_id: ChainId = CHAIN_ID.parse()?;

    let sender_device_key = fixed_p256_signing_key(0x31)?;
    let recipient_device_key = fixed_p256_signing_key(0x32)?;
    let source_note = KagemushaSpendableNoteDescriptorV2 {
        chain_id: chain_id.clone(),
        asset: asset_definition.clone(),
        note_commitment: [0x41; 32],
        spend_nullifier: [0x42; 32],
        amount: KagemushaScaledAmountV2::new(ISSUE_ATOMIC_UNITS, ASSET_SCALE)?,
    };
    let recipient_note = KagemushaSpendableNoteDescriptorV2 {
        chain_id: chain_id.clone(),
        asset: asset_definition.clone(),
        note_commitment: [0x52; 32],
        spend_nullifier: [0x53; 32],
        amount: KagemushaScaledAmountV2::new(AMOUNT_ATOMIC_UNITS, ASSET_SCALE)?,
    };
    let change_note = KagemushaSpendableNoteDescriptorV2 {
        chain_id: chain_id.clone(),
        asset: asset_definition.clone(),
        note_commitment: [0x62; 32],
        spend_nullifier: [0x63; 32],
        amount: KagemushaScaledAmountV2::new(CHANGE_ATOMIC_UNITS, ASSET_SCALE)?,
    };
    let source_request = signed_payment_request(
        &sender_device_key,
        chain_id.clone(),
        asset_definition.clone(),
        sender.clone(),
        SENDER_DEVICE_ID,
        [0x31; 32],
        GENERATED_AT_MS - 1_000,
        GENERATED_AT_MS + DISPLAY_TTL_MS,
        source_note.clone(),
        vec![0x34],
    )?;
    let recipient_request = signed_payment_request(
        &recipient_device_key,
        chain_id,
        asset_definition.clone(),
        recipient.clone(),
        RECIPIENT_DEVICE_ID,
        [0x51; 32],
        GENERATED_AT_MS,
        GENERATED_AT_MS + DISPLAY_TTL_MS,
        recipient_note.clone(),
        vec![0x54],
    )?;
    recipient_request.validate_at(GENERATED_AT_MS)?;

    let payment_bundle = payment_bundle(&recipient_request)?;
    let acknowledgement = acknowledgement(
        &recipient_device_key,
        &recipient_request,
        &payment_bundle,
        ACCEPTED_AT_MS,
    )?;
    acknowledgement.validate_for_payment_v4(&recipient_request, &payment_bundle)?;

    let source_request_bytes = to_bytes(&source_request)?;
    let recipient_signing_bytes = recipient_request.signing_payload().signing_bytes()?;
    let sender_signing_bytes = source_request.signing_payload().signing_bytes()?;
    let payment_bundle_bytes = to_bytes(&payment_bundle)?;
    let acknowledgement_bytes =
        acknowledgement.canonical_archive_for_payment_v4(&recipient_request, &payment_bundle)?;
    let payment_bundle_digest = payment_bundle.digest()?;
    let statement_digest = payment_bundle.statement.digest()?;
    let acknowledgement_digest = acknowledgement.digest()?;
    let source_note_bytes = to_bytes(&source_note)?;
    let recipient_note_bytes = to_bytes(&recipient_note)?;
    let change_note_bytes = to_bytes(&change_note)?;
    let source_claim_hash = sha256_hex(&source_note_bytes);
    let recipient_claim_hash = sha256_hex(&recipient_note_bytes);
    let change_claim_hash = sha256_hex(&change_note_bytes);
    let sender_certificate_payload_hash = sha256_hex(&sender_signing_bytes);
    let recipient_certificate_payload_hash = sha256_hex(&recipient_signing_bytes);

    let sender_certificate = certificate_json(
        &sender_literal,
        SENDER_KEY_ID,
        SENDER_DEVICE_ID,
        &sender_device_key,
        &source_request.signature,
        &sender_signing_bytes,
    );
    let recipient_certificate = certificate_json(
        &recipient_literal,
        RECIPIENT_KEY_ID,
        RECIPIENT_DEVICE_ID,
        &recipient_device_key,
        &recipient_request.signature,
        &recipient_signing_bytes,
    );
    let input_claim = object(vec![
        ("amount", Value::from(ISSUE_AMOUNT)),
        ("asset_id", Value::from(sender_asset_literal.clone())),
        ("claim_hash", Value::from(source_claim_hash.clone())),
        ("domain", Value::from("iroha:kagemusha:v4:input-note")),
        (
            "key_certificate_payload_hash",
            Value::from(sender_certificate_payload_hash.clone()),
        ),
        (
            "note_commitment",
            Value::from(encode(source_note.note_commitment)),
        ),
    ]);
    let recipient_output_claim = object(vec![
        ("account_id", Value::from(recipient_literal.clone())),
        ("amount", Value::from(AMOUNT)),
        (
            "asset_definition_id",
            Value::from(asset_definition_literal.clone()),
        ),
        ("key_certificate", recipient_certificate.clone()),
        (
            "note_commitment",
            Value::from(encode(recipient_note.note_commitment)),
        ),
    ]);
    let change_output_claim = object(vec![
        ("account_id", Value::from(sender_literal.clone())),
        ("amount", Value::from(CHANGE_AMOUNT)),
        (
            "asset_definition_id",
            Value::from(asset_definition_literal.clone()),
        ),
        ("key_certificate", sender_certificate.clone()),
        (
            "note_commitment",
            Value::from(encode(change_note.note_commitment)),
        ),
    ]);

    let payment_token = object(vec![
        ("amount", Value::from(AMOUNT)),
        (
            "asset_definition_id",
            Value::from(asset_definition_literal.clone()),
        ),
        ("change_amount", Value::from(CHANGE_AMOUNT)),
        ("created_at_ms", Value::from(CREATED_AT_MS)),
        ("input_claims", Value::Array(vec![input_claim])),
        (
            "input_nullifiers",
            string_array(&[encode(source_note.spend_nullifier)]),
        ),
        ("invoice_id", Value::from(INVOICE_ID)),
        (
            "one_use_assertion",
            object(vec![
                (
                    "assertion_base64",
                    Value::from(BASE64_STANDARD.encode(recipient_request.signature.as_raw_bytes())),
                ),
                (
                    "challenge_hash_hex",
                    Value::from(encode(recipient_request.digest()?)),
                ),
                ("counter", Value::from(1_u64)),
                ("key_id", Value::from(RECIPIENT_KEY_ID)),
                ("platform", Value::from("ios-appattest")),
            ]),
        ),
        (
            "output_claims",
            Value::Array(vec![recipient_output_claim, change_output_claim]),
        ),
        (
            "output_commitments",
            string_array(&[
                encode(recipient_note.note_commitment),
                encode(change_note.note_commitment),
            ]),
        ),
        (
            "recipient_account_id",
            Value::from(recipient_literal.clone()),
        ),
        ("recipient_key_certificate", recipient_certificate.clone()),
        (
            "recursive_proof",
            object(vec![
                (
                    "proof_backend",
                    Value::from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4),
                ),
                (
                    "proof_bytes_base64",
                    Value::from(
                        BASE64_STANDARD
                            .encode(&payment_bundle.recursive_proof.proof_envelope.proof.bytes),
                    ),
                ),
                (
                    "public_inputs_hash_hex",
                    Value::from(encode(statement_digest)),
                ),
                (
                    "verifier_key_backend",
                    Value::from(KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4),
                ),
                (
                    "verifier_key_id",
                    Value::from(format!(
                        "{}/{}",
                        payment_bundle.statement.verifier_key_id.backend.as_str(),
                        payment_bundle.statement.verifier_key_id.name,
                    )),
                ),
            ]),
        ),
        ("sender_account_id", Value::from(sender_literal.clone())),
        ("sender_key_certificate", sender_certificate.clone()),
        (
            "source_note_commitment",
            Value::from(encode(source_note.note_commitment)),
        ),
        ("token_id", Value::from(encode(payment_bundle_digest))),
        ("type", Value::from("offline_payment_token")),
    ]);

    let receive_request = object(vec![
        ("account_id", Value::from(recipient_literal.clone())),
        ("amount", Value::from(AMOUNT)),
        (
            "asset_definition_id",
            Value::from(asset_definition_literal.clone()),
        ),
        ("display_ttl_ms", Value::from(DISPLAY_TTL_MS)),
        ("generated_at_ms", Value::from(GENERATED_AT_MS)),
        ("invoice_id", Value::from(INVOICE_ID)),
        ("recipient_key_certificate", recipient_certificate),
        ("type", Value::from("offline_receive_request")),
    ]);
    let receipt_ack = object(vec![
        ("accepted_at_ms", Value::from(ACCEPTED_AT_MS)),
        (
            "recipient_account_id",
            Value::from(recipient_literal.clone()),
        ),
        ("token_id", Value::from(encode(payment_bundle_digest))),
        ("type", Value::from("offline_receipt_ack")),
    ]);

    let fixture = object(vec![
        (
            "bad_variants",
            Value::Array(vec![
                object(vec![
                    (
                        "expected_error",
                        Value::from("public_statement_digest_mismatch"),
                    ),
                    ("name", Value::from("forged_output_claim_amount")),
                    (
                        "patch",
                        Value::from("payment_token.output_claims[0].amount=6"),
                    ),
                ]),
                object(vec![
                    ("expected_error", Value::from("recipient_binding_mismatch")),
                    ("name", Value::from("wrong_recipient_output_claim")),
                    (
                        "patch",
                        Value::from(
                            "payment_token.output_claims[0].account_id=payment_token.sender_account_id",
                        ),
                    ),
                ]),
                object(vec![
                    (
                        "expected_error",
                        Value::from("receiver_acknowledgement_replay"),
                    ),
                    ("name", Value::from("reused_one_use_counter")),
                    (
                        "patch",
                        Value::from("payment_token.one_use_assertion.counter=2"),
                    ),
                ]),
            ]),
        ),
        (
            "capabilities",
            str_array(&[
                "offline_payments",
                "offline_kagemusha_v4_release_distribution_available",
                "offline_kagemusha_v4_required_native_bridge_abi_version",
                "offline_kagemusha_v4_release_manifest_sha256s",
                "offline_telemetry",
            ]),
        ),
        (
            "cbsi_binding",
            object(vec![
                ("network", Value::from("taira")),
                (
                    "account_chain_discriminant",
                    Value::from(u64::from(TAIRA_CHAIN_DISCRIMINANT)),
                ),
                ("asset_alias", Value::from(CBSI_SBD_ASSET_ALIAS)),
                (
                    "asset_definition_id",
                    Value::from(asset_definition_literal.clone()),
                ),
            ]),
        ),
        (
            "chain_vectors",
            object(vec![
                (
                    "audit",
                    object(vec![
                        (
                            "input_claim_hashes",
                            string_array(std::slice::from_ref(&source_claim_hash)),
                        ),
                        (
                            "input_nullifiers",
                            string_array(&[encode(source_note.spend_nullifier)]),
                        ),
                        (
                            "norito_base64",
                            Value::from(BASE64_STANDARD.encode(&payment_bundle_bytes)),
                        ),
                        (
                            "output_claim_hashes",
                            string_array(&[
                                recipient_claim_hash.clone(),
                                change_claim_hash.clone(),
                            ]),
                        ),
                        (
                            "output_commitments",
                            string_array(&[
                                encode(recipient_note.note_commitment),
                                encode(change_note.note_commitment),
                            ]),
                        ),
                        ("public_inputs_hash", Value::from(encode(statement_digest))),
                        ("token_id", Value::from(encode(payment_bundle_digest))),
                    ]),
                ),
                (
                    "certificates",
                    object(vec![
                        (
                            "recipient_payload_base64",
                            Value::from(BASE64_STANDARD.encode(&recipient_signing_bytes)),
                        ),
                        (
                            "recipient_payload_hash",
                            Value::from(recipient_certificate_payload_hash.clone()),
                        ),
                        (
                            "sender_payload_base64",
                            Value::from(BASE64_STANDARD.encode(&sender_signing_bytes)),
                        ),
                        (
                            "sender_payload_hash",
                            Value::from(sender_certificate_payload_hash.clone()),
                        ),
                    ]),
                ),
                (
                    "derivation",
                    object(vec![
                        ("chain_id", Value::from(CHAIN_ID)),
                        ("change_note_secret_hex", Value::from(encode([0x64; 32]))),
                        (
                            "change_output_commitment",
                            Value::from(encode(change_note.note_commitment)),
                        ),
                        (
                            "change_output_commitment_preimage_hex",
                            Value::from(encode(&change_note_bytes)),
                        ),
                        (
                            "input_nullifier",
                            Value::from(encode(source_note.spend_nullifier)),
                        ),
                        (
                            "input_nullifier_domain",
                            Value::from("iroha:kagemusha:v4:spend-nullifier"),
                        ),
                        (
                            "input_nullifier_preimage_hex",
                            Value::from(encode(&source_note_bytes)),
                        ),
                        ("issuer_load_lineage_id", Value::from(encode([0x56; 32]))),
                        ("issuer_load_local_revision", Value::from(1_u64)),
                        ("issuer_load_operation_id", Value::from(encode([0x55; 32]))),
                        (
                            "note_commitment_domain",
                            Value::from("iroha:kagemusha:v4:note-commitment"),
                        ),
                        ("payment_request_id", Value::from(INVOICE_ID)),
                        (
                            "payment_token_id",
                            Value::from(encode(payment_bundle_digest)),
                        ),
                        (
                            "payment_token_id_domain",
                            Value::from("iroha:kagemusha:v4:bundle-digest"),
                        ),
                        (
                            "payment_token_id_preimage_hex",
                            Value::from(encode(&payment_bundle_bytes)),
                        ),
                        (
                            "recipient_key_certificate_payload_hash",
                            Value::from(recipient_certificate_payload_hash),
                        ),
                        ("recipient_note_secret_hex", Value::from(encode([0x54; 32]))),
                        (
                            "recipient_output_commitment",
                            Value::from(encode(recipient_note.note_commitment)),
                        ),
                        (
                            "recipient_output_commitment_preimage_hex",
                            Value::from(encode(&recipient_note_bytes)),
                        ),
                        (
                            "redeem_nullifier",
                            Value::from(encode(recipient_note.spend_nullifier)),
                        ),
                        (
                            "sender_key_certificate_payload_hash",
                            Value::from(sender_certificate_payload_hash),
                        ),
                        (
                            "source_note_commitment",
                            Value::from(encode(source_note.note_commitment)),
                        ),
                        (
                            "source_note_commitment_preimage_hex",
                            Value::from(encode(&source_note_bytes)),
                        ),
                        ("source_note_secret_hex", Value::from(encode([0x44; 32]))),
                        ("token_nonce_hex", Value::from(encode([0x51; 32]))),
                    ]),
                ),
                (
                    "issue",
                    object(vec![
                        ("amount", Value::from(ISSUE_AMOUNT)),
                        ("asset_id", Value::from(sender_asset_literal)),
                        ("claim_hash", Value::from(source_claim_hash)),
                        (
                            "norito_base64",
                            Value::from(BASE64_STANDARD.encode(&source_request_bytes)),
                        ),
                        (
                            "note_commitment",
                            Value::from(encode(source_note.note_commitment)),
                        ),
                    ]),
                ),
                (
                    "redeem",
                    object(vec![
                        ("amount", Value::from(AMOUNT)),
                        ("asset_id", Value::from(recipient_asset_literal)),
                        ("claim_hash", Value::from(recipient_claim_hash)),
                        (
                            "input_nullifiers",
                            string_array(&[encode(recipient_note.spend_nullifier)]),
                        ),
                        (
                            "norito_base64",
                            Value::from(BASE64_STANDARD.encode(&acknowledgement_bytes)),
                        ),
                        (
                            "public_inputs_hash",
                            Value::from(encode(acknowledgement_digest)),
                        ),
                        (
                            "source_note_commitment",
                            Value::from(encode(recipient_note.note_commitment)),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "fountain_qr",
            fountain_qr_fixture(&payment_bundle_bytes, 360, 3)?,
        ),
        (
            "generator",
            Value::from(
                "cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin cbsi_offline_vectors",
            ),
        ),
        (
            "offline_fi_public_key_base64",
            Value::from(public_key_base64(&offline_fi_key)?),
        ),
        ("payment_token", payment_token),
        (
            "prefixes",
            object(vec![
                ("fountain_qr", Value::from("iroha:qr:")),
                (
                    "payment_token",
                    Value::from(OFFLINE_BEARER_CASH_PAYMENT_PREFIX),
                ),
                ("receipt_ack", Value::from(OFFLINE_BEARER_CASH_ACK_PREFIX)),
                (
                    "receive_request",
                    Value::from(OFFLINE_BEARER_CASH_RECEIVE_PREFIX),
                ),
            ]),
        ),
        ("receipt_ack", receipt_ack),
        ("receive_request", receive_request),
        ("sdk_interop", sdk_interop_json(&payment_bundle_bytes)?),
        ("version", Value::from(1_u64)),
    ]);

    Ok(FixtureParts {
        fixture,
        sender_literal,
        recipient_literal,
        source_request,
        recipient_request,
        payment_bundle,
        acknowledgement,
    })
}

#[allow(clippy::too_many_arguments)]
fn signed_payment_request(
    key: &P256SigningKey,
    chain_id: ChainId,
    asset: AssetDefinitionId,
    recipient: AccountId,
    receiver_device_id: &str,
    request_id: [u8; 32],
    issued_at_ms: u64,
    expires_at_ms: u64,
    recipient_output: KagemushaSpendableNoteDescriptorV2,
    sender_output_prover_material: Vec<u8>,
) -> Result<KagemushaRecipientPaymentRequestV2, Box<dyn Error>> {
    let receiver_public_key = device_public_key(key)?;
    let payload = KagemushaRecipientPaymentRequestSigningPayloadV2 {
        chain_id,
        asset,
        amount: recipient_output.amount,
        recipient,
        recipient_key_reference: kagemusha_receiver_key_reference_v2(&receiver_public_key)?,
        receiver_device_id: receiver_device_id.to_owned(),
        receiver_public_key,
        request_id,
        issued_at_ms,
        expires_at_ms,
        recipient_output,
        sender_output_prover_material,
    };
    let signature = sign(key, &payload.signing_bytes()?)?;
    Ok(KagemushaRecipientPaymentRequestV2::from_signed_payload(
        payload, signature,
    )?)
}

fn payment_bundle(
    request: &KagemushaRecipientPaymentRequestV2,
) -> Result<KagemushaRecursiveSpendBundleV4, Box<dyn Error>> {
    let anchor = KagemushaRecursiveSpendTopUpAnchorRefV2 {
        topup_operation_id: [0x55; 32],
        anchor_digest: [0x56; 32],
    };
    let lineage_root = kagemusha_recursive_spend_lineage_root_v2(anchor.anchor_digest)?;
    let artifact_binding = KagemushaRecursiveSpendArtifactBindingV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "cbsi-taira-kagemusha-v4-fixture".to_owned(),
        manifest_sha256: [0x57; 32],
    };
    let verifier_key_id = kagemusha_recursive_spend_verifier_key_id_v4(
        KagemushaPastaCycleParityV1::StepEq,
        artifact_binding.manifest_sha256,
    );
    let operation_id = [0x58; 32];
    let statement = KagemushaRecursiveSpendPublicStatementV4 {
        chain_id: request.chain_id().clone(),
        asset: request.asset().clone(),
        asset_scale: request.amount().scale,
        final_root: [0x59; 32],
        next_zero_leaf_index: 1,
        topup_anchor_refs: vec![anchor],
        proof_step_count: 2,
        peer_hop_count: 1,
        current_note: request.recipient_output().clone(),
        branch_claims: vec![KagemushaRecursiveSpendBranchClaimV2::root(lineage_root)?],
        transition: Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(
            KagemushaRecursiveSpendPeerSplitTransitionV4 {
                binding_digest: [0x5a; 32],
                branch: KagemushaRecursiveSpendBranchV2::Recipient,
                recipient_request_digest: request.digest()?,
                operation_id,
                parent_max_proof_step_count: 1,
                parent_max_peer_hop_count: 0,
            },
        )),
        artifact_binding: artifact_binding.clone(),
        verifier_key_id: verifier_key_id.clone(),
    };
    let public_statement_digest = statement.digest()?;
    let mut state_limbs = vec![0; KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LIMBS_V2];
    state_limbs[0] = KAGEMUSHA_RECURSIVE_SPEND_STATE_VECTOR_LAYOUT_VERSION_V2;
    let proof_envelope = KagemushaPastaCycleProofEnvelopeV4 {
        version: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_PROOF_ENVELOPE_VERSION_V4,
        proof_backend: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.to_owned(),
        transcript_profile: KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4.to_owned(),
        step_eq_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4.to_owned(),
        step_ep_circuit_id: KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4.to_owned(),
        artifact_generation: artifact_binding.generation,
        manifest_sha256: artifact_binding.manifest_sha256,
        step_eq_parameter_generation: "cbsi-fixture-eq-params-v5".to_owned(),
        step_ep_parameter_generation: "cbsi-fixture-ep-params-v5".to_owned(),
        step_eq_circuit_params_sha256: [0x5b; 32],
        step_ep_circuit_params_sha256: [0x5c; 32],
        step_eq_verifier_key_sha256: [0x5d; 32],
        step_ep_verifier_key_sha256: [0x5e; 32],
        state_boundary: KagemushaRecursiveSpendStateBoundaryV2::new(state_limbs)?,
        proof: ProofBox::new(
            KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4.into(),
            vec![0x5f],
        ),
    };
    let mut operation_limbs = [0; KAGEMUSHA_RECURSIVE_SPEND_OPERATION_LIMBS_V4];
    operation_limbs[0] = 1;
    let bundle = KagemushaRecursiveSpendBundleV4 {
        statement,
        operation: KagemushaRecursiveSpendOperationVectorV4 {
            limbs: operation_limbs,
        },
        recursive_proof: KagemushaRecursiveSpendProofV4 {
            verifier_key_id,
            public_statement_digest,
            proof_envelope,
        },
    };
    bundle.validate_public_binding()?;
    Ok(bundle)
}

fn acknowledgement(
    key: &P256SigningKey,
    request: &KagemushaRecipientPaymentRequestV2,
    bundle: &KagemushaRecursiveSpendBundleV4,
    accepted_at_ms: u64,
) -> Result<KagemushaReceiverAcknowledgementV2, Box<dyn Error>> {
    let Some(KagemushaRecursiveSpendTransitionV4::PeerSplit(transition)) =
        bundle.statement.transition.as_ref()
    else {
        return Err("fixture payment bundle must carry a peer split".into());
    };
    let payload = KagemushaReceiverAcknowledgementPayloadV2 {
        operation_id: transition.operation_id,
        recipient_request_digest: request.digest()?,
        payment_bundle_digest: bundle.digest()?,
        recipient_commitment: request.recipient_output().note_commitment,
        accepted_at_ms,
        receiver_device_id: request.receiver_device_id().to_owned(),
        receiver_key_reference: kagemusha_receiver_key_reference_v2(request.receiver_public_key())?,
        receiver_public_key: *request.receiver_public_key(),
    };
    let signature = sign(key, &payload.signing_bytes()?)?;
    Ok(KagemushaReceiverAcknowledgementV2 { payload, signature })
}

fn certificate_json(
    account_literal: &str,
    key_id: &str,
    device_id: &str,
    device_key: &P256SigningKey,
    signature: &KagemushaDeviceSignatureV2,
    signing_payload: &[u8],
) -> Value {
    let public_key = device_key.verifying_key().to_encoded_point(false);
    object(vec![
        ("account_id", Value::from(account_literal.to_owned())),
        ("assertion_key_algorithm", Value::from("p256-sha256")),
        (
            "assertion_public_key",
            Value::from(BASE64_STANDARD.encode(public_key.as_bytes())),
        ),
        (
            "assertion_scheme",
            Value::from("kagemusha-device-signature-v2"),
        ),
        ("assertion_usage_count_limit", Value::Null),
        ("device_id", Value::from(device_id.to_owned())),
        (
            "issuer_signature_base64",
            Value::from(BASE64_STANDARD.encode(signature.as_raw_bytes())),
        ),
        (
            "issuer_signature_payload_base64",
            Value::from(BASE64_STANDARD.encode(signing_payload)),
        ),
        ("key_id", Value::from(key_id.to_owned())),
        ("one_use", Value::from(true)),
        ("platform", Value::from("ios-appattest")),
        (
            "public_key",
            Value::from(BASE64_STANDARD.encode(public_key.as_bytes())),
        ),
        ("version", Value::from(2_u64)),
    ])
}

fn sdk_interop_json(payment_bundle_bytes: &[u8]) -> Result<Value, Box<dyn Error>> {
    Ok(object(vec![
        (
            "payment_token_envelope_schema",
            Value::from(PAYMENT_TOKEN_ENVELOPE_SCHEMA),
        ),
        (
            "payment_token_norito_base64",
            Value::from(BASE64_STANDARD.encode(payment_bundle_bytes)),
        ),
        (
            "payment_token_qr",
            fountain_qr_fixture(payment_bundle_bytes, 180, 2)?,
        ),
        (
            "payment_token_sha256_hex",
            Value::from(sha256_hex(payment_bundle_bytes)),
        ),
        (
            "payment_token_text",
            Value::from(format!(
                "{OFFLINE_BEARER_CASH_PAYMENT_PREFIX}{}",
                URL_SAFE_NO_PAD.encode(payment_bundle_bytes)
            )),
        ),
    ]))
}

fn fountain_qr_fixture(
    payload: &[u8],
    chunk_size: u16,
    parity_group: u8,
) -> Result<Value, Box<dyn Error>> {
    let options = QrStreamOptions {
        chunk_size,
        parity_group,
        payload_kind: QrPayloadKind::KagemushaPayment,
        ..QrStreamOptions::default()
    };
    let (envelope, frames) = QrStreamEncoder::encode_frames(payload, options)?;
    let required_unique_frames = frames
        .iter()
        .filter(|frame| frame.kind != QrStreamFrameKind::Parity)
        .count();
    let frames = frames
        .iter()
        .map(|frame| {
            object(vec![
                ("bytes_hex", Value::from(encode(frame.encode()))),
                ("kind", Value::from(frame_kind_label(frame.kind))),
            ])
        })
        .collect();
    Ok(object(vec![
        ("envelope_hex", Value::from(encode(envelope.encode()))),
        ("frame_prefix", Value::from("iroha:qr:")),
        ("frame_size_bytes", Value::from(u64::from(chunk_size))),
        ("frames", Value::Array(frames)),
        ("max_payload_bytes", Value::from(2 * 1024 * 1024_u64)),
        ("payload_sha256_hex", Value::from(sha256_hex(payload))),
        (
            "required_unique_frames",
            Value::from(required_unique_frames as u64),
        ),
    ]))
}

fn frame_kind_label(kind: QrStreamFrameKind) -> &'static str {
    match kind {
        QrStreamFrameKind::Header => "header",
        QrStreamFrameKind::Data => "data",
        QrStreamFrameKind::Parity => "parity",
    }
}

fn fixed_ed25519_keypair(seed: u8) -> Result<KeyPair, Box<dyn Error>> {
    Ok(KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)?)
}

fn fixed_p256_signing_key(seed: u8) -> Result<P256SigningKey, Box<dyn Error>> {
    Ok(P256SigningKey::from_bytes((&[seed; 32]).into())?)
}

fn device_public_key(key: &P256SigningKey) -> Result<KagemushaDevicePublicKeyV2, Box<dyn Error>> {
    Ok(KagemushaDevicePublicKeyV2::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )?)
}

fn sign(
    key: &P256SigningKey,
    message: &[u8],
) -> Result<KagemushaDeviceSignatureV2, Box<dyn Error>> {
    let signature: P256Signature = key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    Ok(KagemushaDeviceSignatureV2::from_raw_bytes(
        signature.to_bytes().as_slice(),
    )?)
}

fn taira_account_literal(account_id: &AccountId) -> Result<String, Box<dyn Error>> {
    let literal = AccountAddress::from_account_id(account_id)?
        .to_i105_for_discriminant(TAIRA_CHAIN_DISCRIMINANT)?;
    let embedded = AccountAddress::i105_discriminant(&literal)?;
    if embedded != TAIRA_CHAIN_DISCRIMINANT {
        return Err(format!(
            "CBSI account literal carries discriminant {embedded}, expected {TAIRA_CHAIN_DISCRIMINANT}"
        )
        .into());
    }
    Ok(literal)
}

fn taira_asset_literal(asset_id: &AssetId) -> Result<String, Box<dyn Error>> {
    if asset_id.scope() != &AssetBalanceScope::Global
        || asset_id.definition().canonical_address() != CBSI_SBD_ASSET_DEFINITION_ID
    {
        return Err("CBSI fixture asset must be exact globally scoped sbd#cbsi".into());
    }
    Ok(format!(
        "{}#{}",
        CBSI_SBD_ASSET_DEFINITION_ID,
        taira_account_literal(asset_id.account())?
    ))
}

fn public_key_base64(key_pair: &KeyPair) -> Result<String, Box<dyn Error>> {
    let (algorithm, bytes) = key_pair.public_key().try_to_bytes()?;
    if algorithm != Algorithm::Ed25519 {
        return Err("CBSI fixture FI key must be Ed25519".into());
    }
    Ok(BASE64_STANDARD.encode(bytes))
}

fn sha256_hex(bytes: &[u8]) -> String {
    encode(Sha256::digest(bytes))
}

fn object(entries: Vec<(&'static str, Value)>) -> Value {
    let mut map = json::Map::new();
    for (key, value) in entries {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

fn string_array(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::from).collect())
}

fn str_array(values: &[&str]) -> Value {
    Value::Array(values.iter().copied().map(Value::from).collect())
}

fn write_fixture(path: &str, value: &Value, check_only: bool) -> Result<(), Box<dyn Error>> {
    let rendered = format!("{}\n", json::to_string_pretty(value)?);
    let path = Path::new(path);
    if path.is_symlink() {
        return Err(format!(
            "fixture output must not be a symbolic link: {}",
            path.display()
        )
        .into());
    }
    if check_only {
        let existing = fs::read(path)?;
        if existing != rendered.as_bytes() {
            return Err(format!(
                "fixture {} is stale; regenerate it with cbsi_offline_vectors",
                path.display()
            )
            .into());
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, rendered)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
        let Value::Object(map) = value else {
            panic!("expected object while reading {key}");
        };
        map.get(key)
            .unwrap_or_else(|| panic!("missing fixture field {key}"))
    }

    fn array(value: &Value) -> &[Value] {
        let Value::Array(values) = value else {
            panic!("expected array");
        };
        values
    }

    fn string(value: &Value) -> &str {
        let Value::String(value) = value else {
            panic!("expected string");
        };
        value
    }

    fn number(value: &Value) -> u64 {
        let Value::Number(value) = value else {
            panic!("expected number");
        };
        value.as_u64().expect("expected unsigned number")
    }

    fn assert_exact_cbsi_identifiers(
        value: &Value,
        field_name: Option<&str>,
        account_literals: &mut usize,
        asset_literals: &mut usize,
    ) {
        match value {
            Value::Object(map) => {
                for (key, nested) in map {
                    assert_exact_cbsi_identifiers(
                        nested,
                        Some(key),
                        account_literals,
                        asset_literals,
                    );
                }
            }
            Value::Array(values) => {
                for nested in values {
                    assert_exact_cbsi_identifiers(
                        nested,
                        field_name,
                        account_literals,
                        asset_literals,
                    );
                }
            }
            Value::String(literal)
                if field_name.is_some_and(|name| {
                    matches!(
                        name,
                        "account_id" | "sender_account_id" | "recipient_account_id"
                    )
                }) =>
            {
                assert_eq!(
                    AccountAddress::i105_discriminant(literal)
                        .expect("fixture account must be canonical I105"),
                    TAIRA_CHAIN_DISCRIMINANT
                );
                *account_literals += 1;
            }
            Value::String(literal) if field_name == Some("asset_definition_id") => {
                assert_eq!(literal, CBSI_SBD_ASSET_DEFINITION_ID);
                *asset_literals += 1;
            }
            Value::String(literal) if field_name == Some("asset_id") => {
                let (definition, account) = literal
                    .split_once('#')
                    .expect("fixture asset balance must include an account");
                assert_eq!(definition, CBSI_SBD_ASSET_DEFINITION_ID);
                assert_eq!(
                    AccountAddress::i105_discriminant(account)
                        .expect("fixture asset account must be canonical I105"),
                    TAIRA_CHAIN_DISCRIMINANT
                );
                *asset_literals += 1;
            }
            _ => {}
        }
    }

    #[test]
    fn committed_interop_fixture_matches_current_typed_generator() {
        let fixture = build_fixture().expect("build fixture");
        let rendered = format!(
            "{}\n",
            json::to_string_pretty(&fixture.fixture).expect("render fixture")
        );
        assert_eq!(
            fs::read(FIXTURE_PATH).expect("read committed fixture"),
            rendered.as_bytes(),
            "committed CBSI Offline Cash fixture is stale"
        );
    }

    #[test]
    fn typed_kagemusha_archives_validate_end_to_end() {
        let parts = build_fixture().expect("build fixture");
        parts
            .source_request
            .validate_public_binding()
            .expect("source request");
        parts
            .recipient_request
            .validate_at(GENERATED_AT_MS)
            .expect("recipient request");
        parts
            .payment_bundle
            .validate_public_binding()
            .expect("payment bundle");
        parts
            .acknowledgement
            .validate_for_payment_v4(&parts.recipient_request, &parts.payment_bundle)
            .expect("receiver acknowledgement");

        let bundle_bytes = BASE64_STANDARD
            .decode(string(field(
                field(&parts.fixture, "sdk_interop"),
                "payment_token_norito_base64",
            )))
            .expect("base64 bundle");
        let decoded: KagemushaRecursiveSpendBundleV4 =
            norito::decode_from_bytes(&bundle_bytes).expect("decode typed bundle");
        assert_eq!(decoded, parts.payment_bundle);
    }

    #[test]
    fn fixture_is_exact_taira_sbd_and_current_kagemusha() {
        let parts = build_fixture().expect("build fixture");
        let binding = field(&parts.fixture, "cbsi_binding");
        assert_eq!(string(field(binding, "network")), "taira");
        assert_eq!(
            number(field(binding, "account_chain_discriminant")),
            u64::from(TAIRA_CHAIN_DISCRIMINANT)
        );
        assert_eq!(string(field(binding, "asset_alias")), CBSI_SBD_ASSET_ALIAS);
        assert_eq!(
            string(field(binding, "asset_definition_id")),
            CBSI_SBD_ASSET_DEFINITION_ID
        );
        let mut accounts = 0;
        let mut assets = 0;
        assert_exact_cbsi_identifiers(&parts.fixture, None, &mut accounts, &mut assets);
        assert!(accounts >= 10);
        assert!(assets >= 8);
        assert_eq!(
            AccountAddress::i105_discriminant(&parts.sender_literal).expect("sender I105"),
            TAIRA_CHAIN_DISCRIMINANT
        );
        assert_eq!(
            AccountAddress::i105_discriminant(&parts.recipient_literal).expect("recipient I105"),
            TAIRA_CHAIN_DISCRIMINANT
        );
        assert_eq!(
            string(field(
                field(&parts.fixture, "sdk_interop"),
                "payment_token_envelope_schema",
            )),
            PAYMENT_TOKEN_ENVELOPE_SCHEMA
        );
        assert_eq!(number(field(&parts.fixture, "version")), 1,);
        assert_eq!(array(field(&parts.fixture, "capabilities")).len(), 5,);
        assert_eq!(
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4,
            21,
        );
    }
}
