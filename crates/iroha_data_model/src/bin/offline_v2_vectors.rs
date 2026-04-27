//! Regenerate canonical Offline V2 interop vectors shared across wallet SDKs.
//!
//! Run with `cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors`
//! to refresh `fixtures/offline/interop_contract_v2.json`. Use `--check` to verify it is up to date.

use std::{env, error::Error, fs, path::Path};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use hex::encode;
use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    offline::{
        OfflineNoteAuditBundleV2, OfflineNoteAuditOutputClaimV2, OfflineNoteIssueV2,
        OfflineNoteIssuedClaimV2, OfflineNoteKeyCertificateV2, OfflineNoteRecursiveProofV2,
        OfflineNoteRedeemV2,
    },
    proof::{ProofBox, VerifyingKeyId},
    qr_stream::{QrPayloadKind, QrStreamEncoder, QrStreamFrameKind, QrStreamOptions},
};
use iroha_primitives::numeric::Numeric;
use norito::{
    json::{self, Value},
    to_bytes,
};
use sha2::{Digest, Sha256};

const FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/offline/interop_contract_v2.json"
);
const TOKEN_ID_LABEL: &str = "offline-v2-token-fixture-1";
const INVOICE_ID: &str = "invoice-fixture-1";
const SENDER_KEY_ID: &str = "sender-key-v2-1";
const RECIPIENT_KEY_ID: &str = "recipient-key-v2-1";
const SENDER_DEVICE_ID: &str = "sender-device-v2-1";
const RECIPIENT_DEVICE_ID: &str = "recipient-device-v2-1";
const AMOUNT: &str = "5";
const CHANGE_AMOUNT: &str = "47";
const ISSUE_AMOUNT: &str = "52";
const GENERATED_AT_MS: u64 = 1_706_000_000_000;
const CREATED_AT_MS: u64 = 1_706_000_000_123;
const ACCEPTED_AT_MS: u64 = 1_706_000_000_333;

fn main() -> Result<(), Box<dyn Error>> {
    let check_only = env::args().any(|arg| arg == "--check");
    let fixture = build_fixture()?;
    write_fixture(FIXTURE_PATH, &fixture, check_only)?;
    Ok(())
}

fn build_fixture() -> Result<Value, Box<dyn Error>> {
    let issuer_key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
    let sender_account_key_pair = KeyPair::from_seed(vec![0x21; 32], Algorithm::Ed25519);
    let recipient_account_key_pair = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
    let sender_note_key_pair = KeyPair::from_seed(vec![0x31; 32], Algorithm::Ed25519);
    let recipient_note_key_pair = KeyPair::from_seed(vec![0x32; 32], Algorithm::Ed25519);

    let sender_account_id = AccountId::new(sender_account_key_pair.public_key().clone());
    let recipient_account_id = AccountId::new(recipient_account_key_pair.public_key().clone());
    let sender_account_id_string = sender_account_id.to_string();
    let recipient_account_id_string = recipient_account_id.to_string();

    let asset_definition_id =
        AssetDefinitionId::new(DomainId::try_new("sbp", "universal")?, "pk_cbdc".parse()?);
    let asset_definition_id_string = asset_definition_id.canonical_address();
    let sender_asset_id = AssetId::new(asset_definition_id.clone(), sender_account_id.clone());
    let recipient_asset_id =
        AssetId::new(asset_definition_id.clone(), recipient_account_id.clone());

    let sender_certificate = signed_certificate(
        &issuer_key_pair,
        &sender_note_key_pair,
        sender_account_id.clone(),
        "ios-appattest",
        SENDER_KEY_ID,
        SENDER_DEVICE_ID,
    )?;
    let recipient_certificate = signed_certificate(
        &issuer_key_pair,
        &recipient_note_key_pair,
        recipient_account_id.clone(),
        "ios-appattest",
        RECIPIENT_KEY_ID,
        RECIPIENT_DEVICE_ID,
    )?;

    let token_id = Hash::new(TOKEN_ID_LABEL);
    let source_note_commitment = Hash::new(b"offline-v2-vector-source-note");
    let input_nullifier = Hash::new(b"offline-v2-vector-input-nullifier");
    let redeem_nullifier = Hash::new(b"offline-v2-vector-redeem-nullifier");
    let recipient_commitment = Hash::new(b"offline-v2-vector-recipient-output");
    let change_commitment = Hash::new(b"offline-v2-vector-change-output");

    let issue = OfflineNoteIssueV2 {
        note_commitment: source_note_commitment,
        key_certificate: sender_certificate.model.clone(),
        asset: sender_asset_id.clone(),
        amount: Numeric::new(52, 0),
    };
    let issue_claim = OfflineNoteIssuedClaimV2::from_issue(&issue)?;
    let audit_input_claims = vec![issue_claim.clone()];
    let recipient_output_claim = OfflineNoteAuditOutputClaimV2 {
        note_commitment: recipient_commitment,
        key_certificate: recipient_certificate.model.clone(),
        asset: recipient_asset_id.clone(),
        amount: Numeric::new(5, 0),
    };
    let change_output_claim = OfflineNoteAuditOutputClaimV2 {
        note_commitment: change_commitment,
        key_certificate: sender_certificate.model.clone(),
        asset: sender_asset_id.clone(),
        amount: Numeric::new(47, 0),
    };
    let audit_output_claims = vec![recipient_output_claim.clone(), change_output_claim.clone()];
    let audit_proof = OfflineNoteRecursiveProofV2 {
        verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
        public_inputs_hash: Hash::new(b"offline-v2-vector-audit-public-inputs-placeholder"),
        proof: ProofBox::new(
            "halo2/ipa".into(),
            b"offline-v2-vector-audit-proof".to_vec(),
        ),
    };
    let audit_for_hash = OfflineNoteAuditBundleV2 {
        token_id,
        sender_key_certificate: sender_certificate.model.clone(),
        input_nullifiers: vec![input_nullifier],
        input_claims: audit_input_claims.clone(),
        output_commitments: vec![recipient_commitment, change_commitment],
        output_claims: audit_output_claims.clone(),
        recursive_proof: audit_proof,
    };
    let audit_public_inputs_hash = audit_for_hash.public_inputs_hash()?;
    let audit = OfflineNoteAuditBundleV2 {
        token_id,
        sender_key_certificate: sender_certificate.model.clone(),
        input_nullifiers: vec![input_nullifier],
        input_claims: audit_input_claims.clone(),
        output_commitments: vec![recipient_commitment, change_commitment],
        output_claims: audit_output_claims.clone(),
        recursive_proof: OfflineNoteRecursiveProofV2 {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
            public_inputs_hash: audit_public_inputs_hash,
            proof: ProofBox::new(
                "halo2/ipa".into(),
                b"offline-v2-vector-audit-proof".to_vec(),
            ),
        },
    };

    let redeem_proof = OfflineNoteRecursiveProofV2 {
        verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
        public_inputs_hash: Hash::new(b"offline-v2-vector-redeem-public-inputs-placeholder"),
        proof: ProofBox::new(
            "halo2/ipa".into(),
            b"offline-v2-vector-redeem-proof".to_vec(),
        ),
    };
    let redeem_for_hash = OfflineNoteRedeemV2 {
        source_note_commitment: recipient_commitment,
        input_nullifiers: vec![redeem_nullifier],
        sender_key_certificate: recipient_certificate.model.clone(),
        recipient: recipient_account_id.clone(),
        asset: recipient_asset_id.clone(),
        amount: Numeric::new(5, 0),
        recursive_proof: redeem_proof,
    };
    let redeem_public_inputs_hash = redeem_for_hash.public_inputs_hash()?;
    let redeem = OfflineNoteRedeemV2 {
        source_note_commitment: recipient_commitment,
        input_nullifiers: vec![redeem_nullifier],
        sender_key_certificate: recipient_certificate.model.clone(),
        recipient: recipient_account_id.clone(),
        asset: recipient_asset_id.clone(),
        amount: Numeric::new(5, 0),
        recursive_proof: OfflineNoteRecursiveProofV2 {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
            public_inputs_hash: redeem_public_inputs_hash,
            proof: ProofBox::new(
                "halo2/ipa".into(),
                b"offline-v2-vector-redeem-proof".to_vec(),
            ),
        },
    };

    let token_id_string = token_id.to_string();
    let input_nullifier_string = input_nullifier.to_string();
    let source_note_commitment_string = source_note_commitment.to_string();
    let recipient_commitment_string = recipient_commitment.to_string();
    let change_commitment_string = change_commitment.to_string();
    let output_commitments = vec![
        recipient_commitment_string.clone(),
        change_commitment_string.clone(),
    ];
    let input_nullifiers = vec![input_nullifier_string.clone()];
    let input_claim_hashes = audit_input_claims
        .iter()
        .map(|claim| Ok(claim.claim_hash()?.to_string()))
        .collect::<Result<Vec<_>, norito::Error>>()?;
    let input_claim_values = audit_input_claims
        .iter()
        .map(mobile_input_claim_json)
        .collect::<Result<Vec<_>, _>>()?;

    let output_claim_values = vec![
        mobile_output_claim_json(
            &recipient_commitment_string,
            &recipient_certificate,
            &recipient_account_id_string,
            &asset_definition_id_string,
            AMOUNT,
        )?,
        mobile_output_claim_json(
            &change_commitment_string,
            &sender_certificate,
            &sender_account_id_string,
            &asset_definition_id_string,
            CHANGE_AMOUNT,
        )?,
    ];

    let public_inputs_hash_hex = mobile_public_inputs_hash_hex(MobilePublicInputFields {
        token_id: &token_id_string,
        invoice_id: INVOICE_ID,
        sender_account_id: &sender_account_id_string,
        recipient_account_id: &recipient_account_id_string,
        sender_certificate: &sender_certificate,
        recipient_certificate: &recipient_certificate,
        asset_definition_id: &asset_definition_id_string,
        amount: AMOUNT,
        change_amount: CHANGE_AMOUNT,
        source_note_commitment: Some(&source_note_commitment_string),
        input_nullifiers: &input_nullifiers,
        input_claims: &input_claim_hashes,
        output_commitments: &output_commitments,
        output_claims: &[
            MobileOutputClaim {
                note_commitment: &recipient_commitment_string,
                account_id: &recipient_account_id_string,
                asset_definition_id: &asset_definition_id_string,
                amount: AMOUNT,
                key_certificate: &recipient_certificate,
            },
            MobileOutputClaim {
                note_commitment: &change_commitment_string,
                account_id: &sender_account_id_string,
                asset_definition_id: &asset_definition_id_string,
                amount: CHANGE_AMOUNT,
                key_certificate: &sender_certificate,
            },
        ],
    });
    let one_use_signature = Signature::new(
        sender_note_key_pair.private_key(),
        public_inputs_hash_hex.as_bytes(),
    );
    let one_use_signature_base64 = BASE64_STANDARD.encode(one_use_signature.payload());
    let sender_public_key_base64 = public_key_base64(&sender_note_key_pair);
    let proof_transcript = [
        "offline-note-v2-proof-v1",
        "ed25519",
        &public_inputs_hash_hex,
        &sender_public_key_base64,
        &one_use_signature_base64,
    ]
    .join(":");
    let proof_bytes_base64 = BASE64_STANDARD.encode(proof_transcript.as_bytes());

    let payment_token = payment_token_json(PaymentTokenJsonFields {
        token_id: &token_id_string,
        sender_account_id: &sender_account_id_string,
        recipient_account_id: &recipient_account_id_string,
        asset_definition_id: &asset_definition_id_string,
        source_note_commitment: &source_note_commitment_string,
        input_nullifiers: &input_nullifiers,
        input_claims: &input_claim_values,
        output_commitments: &output_commitments,
        output_claims: &output_claim_values,
        sender_certificate: &sender_certificate,
        recipient_certificate: &recipient_certificate,
        public_inputs_hash_hex: &public_inputs_hash_hex,
        assertion_base64: &one_use_signature_base64,
        proof_bytes_base64: &proof_bytes_base64,
    })?;
    let receive_challenge = object(vec![
        ("version", Value::from(2_u64)),
        ("type", Value::from("offline_receive_challenge_v2")),
        ("invoice_id", Value::from(INVOICE_ID)),
        (
            "account_id",
            Value::from(recipient_account_id_string.clone()),
        ),
        (
            "asset_definition_id",
            Value::from(asset_definition_id_string.clone()),
        ),
        ("amount", Value::from(AMOUNT)),
        (
            "recipient_key_certificate",
            mobile_certificate_json(&recipient_certificate),
        ),
        ("display_ttl_ms", Value::from(60_000_u64)),
        ("generated_at_ms", Value::from(GENERATED_AT_MS)),
    ]);
    let receipt_ack = object(vec![
        ("version", Value::from(2_u64)),
        ("type", Value::from("offline_receipt_ack_v2")),
        ("token_id", Value::from(token_id_string.clone())),
        (
            "recipient_account_id",
            Value::from(recipient_account_id_string.clone()),
        ),
        ("accepted_at_ms", Value::from(ACCEPTED_AT_MS)),
    ]);

    let payment_token_payload = json::to_string(&payment_token)?;
    let fountain = fountain_qr_fixture(payment_token_payload.as_bytes())?;
    let audit_output_claim_hashes = audit_output_claims
        .iter()
        .map(|claim| Ok(OfflineNoteIssuedClaimV2::from_audit_output(claim)?.claim_hash()?))
        .collect::<Result<Vec<Hash>, norito::Error>>()?;
    let redeem_claim = OfflineNoteIssuedClaimV2::from_redemption(&redeem)?;

    Ok(object(vec![
        ("version", Value::from(2_u64)),
        (
            "generator",
            Value::from(
                "cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors",
            ),
        ),
        (
            "prefixes",
            object(vec![
                (
                    "receive_challenge",
                    Value::from("wallet-offline-challenge-v2:"),
                ),
                ("payment_token", Value::from("wallet-offline-payment-v2:")),
                ("receipt_ack", Value::from("wallet-offline-ack-v2:")),
                ("fountain_qr", Value::from("fountainqr-v1:")),
            ]),
        ),
        (
            "capabilities",
            str_array(&[
                "offline_note_v2",
                "offline_one_use_keys",
                "offline_recursive_note_proof",
                "offline_fountain_qr_v1",
                "offline_sync_optional",
                "offline_telemetry",
            ]),
        ),
        (
            "offline_fi_public_key_base64",
            Value::from(public_key_base64(&issuer_key_pair)),
        ),
        ("receive_challenge", receive_challenge),
        ("payment_token", payment_token),
        ("receipt_ack", receipt_ack),
        ("fountain_qr_v1", fountain),
        (
            "chain_vectors",
            object(vec![
                (
                    "certificates",
                    object(vec![
                        (
                            "sender_payload_hash",
                            Value::from(sender_certificate.model.payload_hash()?.to_string()),
                        ),
                        (
                            "recipient_payload_hash",
                            Value::from(recipient_certificate.model.payload_hash()?.to_string()),
                        ),
                        (
                            "sender_payload_base64",
                            Value::from(sender_certificate.issuer_signature_payload_base64.clone()),
                        ),
                        (
                            "recipient_payload_base64",
                            Value::from(
                                recipient_certificate
                                    .issuer_signature_payload_base64
                                    .clone(),
                            ),
                        ),
                    ]),
                ),
                (
                    "issue",
                    object(vec![
                        (
                            "note_commitment",
                            Value::from(source_note_commitment_string),
                        ),
                        ("asset_id", Value::from(sender_asset_id.to_string())),
                        ("amount", Value::from(ISSUE_AMOUNT)),
                        (
                            "claim_hash",
                            Value::from(issue_claim.claim_hash()?.to_string()),
                        ),
                        (
                            "norito_base64",
                            Value::from(BASE64_STANDARD.encode(to_bytes(&issue)?)),
                        ),
                    ]),
                ),
                (
                    "audit",
                    object(vec![
                        ("token_id", Value::from(token_id_string)),
                        (
                            "input_nullifiers",
                            Value::Array(vec![Value::from(input_nullifier_string)]),
                        ),
                        ("input_claim_hashes", string_array(&input_claim_hashes)),
                        ("output_commitments", string_array(&output_commitments)),
                        (
                            "output_claim_hashes",
                            string_array(
                                &audit_output_claim_hashes
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>(),
                            ),
                        ),
                        (
                            "public_inputs_hash",
                            Value::from(audit_public_inputs_hash.to_string()),
                        ),
                        (
                            "norito_base64",
                            Value::from(BASE64_STANDARD.encode(to_bytes(&audit)?)),
                        ),
                    ]),
                ),
                (
                    "redeem",
                    object(vec![
                        (
                            "source_note_commitment",
                            Value::from(recipient_commitment_string),
                        ),
                        (
                            "input_nullifiers",
                            Value::Array(vec![Value::from(redeem_nullifier.to_string())]),
                        ),
                        ("asset_id", Value::from(recipient_asset_id.to_string())),
                        ("amount", Value::from(AMOUNT)),
                        (
                            "claim_hash",
                            Value::from(redeem_claim.claim_hash()?.to_string()),
                        ),
                        (
                            "public_inputs_hash",
                            Value::from(redeem_public_inputs_hash.to_string()),
                        ),
                        (
                            "norito_base64",
                            Value::from(BASE64_STANDARD.encode(to_bytes(&redeem)?)),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "bad_variants",
            Value::Array(vec![
                object(vec![
                    ("name", Value::from("forged_output_claim_amount")),
                    (
                        "patch",
                        Value::from("payment_token.output_claims[0].amount=6"),
                    ),
                    ("expected_error", Value::from("public_inputs_hash_mismatch")),
                ]),
                object(vec![
                    ("name", Value::from("wrong_recipient_output_claim")),
                    (
                        "patch",
                        Value::from(
                            "payment_token.output_claims[0].account_id=payment_token.sender_account_id",
                        ),
                    ),
                    ("expected_error", Value::from("recipient_binding_mismatch")),
                ]),
                object(vec![
                    ("name", Value::from("reused_one_use_counter")),
                    (
                        "patch",
                        Value::from("payment_token.one_use_assertion.counter=2"),
                    ),
                    ("expected_error", Value::from("one_use_assertion_replay")),
                ]),
            ]),
        ),
    ]))
}

struct MobilePublicInputFields<'a> {
    token_id: &'a str,
    invoice_id: &'a str,
    sender_account_id: &'a str,
    recipient_account_id: &'a str,
    sender_certificate: &'a VectorCertificate,
    recipient_certificate: &'a VectorCertificate,
    asset_definition_id: &'a str,
    amount: &'a str,
    change_amount: &'a str,
    source_note_commitment: Option<&'a str>,
    input_nullifiers: &'a [String],
    input_claims: &'a [String],
    output_commitments: &'a [String],
    output_claims: &'a [MobileOutputClaim<'a>],
}

struct MobileOutputClaim<'a> {
    note_commitment: &'a str,
    account_id: &'a str,
    asset_definition_id: &'a str,
    amount: &'a str,
    key_certificate: &'a VectorCertificate,
}

struct PaymentTokenJsonFields<'a> {
    token_id: &'a str,
    sender_account_id: &'a str,
    recipient_account_id: &'a str,
    asset_definition_id: &'a str,
    source_note_commitment: &'a str,
    input_nullifiers: &'a [String],
    input_claims: &'a [Value],
    output_commitments: &'a [String],
    output_claims: &'a [Value],
    sender_certificate: &'a VectorCertificate,
    recipient_certificate: &'a VectorCertificate,
    public_inputs_hash_hex: &'a str,
    assertion_base64: &'a str,
    proof_bytes_base64: &'a str,
}

#[derive(Clone)]
struct VectorCertificate {
    model: OfflineNoteKeyCertificateV2,
    version: u16,
    platform: String,
    key_id: String,
    device_id: String,
    account_id: String,
    public_key_base64: String,
    one_use: bool,
    issuer_signature_base64: String,
    issuer_signature_payload_base64: String,
}

fn signed_certificate(
    issuer_key_pair: &KeyPair,
    note_key_pair: &KeyPair,
    account_id: AccountId,
    platform: &str,
    key_id: &str,
    device_id: &str,
) -> Result<VectorCertificate, Box<dyn Error>> {
    let (_algorithm, public_key) = note_key_pair.public_key().to_bytes();
    let public_key = public_key.to_vec();
    let unsigned_certificate = OfflineNoteKeyCertificateV2 {
        version: 2,
        platform: platform.to_owned(),
        key_id: key_id.to_owned(),
        device_id: device_id.to_owned(),
        account_id: account_id.clone(),
        public_key: public_key.clone(),
        one_use: true,
        issuer_signature: Signature::from_bytes(&[0_u8; 64]),
    };
    let signing_bytes = unsigned_certificate.signing_bytes()?;
    let issuer_signature = Signature::new(issuer_key_pair.private_key(), &signing_bytes);
    let certificate = OfflineNoteKeyCertificateV2 {
        version: 2,
        platform: platform.to_owned(),
        key_id: key_id.to_owned(),
        device_id: device_id.to_owned(),
        account_id: account_id.clone(),
        public_key: public_key.clone(),
        one_use: true,
        issuer_signature: issuer_signature.clone(),
    };
    Ok(VectorCertificate {
        model: certificate,
        version: 2,
        platform: platform.to_owned(),
        key_id: key_id.to_owned(),
        device_id: device_id.to_owned(),
        account_id: account_id.to_string(),
        public_key_base64: BASE64_STANDARD.encode(&public_key),
        one_use: true,
        issuer_signature_base64: BASE64_STANDARD.encode(issuer_signature.payload()),
        issuer_signature_payload_base64: BASE64_STANDARD.encode(signing_bytes),
    })
}

fn mobile_certificate_json(certificate: &VectorCertificate) -> Value {
    object(vec![
        ("version", Value::from(certificate.version)),
        ("platform", Value::from(certificate.platform.clone())),
        ("key_id", Value::from(certificate.key_id.clone())),
        ("device_id", Value::from(certificate.device_id.clone())),
        ("account_id", Value::from(certificate.account_id.clone())),
        (
            "public_key",
            Value::from(certificate.public_key_base64.clone()),
        ),
        ("one_use", Value::from(certificate.one_use)),
        (
            "issuer_signature_base64",
            Value::from(certificate.issuer_signature_base64.clone()),
        ),
        (
            "issuer_signature_payload_base64",
            Value::from(certificate.issuer_signature_payload_base64.clone()),
        ),
    ])
}

fn mobile_output_claim_json(
    note_commitment: &str,
    certificate: &VectorCertificate,
    account_id: &str,
    asset_definition_id: &str,
    amount: &str,
) -> Result<Value, Box<dyn Error>> {
    Ok(object(vec![
        ("note_commitment", Value::from(note_commitment)),
        ("key_certificate", mobile_certificate_json(certificate)),
        ("account_id", Value::from(account_id)),
        ("asset_definition_id", Value::from(asset_definition_id)),
        ("amount", Value::from(amount)),
    ]))
}

fn mobile_input_claim_json(claim: &OfflineNoteIssuedClaimV2) -> Result<Value, Box<dyn Error>> {
    Ok(object(vec![
        ("domain", Value::from(claim.domain.clone())),
        (
            "note_commitment",
            Value::from(claim.note_commitment.to_string()),
        ),
        (
            "key_certificate_payload_hash",
            Value::from(claim.key_certificate_payload_hash.to_string()),
        ),
        ("asset_id", Value::from(claim.asset.to_string())),
        ("amount", Value::from(claim.amount.to_string())),
        ("claim_hash", Value::from(claim.claim_hash()?.to_string())),
    ]))
}

fn payment_token_json(fields: PaymentTokenJsonFields<'_>) -> Result<Value, Box<dyn Error>> {
    Ok(object(vec![
        ("version", Value::from(2_u64)),
        ("type", Value::from("offline_payment_token_v2")),
        ("token_id", Value::from(fields.token_id)),
        ("invoice_id", Value::from(INVOICE_ID)),
        ("sender_account_id", Value::from(fields.sender_account_id)),
        (
            "recipient_account_id",
            Value::from(fields.recipient_account_id),
        ),
        (
            "asset_definition_id",
            Value::from(fields.asset_definition_id),
        ),
        ("amount", Value::from(AMOUNT)),
        ("change_amount", Value::from(CHANGE_AMOUNT)),
        (
            "source_note_commitment",
            Value::from(fields.source_note_commitment),
        ),
        ("input_nullifiers", string_array(fields.input_nullifiers)),
        ("input_claims", Value::Array(fields.input_claims.to_vec())),
        (
            "output_commitments",
            string_array(fields.output_commitments),
        ),
        ("output_claims", Value::Array(fields.output_claims.to_vec())),
        (
            "sender_key_certificate",
            mobile_certificate_json(fields.sender_certificate),
        ),
        (
            "recipient_key_certificate",
            mobile_certificate_json(fields.recipient_certificate),
        ),
        (
            "one_use_assertion",
            object(vec![
                ("platform", Value::from("ios-appattest")),
                ("key_id", Value::from(SENDER_KEY_ID)),
                ("counter", Value::from(1_u64)),
                (
                    "challenge_hash_hex",
                    Value::from(fields.public_inputs_hash_hex),
                ),
                ("assertion_base64", Value::from(fields.assertion_base64)),
            ]),
        ),
        (
            "recursive_proof",
            object(vec![
                (
                    "verifier_key_id",
                    Value::from("offline-note-v2-recursive-v1"),
                ),
                (
                    "public_inputs_hash_hex",
                    Value::from(fields.public_inputs_hash_hex),
                ),
                ("proof_bytes_base64", Value::from(fields.proof_bytes_base64)),
            ]),
        ),
        ("created_at_ms", Value::from(CREATED_AT_MS)),
    ]))
}

fn mobile_public_inputs_hash_hex(fields: MobilePublicInputFields<'_>) -> String {
    let mut transcript_fields = vec![
        "offline-note-v2-public-inputs-v1".to_owned(),
        format!("token_id={}", fields.token_id),
        format!("invoice_id={}", fields.invoice_id),
        format!("sender_account_id={}", fields.sender_account_id),
        format!("recipient_account_id={}", fields.recipient_account_id),
        format!("sender_key_id={}", fields.sender_certificate.key_id),
        format!(
            "sender_public_key={}",
            fields.sender_certificate.public_key_base64
        ),
        format!("recipient_key_id={}", fields.recipient_certificate.key_id),
        format!(
            "recipient_public_key={}",
            fields.recipient_certificate.public_key_base64
        ),
        format!("asset_definition_id={}", fields.asset_definition_id),
        format!("amount={}", fields.amount),
        format!("change_amount={}", fields.change_amount),
    ];
    if let Some(source_note_commitment) = fields.source_note_commitment {
        if !source_note_commitment.trim().is_empty() {
            transcript_fields.push(format!("source_note_commitment={source_note_commitment}"));
        }
    }
    transcript_fields.push(format!(
        "input_nullifiers={}",
        fields.input_nullifiers.join(",")
    ));
    transcript_fields.push(format!("input_claims={}", fields.input_claims.join(",")));
    transcript_fields.push(format!(
        "output_commitments={}",
        fields.output_commitments.join(",")
    ));
    transcript_fields.push(format!(
        "output_claims={}",
        fields
            .output_claims
            .iter()
            .map(output_claim_transcript_field)
            .collect::<Vec<_>>()
            .join(",")
    ));
    let transcript = transcript_fields.join("|");
    encode(Sha256::digest(transcript.as_bytes()))
}

fn output_claim_transcript_field(claim: &MobileOutputClaim<'_>) -> String {
    [
        claim.note_commitment,
        claim.account_id,
        claim.asset_definition_id,
        claim.amount,
        &claim.key_certificate.key_id,
        &claim.key_certificate.public_key_base64,
    ]
    .join(":")
}

fn fountain_qr_fixture(payload: &[u8]) -> Result<Value, Box<dyn Error>> {
    let options = QrStreamOptions {
        chunk_size: 360,
        parity_group: 3,
        payload_kind: QrPayloadKind::OfflinePaymentTokenV2,
        ..QrStreamOptions::default()
    };
    let (envelope, frames) = QrStreamEncoder::encode_frames(payload, options)?;
    let frames_value = frames
        .iter()
        .map(|frame| {
            object(vec![
                ("kind", Value::from(frame_kind_label(frame.kind))),
                ("bytes_hex", Value::from(encode(frame.encode()))),
            ])
        })
        .collect::<Vec<_>>();
    Ok(object(vec![
        ("frame_prefix", Value::from("fountainqr-v1:")),
        (
            "payload_sha256_hex",
            Value::from(encode(Sha256::digest(payload))),
        ),
        ("frame_size_bytes", Value::from(options.chunk_size as u64)),
        (
            "required_unique_frames",
            Value::from(
                frames
                    .iter()
                    .filter(|frame| frame.kind != QrStreamFrameKind::Parity)
                    .count() as u64,
            ),
        ),
        ("max_payload_bytes", Value::from(12_288_u64)),
        ("envelope_hex", Value::from(encode(envelope.encode()))),
        ("frames", Value::Array(frames_value)),
    ]))
}

fn frame_kind_label(kind: QrStreamFrameKind) -> &'static str {
    match kind {
        QrStreamFrameKind::Header => "header",
        QrStreamFrameKind::Data => "data",
        QrStreamFrameKind::Parity => "parity",
    }
}

fn public_key_base64(key_pair: &KeyPair) -> String {
    let (_algorithm, public_key) = key_pair.public_key().to_bytes();
    BASE64_STANDARD.encode(public_key)
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
    let rendered = json::to_string_pretty(value)?;
    if check_only {
        let existing = fs::read_to_string(path)?;
        if existing.trim() != rendered.trim() {
            return Err(format!(
                "fixture {path} is stale; run cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors"
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

    #[test]
    fn committed_interop_fixture_matches_generated_data_model_vectors() {
        let fixture = build_fixture().expect("build fixture");
        let rendered = json::to_string_pretty(&fixture).expect("render fixture");
        let committed = fs::read_to_string(FIXTURE_PATH).expect("read committed fixture");
        assert_eq!(
            committed.trim(),
            rendered.trim(),
            "committed Offline V2 interop fixture is stale"
        );
    }

    #[test]
    fn committed_interop_fixture_covers_v2_lifecycle_surfaces() {
        let committed = fs::read_to_string(FIXTURE_PATH).expect("read committed fixture");
        let fixture: Value = json::from_str(&committed).expect("parse committed fixture");

        assert_eq!(number(field(&fixture, "version")), 2);
        let capabilities = array(field(&fixture, "capabilities"));
        assert!(
            capabilities
                .iter()
                .any(|capability| string(capability) == "offline_note_v2")
        );
        assert!(
            capabilities
                .iter()
                .any(|capability| string(capability) == "offline_one_use_keys")
        );
        assert!(
            capabilities
                .iter()
                .any(|capability| string(capability) == "offline_recursive_note_proof")
        );

        let prefixes = field(&fixture, "prefixes");
        assert_eq!(
            string(field(prefixes, "receive_challenge")),
            "wallet-offline-challenge-v2:"
        );
        assert_eq!(
            string(field(prefixes, "payment_token")),
            "wallet-offline-payment-v2:"
        );
        assert_eq!(
            string(field(prefixes, "receipt_ack")),
            "wallet-offline-ack-v2:"
        );

        let token = field(&fixture, "payment_token");
        assert_eq!(string(field(token, "type")), "offline_payment_token_v2");
        assert_eq!(array(field(token, "input_nullifiers")).len(), 1);
        assert_eq!(array(field(token, "input_claims")).len(), 1);
        assert_eq!(array(field(token, "output_commitments")).len(), 2);
        assert_eq!(array(field(token, "output_claims")).len(), 2);
        let proof = field(token, "recursive_proof");
        assert_eq!(
            string(field(proof, "verifier_key_id")),
            "offline-note-v2-recursive-v1"
        );
        assert!(!string(field(proof, "public_inputs_hash_hex")).is_empty());
        let assertion = field(token, "one_use_assertion");
        assert_eq!(number(field(assertion, "counter")), 1);
        assert_eq!(
            string(field(assertion, "challenge_hash_hex")),
            string(field(proof, "public_inputs_hash_hex"))
        );

        let fountain = field(&fixture, "fountain_qr_v1");
        assert_eq!(string(field(fountain, "frame_prefix")), "fountainqr-v1:");
        assert!(number(field(fountain, "required_unique_frames")) > 0);
        assert!(
            array(field(fountain, "frames")).len()
                >= number(field(fountain, "required_unique_frames")) as usize
        );

        let chain = field(&fixture, "chain_vectors");
        for section in ["issue", "audit", "redeem"] {
            assert!(
                !string(field(field(chain, section), "norito_base64")).is_empty(),
                "{section} chain vector must expose Norito bytes"
            );
        }
        assert_eq!(
            array(field(field(chain, "audit"), "input_claim_hashes")).len(),
            1
        );
        assert_eq!(
            array(field(field(chain, "audit"), "output_claim_hashes")).len(),
            2
        );
        assert!(!string(field(field(chain, "audit"), "public_inputs_hash")).is_empty());
        assert!(!string(field(field(chain, "redeem"), "public_inputs_hash")).is_empty());
        assert_eq!(array(field(&fixture, "bad_variants")).len(), 3);
    }
}
