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
        OfflineNoteAuditBundle, OfflineNoteAuditOutputClaim, OfflineNoteIssue,
        OfflineNoteIssuedClaim, OfflineNoteKeyCertificate, OfflineNoteRecursiveProof,
        OfflineNoteRedeem,
    },
    proof::{ProofBox, VerifyingKeyId},
    qr_stream::{QrPayloadKind, QrStreamEncoder, QrStreamFrameKind, QrStreamOptions},
};
use iroha_primitives::numeric::Numeric;
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

#[allow(clippy::too_many_lines)]
fn build_fixture() -> Result<Value, Box<dyn Error>> {
    let issuer_key_pair = fixed_ed25519_keypair("issuer", 0x11)?;
    let sender_account_key_pair = fixed_ed25519_keypair("sender account", 0x21)?;
    let recipient_account_key_pair = fixed_ed25519_keypair("recipient account", 0x22)?;
    let sender_note_key_pair = fixed_ed25519_keypair("sender note", 0x31)?;
    let recipient_note_key_pair = fixed_ed25519_keypair("recipient note", 0x32)?;

    let sender_account_id = AccountId::new(sender_account_key_pair.public_key().clone());
    let recipient_account_id = AccountId::new(recipient_account_key_pair.public_key().clone());
    let sender_account_id_string = sender_account_id.to_string();
    let recipient_account_id_string = recipient_account_id.to_string();

    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("paynet", "universal")?,
        "pk_cbdc".parse()?,
    );
    let asset_definition_id_string = asset_definition_id.canonical_address();
    let sender_asset_id = AssetId::new(asset_definition_id.clone(), sender_account_id.clone());
    let recipient_asset_id =
        AssetId::new(asset_definition_id.clone(), recipient_account_id.clone());

    let sender_certificate = signed_certificate(
        &issuer_key_pair,
        &sender_note_key_pair,
        &sender_account_id,
        "ios-appattest",
        SENDER_KEY_ID,
        SENDER_DEVICE_ID,
    )?;
    let recipient_certificate = signed_certificate(
        &issuer_key_pair,
        &recipient_note_key_pair,
        &recipient_account_id,
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

    let issue = OfflineNoteIssue {
        note_commitment: source_note_commitment,
        key_certificate: sender_certificate.model.clone(),
        asset: sender_asset_id.clone(),
        amount: Numeric::new(52, 0),
    };
    let issue_claim = OfflineNoteIssuedClaim::from_issue(&issue)?;
    let audit_input_claims = vec![issue_claim.clone()];
    let recipient_output_claim = OfflineNoteAuditOutputClaim {
        note_commitment: recipient_commitment,
        key_certificate: recipient_certificate.model.clone(),
        asset: recipient_asset_id.clone(),
        amount: Numeric::new(5, 0),
    };
    let change_output_claim = OfflineNoteAuditOutputClaim {
        note_commitment: change_commitment,
        key_certificate: sender_certificate.model.clone(),
        asset: sender_asset_id.clone(),
        amount: Numeric::new(47, 0),
    };
    let audit_output_claims = vec![recipient_output_claim.clone(), change_output_claim.clone()];
    let audit_proof = OfflineNoteRecursiveProof {
        verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
        public_inputs_hash: Hash::new(b"offline-v2-vector-audit-public-inputs-placeholder"),
        proof: ProofBox::new(
            "halo2/ipa".into(),
            b"offline-v2-vector-audit-proof".to_vec(),
        ),
    };
    let audit_for_hash = OfflineNoteAuditBundle {
        token_id,
        sender_key_certificate: sender_certificate.model.clone(),
        input_nullifiers: vec![input_nullifier],
        input_claims: audit_input_claims.clone(),
        output_commitments: vec![recipient_commitment, change_commitment],
        output_claims: audit_output_claims.clone(),
        recursive_proof: audit_proof,
    };
    let audit_public_inputs_hash = audit_for_hash.public_inputs_hash()?;
    let audit = OfflineNoteAuditBundle {
        token_id,
        sender_key_certificate: sender_certificate.model.clone(),
        input_nullifiers: vec![input_nullifier],
        input_claims: audit_input_claims.clone(),
        output_commitments: vec![recipient_commitment, change_commitment],
        output_claims: audit_output_claims.clone(),
        recursive_proof: OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
            public_inputs_hash: audit_public_inputs_hash,
            proof: ProofBox::new(
                "halo2/ipa".into(),
                b"offline-v2-vector-audit-proof".to_vec(),
            ),
        },
    };

    let redeem_proof = OfflineNoteRecursiveProof {
        verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-v2-recursive-v1"),
        public_inputs_hash: Hash::new(b"offline-v2-vector-redeem-public-inputs-placeholder"),
        proof: ProofBox::new(
            "halo2/ipa".into(),
            b"offline-v2-vector-redeem-proof".to_vec(),
        ),
    };
    let redeem_for_hash = OfflineNoteRedeem {
        source_note_commitment: recipient_commitment,
        input_nullifiers: vec![redeem_nullifier],
        sender_key_certificate: recipient_certificate.model.clone(),
        recipient: recipient_account_id.clone(),
        asset: recipient_asset_id.clone(),
        amount: Numeric::new(5, 0),
        recursive_proof: redeem_proof,
    };
    let redeem_public_inputs_hash = redeem_for_hash.public_inputs_hash()?;
    let redeem = OfflineNoteRedeem {
        source_note_commitment: recipient_commitment,
        input_nullifiers: vec![redeem_nullifier],
        sender_key_certificate: recipient_certificate.model.clone(),
        recipient: recipient_account_id.clone(),
        asset: recipient_asset_id.clone(),
        amount: Numeric::new(5, 0),
        recursive_proof: OfflineNoteRecursiveProof {
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
        ),
        mobile_output_claim_json(
            &change_commitment_string,
            &sender_certificate,
            &sender_account_id_string,
            &asset_definition_id_string,
            CHANGE_AMOUNT,
        ),
    ];

    let public_inputs_hash_hex = audit_public_inputs_hash.to_string();
    let one_use_signature = sign_p256_assertion(
        &sender_certificate.assertion_signing_key,
        public_inputs_hash_hex.as_bytes(),
    );
    let one_use_signature_base64 = BASE64_STANDARD.encode(one_use_signature);
    let proof_bytes_base64 = BASE64_STANDARD.encode(b"offline-v2-vector-audit-proof");

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
    });
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
        .map(|claim| Ok(OfflineNoteIssuedClaim::from_audit_output(claim)?.claim_hash()?))
        .collect::<Result<Vec<Hash>, norito::Error>>()?;
    let redeem_claim = OfflineNoteIssuedClaim::from_redemption(&redeem)?;

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
                ("fountain_qr", Value::from("iroha:qr1:")),
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
            Value::from(public_key_base64(&issuer_key_pair)?),
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

#[derive(Clone, Copy)]
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
    model: OfflineNoteKeyCertificate,
    version: u16,
    platform: String,
    key_id: String,
    device_id: String,
    account_id: String,
    public_key_base64: String,
    assertion_scheme: String,
    assertion_key_algorithm: String,
    assertion_public_key_base64: String,
    assertion_usage_count_limit: Option<u32>,
    assertion_signing_key: P256SigningKey,
    one_use: bool,
    issuer_signature_base64: String,
    issuer_signature_payload_base64: String,
}

fn fixed_ed25519_keypair(label: &str, seed_byte: u8) -> Result<KeyPair, Box<dyn Error>> {
    KeyPair::try_from_seed(vec![seed_byte; 32], Algorithm::Ed25519).map_err(|err| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("failed to derive offline v2 fixture {label} Ed25519 keypair: {err}"),
        )
        .into()
    })
}

fn signed_certificate(
    issuer_key_pair: &KeyPair,
    note_key_pair: &KeyPair,
    account_id: &AccountId,
    platform: &str,
    key_id: &str,
    device_id: &str,
) -> Result<VectorCertificate, Box<dyn Error>> {
    let public_key = checked_ed25519_public_key_payload(note_key_pair, "note public key")?;
    let public_key = public_key.to_vec();
    let assertion_signing_key = p256_assertion_signing_key(platform, key_id, device_id);
    let assertion_public_key = p256_assertion_public_key(&assertion_signing_key);
    let (assertion_scheme, assertion_key_algorithm, assertion_usage_count_limit) =
        if platform == "android-keymint" || platform == "android" {
            (
                "android-keymint-ecdsa-p256-usage-limit-v1".to_owned(),
                "ecdsa-p256-sha256".to_owned(),
                Some(1),
            )
        } else {
            (
                "apple-appattest-counter-v1".to_owned(),
                "app-attest-p256".to_owned(),
                None,
            )
        };
    let unsigned_certificate = OfflineNoteKeyCertificate {
        version: 2,
        platform: platform.to_owned(),
        key_id: key_id.to_owned(),
        device_id: device_id.to_owned(),
        account_id: account_id.clone(),
        public_key: public_key.clone(),
        assertion_scheme: assertion_scheme.clone(),
        assertion_key_algorithm: assertion_key_algorithm.clone(),
        assertion_public_key: assertion_public_key.clone(),
        assertion_usage_count_limit,
        one_use: true,
        issuer_signature: Signature::from_bytes(&[0_u8; 64]),
    };
    let signing_bytes = unsigned_certificate.signing_bytes()?;
    let issuer_signature = Signature::new(issuer_key_pair.private_key(), &signing_bytes);
    let certificate = OfflineNoteKeyCertificate {
        version: 2,
        platform: platform.to_owned(),
        key_id: key_id.to_owned(),
        device_id: device_id.to_owned(),
        account_id: account_id.clone(),
        public_key: public_key.clone(),
        assertion_scheme,
        assertion_key_algorithm,
        assertion_public_key,
        assertion_usage_count_limit,
        one_use: true,
        issuer_signature: issuer_signature.clone(),
    };
    Ok(VectorCertificate {
        model: certificate.clone(),
        version: 2,
        platform: platform.to_owned(),
        key_id: key_id.to_owned(),
        device_id: device_id.to_owned(),
        account_id: account_id.to_string(),
        public_key_base64: BASE64_STANDARD.encode(&public_key),
        assertion_scheme: certificate.assertion_scheme.clone(),
        assertion_key_algorithm: certificate.assertion_key_algorithm.clone(),
        assertion_public_key_base64: BASE64_STANDARD.encode(&certificate.assertion_public_key),
        assertion_usage_count_limit: certificate.assertion_usage_count_limit,
        assertion_signing_key,
        one_use: true,
        issuer_signature_base64: BASE64_STANDARD.encode(issuer_signature.payload()),
        issuer_signature_payload_base64: BASE64_STANDARD.encode(signing_bytes),
    })
}

fn p256_assertion_signing_key(platform: &str, key_id: &str, device_id: &str) -> P256SigningKey {
    let mut counter = 0_u32;
    loop {
        let mut hasher = Sha256::new();
        hasher.update(b"iroha:offline-v2:assertion-key:p256:v1");
        update_hash_field(&mut hasher, platform);
        update_hash_field(&mut hasher, key_id);
        update_hash_field(&mut hasher, device_id);
        hasher.update(counter.to_be_bytes());
        let secret = hasher.finalize();
        if let Ok(signing_key) = P256SigningKey::from_slice(secret.as_ref()) {
            return signing_key;
        }
        counter = counter
            .checked_add(1)
            .expect("P-256 fixture key derivation counter exhausted");
    }
}

fn update_hash_field(hasher: &mut Sha256, value: &str) {
    let bytes = value.as_bytes();
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn p256_assertion_public_key(signing_key: &P256SigningKey) -> Vec<u8> {
    signing_key
        .verifying_key()
        .to_encoded_point(false)
        .as_bytes()
        .to_vec()
}

fn sign_p256_assertion(signing_key: &P256SigningKey, message: &[u8]) -> Vec<u8> {
    let signature: P256Signature = signing_key.sign(message);
    signature.to_der().as_bytes().to_vec()
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
        (
            "assertion_scheme",
            Value::from(certificate.assertion_scheme.clone()),
        ),
        (
            "assertion_key_algorithm",
            Value::from(certificate.assertion_key_algorithm.clone()),
        ),
        (
            "assertion_public_key",
            Value::from(certificate.assertion_public_key_base64.clone()),
        ),
        (
            "assertion_usage_count_limit",
            certificate
                .assertion_usage_count_limit
                .map(u64::from)
                .map_or(Value::Null, Value::from),
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
) -> Value {
    object(vec![
        ("note_commitment", Value::from(note_commitment)),
        ("key_certificate", mobile_certificate_json(certificate)),
        ("account_id", Value::from(account_id)),
        ("asset_definition_id", Value::from(asset_definition_id)),
        ("amount", Value::from(amount)),
    ])
}

fn mobile_input_claim_json(claim: &OfflineNoteIssuedClaim) -> Result<Value, Box<dyn Error>> {
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

fn payment_token_json(fields: PaymentTokenJsonFields<'_>) -> Value {
    object(vec![
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
    ])
}

fn fountain_qr_fixture(payload: &[u8]) -> Result<Value, Box<dyn Error>> {
    let options = QrStreamOptions {
        chunk_size: 360,
        parity_group: 3,
        payload_kind: QrPayloadKind::OfflinePaymentToken,
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
        ("frame_prefix", Value::from("iroha:qr1:")),
        (
            "payload_sha256_hex",
            Value::from(encode(Sha256::digest(payload))),
        ),
        (
            "frame_size_bytes",
            Value::from(u64::from(options.chunk_size)),
        ),
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

fn checked_ed25519_public_key_payload<'a>(
    key_pair: &'a KeyPair,
    context: &str,
) -> Result<&'a [u8], Box<dyn Error>> {
    let (algorithm, public_key) = key_pair
        .public_key()
        .try_to_bytes()
        .map_err(|err| format!("{context} is malformed: {err}"))?;
    if algorithm != Algorithm::Ed25519 {
        return Err(format!(
            "{context} must be Ed25519, got {}",
            algorithm.as_static_str()
        )
        .into());
    }
    Ok(public_key)
}

fn public_key_base64(key_pair: &KeyPair) -> Result<String, Box<dyn Error>> {
    Ok(BASE64_STANDARD.encode(checked_ed25519_public_key_payload(
        key_pair,
        "offline FI public key",
    )?))
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
    use p256::ecdsa::{VerifyingKey as P256VerifyingKey, signature::Verifier as _};

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

    fn assertion_verifying_key(certificate: &Value) -> P256VerifyingKey {
        let public_key = BASE64_STANDARD
            .decode(string(field(certificate, "assertion_public_key")))
            .expect("decode assertion public key");
        assert_eq!(public_key.len(), 65);
        assert_eq!(public_key[0], 0x04);
        P256VerifyingKey::from_sec1_bytes(&public_key).expect("valid SEC1 P-256 assertion key")
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
        assert_eq!(string(field(fountain, "frame_prefix")), "iroha:qr1:");
        assert!(number(field(fountain, "required_unique_frames")) > 0);
        assert!(
            array(field(fountain, "frames")).len()
                >= number(field(fountain, "required_unique_frames")) as usize
        );

        let chain = field(&fixture, "chain_vectors");
        assert_eq!(
            string(field(proof, "public_inputs_hash_hex")),
            string(field(field(chain, "audit"), "public_inputs_hash"))
        );
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

    #[test]
    fn committed_interop_fixture_uses_valid_p256_assertions() {
        let committed = fs::read_to_string(FIXTURE_PATH).expect("read committed fixture");
        let fixture: Value = json::from_str(&committed).expect("parse committed fixture");
        let token = field(&fixture, "payment_token");
        let sender_certificate = field(token, "sender_key_certificate");
        let recipient_certificate = field(token, "recipient_key_certificate");

        assert_eq!(
            string(field(sender_certificate, "assertion_key_algorithm")),
            "app-attest-p256"
        );
        assert_eq!(
            string(field(recipient_certificate, "assertion_key_algorithm")),
            "app-attest-p256"
        );
        let sender_key = assertion_verifying_key(sender_certificate);
        let _recipient_key = assertion_verifying_key(recipient_certificate);

        let assertion = field(token, "one_use_assertion");
        let signature_bytes = BASE64_STANDARD
            .decode(string(field(assertion, "assertion_base64")))
            .expect("decode one-use assertion");
        let signature =
            P256Signature::from_der(&signature_bytes).expect("valid DER P-256 assertion");
        sender_key
            .verify(
                string(field(assertion, "challenge_hash_hex")).as_bytes(),
                &signature,
            )
            .expect("assertion verifies against sender assertion key");
    }

    #[test]
    fn android_keymint_certificate_uses_valid_p256_assertion_key() {
        let issuer_key_pair =
            fixed_ed25519_keypair("issuer", 0x11).expect("fixed issuer key derives");
        let note_key_pair =
            fixed_ed25519_keypair("android note", 0x41).expect("fixed note key derives");
        let account_id = AccountId::new(note_key_pair.public_key().clone());
        let certificate = signed_certificate(
            &issuer_key_pair,
            &note_key_pair,
            &account_id,
            "android-keymint",
            "android-key-v2-1",
            "android-device-v2-1",
        )
        .expect("android certificate");

        assert_eq!(
            certificate.model.assertion_scheme,
            "android-keymint-ecdsa-p256-usage-limit-v1"
        );
        assert_eq!(
            certificate.model.assertion_key_algorithm,
            "ecdsa-p256-sha256"
        );
        assert_eq!(certificate.model.assertion_usage_count_limit, Some(1));
        assert_eq!(certificate.model.assertion_public_key.len(), 65);
        assert_eq!(certificate.model.assertion_public_key[0], 0x04);

        let verifying_key =
            P256VerifyingKey::from_sec1_bytes(&certificate.model.assertion_public_key)
                .expect("valid Android assertion key");
        let challenge = b"android-p256-assertion";
        let signature_bytes = sign_p256_assertion(&certificate.assertion_signing_key, challenge);
        let signature =
            P256Signature::from_der(&signature_bytes).expect("valid DER Android assertion");
        verifying_key
            .verify(challenge, &signature)
            .expect("Android assertion verifies against certificate key");
    }

    #[test]
    fn fixed_ed25519_keypair_uses_checked_seed_derivation() {
        let keypair =
            fixed_ed25519_keypair("issuer", 0x11).expect("fixed issuer Ed25519 key derives");

        assert_eq!(
            keypair
                .public_key()
                .try_algorithm()
                .expect("fixed public key algorithm"),
            Algorithm::Ed25519
        );
    }
}
