use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{body::Bytes, http::HeaderMap, response::Response as AxResponse};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use iroha_config::parameters::actual;
use iroha_crypto::{Hash, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, IssueOfflineNoteV2, RedeemOfflineNoteV2},
    offline::{
        OFFLINE_NOTE_KEY_CERTIFICATE_VERSION, OfflineNoteIssue, OfflineNoteKeyCertificate,
        OfflineNoteRecursiveProof, OfflineNoteRedeem,
    },
    proof::{ProofBox, VerifyingKeyId},
    transaction::TransactionBuilder,
};
use iroha_primitives::numeric::Numeric;
use norito::json::{self, Map, Value};
use sha2::{Digest as _, Sha256};

use crate::{AppState, Error, SharedAppState, app_auth, json_ok, routing};

const ENDPOINT_KEYS_REFILL: &str = "v1/offline/v2/keys/refill";
const ENDPOINT_NOTES_ISSUE: &str = "v1/offline/v2/notes/issue";
const ENDPOINT_NOTES_REDEEM: &str = "v1/offline/v2/notes/redeem";
const ENDPOINT_AUDIT: &str = "v1/offline/v2/audit";
const PATH_KEYS_REFILL: &str = "/v1/offline/v2/keys/refill";
const PATH_NOTES_ISSUE: &str = "/v1/offline/v2/notes/issue";
const PATH_NOTES_REDEEM: &str = "/v1/offline/v2/notes/redeem";
const PATH_AUDIT: &str = "/v1/offline/v2/audit";

#[derive(Debug, Clone)]
pub(crate) struct OfflineV2IssuerRuntime {
    authority: AccountId,
    key_pair: KeyPair,
    attestation_verifier_public_key: PublicKey,
    max_balance: Numeric,
    max_tx_value: Numeric,
    certificate_ttl: Duration,
    authorization_refresh: Duration,
    authorization_ttl: Duration,
}

impl OfflineV2IssuerRuntime {
    pub(crate) fn from_config(config: actual::ToriiOfflineIssuer) -> Self {
        Self {
            authority: config.authority,
            key_pair: config.key_pair,
            attestation_verifier_public_key: config.attestation_verifier_public_key,
            max_balance: config.max_balance,
            max_tx_value: config.max_tx_value,
            certificate_ttl: config.certificate_ttl,
            authorization_refresh: config.authorization_refresh,
            authorization_ttl: config.authorization_ttl,
        }
    }

    fn sign_bytes(&self, payload: &[u8], context: &'static str) -> Result<Signature, Error> {
        Signature::try_new(self.key_pair.private_key(), payload)
            .map_err(|source| offline_v2_signing_error(context, source))
    }

    fn sign_json_base64(&self, payload: &Value, context: &'static str) -> Result<String, Error> {
        let bytes = json::to_vec(payload)
            .map_err(|source| Error::SerializationFailure { context, source })?;
        Ok(BASE64_STANDARD.encode(self.sign_bytes(&bytes, context)?.payload()))
    }
}

struct ParsedOfflineRequest {
    value: Value,
    account_id: AccountId,
    account_literal: String,
    operation_id: String,
    device_id: String,
    offline_public_key: String,
    asset_definition_id: AssetDefinitionId,
    asset_definition_literal: String,
    device_binding: Value,
}

struct VerifiedDeviceAttestation {
    platform: String,
    key_id: String,
    public_key: Vec<u8>,
    public_key_base64: String,
    assertion_scheme: String,
    assertion_key_algorithm: String,
    assertion_public_key: Vec<u8>,
    assertion_public_key_base64: String,
}

struct VerifiedLineageState {
    balance: Numeric,
    revision: u64,
}

enum LineageKeyPolicy {
    MatchRequest,
    PreserveSignedState,
}

pub(crate) async fn handle_key_refill(
    app: SharedAppState,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let parsed = parse_and_authorize(
        app.as_ref(),
        method,
        uri,
        headers,
        body.as_ref(),
        ENDPOINT_KEYS_REFILL,
    )?;
    let now_ms = now_ms();
    let attestation = verify_device_attestation(&issuer, &parsed, now_ms)?;
    let existing_lineage_id = optional_string(&parsed.value, "existing_lineage_id")
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty());
    let lineage_state = existing_lineage_id
        .as_deref()
        .map(|lineage_id| verify_existing_lineage_state(&issuer, &parsed, lineage_id, now_ms))
        .transpose()?;
    let lineage_id = existing_lineage_id.unwrap_or_else(|| {
        offline_v2_identifier(
            "lineage",
            &format!(
                "{}:{}:{}",
                parsed.account_literal, parsed.device_id, parsed.offline_public_key
            ),
        )
    });
    let certificate = build_key_certificate(&issuer, &parsed, &attestation, now_ms)?;
    let balance = lineage_state
        .as_ref()
        .map(|state| state.balance.to_string())
        .unwrap_or_else(|| "0".to_string());
    let revision = lineage_state
        .as_ref()
        .map(|state| state.revision)
        .unwrap_or(0)
        .checked_add(if lineage_state.is_some() { 1 } else { 0 })
        .ok_or_else(|| {
            validation(
                "OFFLINE_V2_LINEAGE_REVISION_OVERFLOW",
                "Offline Notes V2 lineage revision overflowed.",
            )
        })?;
    let lineage_state = build_lineage_state(
        &issuer,
        &parsed,
        &lineage_id,
        &balance,
        "0",
        revision,
        now_ms,
        Some(certificate.clone()),
    )?;

    json_ok(json_object(vec![
        ("operation_id", string_value(parsed.operation_id)),
        ("lineage_state", lineage_state),
        ("key_certificate", certificate.clone()),
        ("key_certificates", Value::Array(vec![certificate])),
    ]))
}

pub(crate) async fn handle_notes_issue(
    app: SharedAppState,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let parsed = parse_and_authorize(
        app.as_ref(),
        method,
        uri,
        headers,
        body.as_ref(),
        ENDPOINT_NOTES_ISSUE,
    )?;
    let lineage_id = required_string(&parsed.value, "lineage_id")?;
    let amount = parse_positive_amount(required_string(&parsed.value, "amount")?, "amount")?;
    if amount > issuer.max_tx_value.clone() {
        return Err(validation(
            "OFFLINE_AMOUNT_EXCEEDS_LIMIT",
            "Offline note amount exceeds issuer policy.",
        ));
    }
    let now_ms = now_ms();
    let attestation = verify_device_attestation(&issuer, &parsed, now_ms)?;
    let lineage_state = verify_lineage_state(&issuer, &parsed, lineage_id, now_ms)?;
    let pre_balance = lineage_state.balance;
    if let Some(local_balance) = optional_string(&parsed.value, "local_balance") {
        let local_balance = parse_amount(local_balance, "local_balance")?;
        if local_balance != pre_balance {
            return Err(validation(
                "OFFLINE_V2_LINEAGE_BALANCE_MISMATCH",
                "Offline Notes V2 local_balance does not match signed lineage state.",
            ));
        }
    }
    let post_balance = pre_balance
        .clone()
        .checked_add(amount.clone())
        .ok_or_else(|| {
            validation(
                "OFFLINE_BALANCE_OVERFLOW",
                "Offline note balance overflowed issuer policy arithmetic.",
            )
        })?;
    if post_balance > issuer.max_balance.clone() {
        return Err(validation(
            "OFFLINE_BALANCE_EXCEEDS_LIMIT",
            "Offline note balance exceeds issuer policy.",
        ));
    }
    let note_commitment = parse_hash_field(&parsed.value, "note_commitment")?;

    if let Some(local_revision) = parsed.value.get("local_revision").and_then(Value::as_u64)
        && local_revision != lineage_state.revision
    {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_REVISION_MISMATCH",
            "Offline Notes V2 local_revision does not match signed lineage state.",
        ));
    }
    let local_revision = lineage_state.revision.checked_add(1).ok_or_else(|| {
        validation(
            "OFFLINE_V2_LINEAGE_REVISION_OVERFLOW",
            "Offline Notes V2 lineage revision overflowed.",
        )
    })?;
    let entry_hash = settlement_entry_hash(
        &parsed.operation_id,
        lineage_id,
        &parsed.account_literal,
        &parsed.device_id,
        &parsed.offline_public_key,
        &parsed.asset_definition_literal,
        &amount.to_string(),
        &pre_balance.to_string(),
        &post_balance.to_string(),
        local_revision,
    )?;
    let certificate = build_key_certificate(&issuer, &parsed, &attestation, now_ms)?;
    let chain_certificate = build_chain_certificate(&issuer, &parsed, &attestation)?;
    let issue = IssueOfflineNoteV2::new(OfflineNoteIssue {
        note_commitment: note_commitment.clone(),
        key_certificate: chain_certificate,
        asset: AssetId::new(
            parsed.asset_definition_id.clone(),
            parsed.account_id.clone(),
        ),
        amount: amount.clone(),
    });
    let tx = TransactionBuilder::new((*app.chain_id).clone(), issuer.authority.clone().into())
        .with_instructions([InstructionBox::from(issue)])
        .sign(issuer.key_pair.private_key());
    let tx_hash = tx.hash().to_string();
    routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_NOTES_ISSUE,
    )
    .await?;

    let settlement = build_settlement(
        &issuer,
        &parsed,
        "load",
        &pre_balance.to_string(),
        &post_balance.to_string(),
        amount.to_string(),
        &entry_hash,
        &tx_hash,
        now_ms,
    )?;
    let lineage_state = build_lineage_state(
        &issuer,
        &parsed,
        lineage_id,
        &post_balance.to_string(),
        "0",
        local_revision,
        now_ms,
        Some(certificate.clone()),
    )?;

    json_ok(json_object(vec![
        ("operation_id", string_value(parsed.operation_id)),
        ("settlement", settlement),
        ("lineage_state", lineage_state),
        ("local_balance", string_value(post_balance.to_string())),
        ("locked_balance", string_value("0")),
        ("local_revision", number_value(local_revision)),
        (
            "local_state_hash",
            string_value(lineage_state_hash(
                &parsed.account_literal,
                lineage_id,
                &parsed.device_id,
                &parsed.offline_public_key,
                &parsed.asset_definition_literal,
                &post_balance.to_string(),
                "0",
                local_revision,
            )?),
        ),
        (
            "issued_note_commitment",
            string_value(note_commitment.to_string()),
        ),
        ("key_certificate", certificate.clone()),
        ("key_certificates", Value::Array(vec![certificate])),
    ]))
}

pub(crate) async fn handle_notes_redeem(
    app: SharedAppState,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let parsed = parse_and_authorize(
        app.as_ref(),
        method,
        uri,
        headers,
        body.as_ref(),
        ENDPOINT_NOTES_REDEEM,
    )?;
    let redemption = parse_redemption(&parsed)?;
    if redemption.amount > issuer.max_tx_value.clone() {
        return Err(validation(
            "OFFLINE_AMOUNT_EXCEEDS_LIMIT",
            "Offline note amount exceeds issuer policy.",
        ));
    }
    let public_inputs_hash = redemption.public_inputs_hash().map_err(|source| {
        validation_owned(
            "OFFLINE_V2_REDEMPTION_INVALID",
            format!("failed to encode Offline Notes V2 redemption public inputs: {source}"),
        )
    })?;
    if redemption.recursive_proof.public_inputs_hash != public_inputs_hash {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_PROOF_BINDING",
            "Offline Notes V2 redemption proof is not bound to redemption public inputs.",
        ));
    }
    let source_note_commitment = redemption.source_note_commitment.to_string();
    let input_nullifiers = redemption
        .input_nullifiers
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let amount = redemption.amount.to_string();

    let instruction = RedeemOfflineNoteV2::new(redemption);
    let tx = TransactionBuilder::new((*app.chain_id).clone(), issuer.authority.clone().into())
        .with_instructions([InstructionBox::from(instruction)])
        .sign(issuer.key_pair.private_key());
    let tx_hash = tx.hash().to_string();
    routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry.clone(),
        PATH_NOTES_REDEEM,
    )
    .await?;

    let now_ms = now_ms();
    let settlement = build_redeem_settlement(
        &issuer,
        &parsed,
        &amount,
        &source_note_commitment,
        &input_nullifiers,
        &public_inputs_hash.to_string(),
        &tx_hash,
        now_ms,
    )?;

    json_ok(json_object(vec![
        ("operation_id", string_value(parsed.operation_id)),
        ("settlement", settlement),
        ("chain_tx_hash", string_value(tx_hash)),
        (
            "source_note_commitment",
            string_value(source_note_commitment),
        ),
        (
            "input_nullifiers",
            Value::Array(input_nullifiers.into_iter().map(string_value).collect()),
        ),
        (
            "public_inputs_hash",
            string_value(public_inputs_hash.to_string()),
        ),
    ]))
}

pub(crate) async fn handle_audit(
    app: SharedAppState,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let _issuer = require_issuer(&app)?;
    let parsed = parse_and_authorize(
        app.as_ref(),
        method,
        uri,
        headers,
        body.as_ref(),
        ENDPOINT_AUDIT,
    )?;
    let accepted_receipt_ids = parsed
        .value
        .get("receipts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    optional_string(item, "id").or_else(|| optional_string(item, "receipt_id"))
                })
                .map(string_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json_ok(json_object(vec![
        ("operation_id", string_value(parsed.operation_id)),
        ("accepted_receipt_ids", Value::Array(accepted_receipt_ids)),
    ]))
}

fn require_issuer(app: &AppState) -> Result<Arc<OfflineV2IssuerRuntime>, Error> {
    app.offline_v2_issuer
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "OFFLINE_V2_ISSUER_DISABLED",
            message: "Offline Notes V2 issuer is not configured on this Torii node.".to_string(),
        })
}

fn offline_v2_signing_error(context: &'static str, source: iroha_crypto::Error) -> Error {
    Error::Query(ValidationFail::InternalError(format!(
        "Offline Notes V2 issuer failed to sign {context}: {source}"
    )))
}

fn parse_and_authorize(
    app: &AppState,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: &[u8],
    endpoint: &'static str,
) -> Result<ParsedOfflineRequest, Error> {
    reject_legacy_auth_headers(headers)?;
    let value: Value = json::from_slice(body).map_err(|err| {
        validation_owned(
            "OFFLINE_V2_INVALID_JSON",
            format!("Offline Notes V2 request body is not valid JSON: {err}"),
        )
    })?;
    let (body_auth, unsigned_body) = extract_body_auth(&value)?;
    let account_literal = required_string(&value, "account_id")?.to_string();
    let (account_id, canonical_account) = routing::parse_account_literal_with_state(
        &app.state,
        &account_literal,
        &app.telemetry,
        endpoint,
    )
    .map_err(|err| {
        validation_owned(
            "OFFLINE_V2_INVALID_ACCOUNT",
            format!("Invalid Offline Notes V2 account_id: {}", err.reason()),
        )
    })?;
    app_auth::verify_canonical_body_request(
        &app.state,
        body_auth,
        method,
        uri,
        &unsigned_body,
        Some(&account_id),
    )
    .map_err(|err| Error::AppForbidden {
        code: "OFFLINE_V2_SIGNATURE_INVALID",
        message: app_auth_error_message(err),
    })?;

    let device_id = required_string(&value, "device_id")?.to_string();
    if let Some(header_device_id) = headers
        .get("X-Device-Id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if header_device_id != device_id {
            return Err(validation(
                "OFFLINE_V2_DEVICE_MISMATCH",
                "Offline Notes V2 device_id does not match X-Device-Id.",
            ));
        }
    }
    let device_binding = value.get("device_binding").cloned().ok_or_else(|| {
        validation(
            "OFFLINE_V2_DEVICE_BINDING_REQUIRED",
            "device_binding is required.",
        )
    })?;
    if optional_string(&device_binding, "device_id")
        .is_some_and(|binding_device| binding_device != device_id)
    {
        return Err(validation(
            "OFFLINE_V2_DEVICE_BINDING_MISMATCH",
            "device_binding.device_id does not match device_id.",
        ));
    }
    let offline_public_key = required_string(&value, "offline_public_key")
        .or_else(|_| required_string(&device_binding, "offline_public_key"))?
        .to_string();
    if optional_string(&device_binding, "offline_public_key")
        .is_some_and(|binding_key| binding_key != offline_public_key)
    {
        return Err(validation(
            "OFFLINE_V2_DEVICE_BINDING_KEY_MISMATCH",
            "device_binding.offline_public_key does not match offline_public_key.",
        ));
    }
    let asset_literal = required_string(&value, "asset_definition_id")?.to_string();
    let world = app.state.world_view();
    let now = routing::asset_alias_observation_time_ms(app.state.as_ref());
    let asset_definition_id =
        routing::resolve_asset_definition_selector(&world, &asset_literal, now).map_err(|_| {
            validation_owned(
                "OFFLINE_V2_INVALID_ASSET",
                format!("Unknown or invalid asset_definition_id `{asset_literal}`."),
            )
        })?;
    let operation_id = required_string(&value, "operation_id")?.to_string();

    Ok(ParsedOfflineRequest {
        value,
        account_id,
        account_literal: canonical_account,
        operation_id,
        device_id,
        offline_public_key,
        asset_definition_id,
        asset_definition_literal: asset_literal,
        device_binding,
    })
}

fn reject_legacy_auth_headers(headers: &HeaderMap) -> Result<(), Error> {
    for name in [
        app_auth::HEADER_ACCOUNT,
        app_auth::HEADER_SIGNATURE,
        app_auth::HEADER_TIMESTAMP_MS,
        app_auth::HEADER_NONCE,
        app_auth::HEADER_WITNESS,
    ] {
        if headers.contains_key(name) {
            return Err(Error::AppForbidden {
                code: "OFFLINE_V2_HEADER_AUTH_REJECTED",
                message: "Offline Notes V2 issuer requests must put account_id, timestamp_ms, nonce, and signature_base64 or witness_base64 in the JSON body; X-Iroha canonical auth headers are not accepted.".to_string(),
            });
        }
    }
    Ok(())
}

fn extract_body_auth(
    value: &Value,
) -> Result<(app_auth::CanonicalRequestBodyAuth<'_>, Vec<u8>), Error> {
    let account_id = required_string(value, "account_id")?;
    let timestamp_ms = required_u64_with_code(
        value,
        "timestamp_ms",
        "OFFLINE_V2_SIGNATURE_REQUIRED",
    )?;
    let nonce = required_string(value, "nonce")?;
    let signature_base64 = optional_string(value, "signature_base64");
    let witness_base64 = optional_string(value, "witness_base64");
    let proof = match (signature_base64, witness_base64) {
        (Some(signature), None) => app_auth::CanonicalRequestBodyProof::SignatureBase64(signature),
        (None, Some(witness)) => app_auth::CanonicalRequestBodyProof::WitnessBase64(witness),
        (None, None) => {
            return Err(Error::AppForbidden {
                code: "OFFLINE_V2_SIGNATURE_REQUIRED",
                message: "Offline Notes V2 issuer requests require exactly one body proof field: signature_base64 or witness_base64.".to_string(),
            });
        }
        (Some(_), Some(_)) => {
            return Err(Error::AppForbidden {
                code: "OFFLINE_V2_SIGNATURE_INVALID",
                message: "Offline Notes V2 issuer requests must not include both signature_base64 and witness_base64.".to_string(),
            });
        }
    };
    let mut unsigned = value.clone();
    let Value::Object(map) = &mut unsigned else {
        return Err(validation(
            "OFFLINE_V2_INVALID_JSON",
            "Offline Notes V2 request body must be a JSON object.",
        ));
    };
    map.remove("signature_base64");
    map.remove("witness_base64");
    let unsigned_body = json::to_vec(&unsigned).map_err(|source| Error::SerializationFailure {
        context: "offline_v2_body_auth_unsigned_json",
        source,
    })?;

    Ok((
        app_auth::CanonicalRequestBodyAuth {
            account_id,
            timestamp_ms,
            nonce,
            proof,
        },
        unsigned_body,
    ))
}

fn app_auth_error_message(error: Error) -> String {
    match error {
        Error::Query(ValidationFail::NotPermitted(message)) => message,
        other => other.to_string(),
    }
}

fn verify_device_attestation(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    now_ms: u64,
) -> Result<VerifiedDeviceAttestation, Error> {
    let receipt = request
        .device_binding
        .get("attestation_receipt")
        .ok_or_else(|| {
            validation(
                "OFFLINE_V2_ATTESTATION_RECEIPT_REQUIRED",
                "device_binding.attestation_receipt is required.",
            )
        })?;
    let receipt_object = value_object_ref(receipt, "OFFLINE_V2_INVALID_ATTESTATION_RECEIPT")?;
    let signature = required_string(receipt, "signature_base64")?;
    let mut unsigned_object = receipt_object.clone();
    unsigned_object.remove("signature_base64");
    let unsigned = Value::Object(unsigned_object);
    verify_json_signature(
        &issuer.attestation_verifier_public_key,
        &unsigned,
        signature,
        "offline_v2_attestation_receipt",
        "OFFLINE_V2_ATTESTATION_RECEIPT_INVALID",
        "Offline Notes V2 attestation receipt signature is invalid.",
    )?;

    let version = required_u64(receipt, "version")?;
    if version != 1 {
        return Err(validation(
            "OFFLINE_V2_ATTESTATION_RECEIPT_INVALID",
            "Offline Notes V2 attestation receipt version is unsupported.",
        ));
    }
    if required_string(receipt, "account_id")? != request.account_literal {
        return Err(validation(
            "OFFLINE_V2_ATTESTATION_ACCOUNT_MISMATCH",
            "Offline Notes V2 attestation receipt account_id does not match request account_id.",
        ));
    }
    if required_string(receipt, "device_id")? != request.device_id {
        return Err(validation(
            "OFFLINE_V2_ATTESTATION_DEVICE_MISMATCH",
            "Offline Notes V2 attestation receipt device_id does not match request device_id.",
        ));
    }
    if !required_bool(receipt, "hardware_one_use")? {
        return Err(validation(
            "OFFLINE_V2_ATTESTATION_NOT_ONE_USE",
            "Offline Notes V2 attestation receipt does not certify hardware one-use semantics.",
        ));
    }
    let issued_at = required_u64(receipt, "issued_at_ms")?;
    let expires_at = required_u64(receipt, "expires_at_ms")?;
    if issued_at > now_ms || expires_at <= now_ms || issued_at >= expires_at {
        return Err(validation(
            "OFFLINE_V2_ATTESTATION_RECEIPT_EXPIRED",
            "Offline Notes V2 attestation receipt is not currently valid.",
        ));
    }

    let request_public_key = decode_note_public_key(&request.offline_public_key)?;
    let public_key_base64 = required_string(receipt, "offline_public_key_base64")?;
    let public_key = decode_canonical_base64(
        public_key_base64,
        "offline_public_key_base64",
        "OFFLINE_V2_INVALID_NOTE_PUBLIC_KEY",
    )?;
    if public_key.len() != 32 || public_key != request_public_key {
        return Err(validation(
            "OFFLINE_V2_ATTESTATION_KEY_MISMATCH",
            "Offline Notes V2 attestation receipt note key does not match request offline_public_key.",
        ));
    }

    let assertion_public_key_base64 = required_string(receipt, "assertion_public_key_base64")?;
    let assertion_public_key = decode_canonical_base64(
        assertion_public_key_base64,
        "assertion_public_key_base64",
        "OFFLINE_V2_INVALID_ASSERTION_PUBLIC_KEY",
    )?;
    if assertion_public_key.is_empty() {
        return Err(validation(
            "OFFLINE_V2_INVALID_ASSERTION_PUBLIC_KEY",
            "Offline Notes V2 assertion public key must not be empty.",
        ));
    }
    verify_optional_assertion_public_key(request, &assertion_public_key)?;

    let attestation_report_hash = required_string(receipt, "attestation_report_hash_hex")?;
    let report_hash_bytes = hex::decode(attestation_report_hash).map_err(|_| {
        validation(
            "OFFLINE_V2_INVALID_ATTESTATION_REPORT_HASH",
            "Offline Notes V2 attestation_report_hash_hex must be hex.",
        )
    })?;
    if report_hash_bytes.len() != Hash::LENGTH {
        return Err(validation(
            "OFFLINE_V2_INVALID_ATTESTATION_REPORT_HASH",
            "Offline Notes V2 attestation_report_hash_hex must encode 32 bytes.",
        ));
    }
    if let Some(report) = optional_string(&request.device_binding, "attestation_report_base64") {
        let report_bytes = decode_base64_material(report).ok_or_else(|| {
            validation(
                "OFFLINE_V2_INVALID_ATTESTATION_REPORT",
                "Offline Notes V2 attestation_report_base64 must be base64.",
            )
        })?;
        if !sha256_hex(&report_bytes).eq_ignore_ascii_case(attestation_report_hash) {
            return Err(validation(
                "OFFLINE_V2_ATTESTATION_REPORT_MISMATCH",
                "Offline Notes V2 attestation report hash does not match receipt.",
            ));
        }
    }

    Ok(VerifiedDeviceAttestation {
        platform: required_string(receipt, "platform")?.to_string(),
        key_id: required_string(receipt, "attestation_key_id")?.to_string(),
        public_key,
        public_key_base64: BASE64_STANDARD.encode(&request_public_key),
        assertion_scheme: required_string(receipt, "assertion_scheme")?.to_string(),
        assertion_key_algorithm: required_string(receipt, "assertion_key_algorithm")?.to_string(),
        assertion_public_key_base64: BASE64_STANDARD.encode(&assertion_public_key),
        assertion_public_key,
    })
}

fn verify_lineage_state(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    expected_lineage_id: &str,
    now_ms: u64,
) -> Result<VerifiedLineageState, Error> {
    verify_lineage_state_with_key_policy(
        issuer,
        request,
        expected_lineage_id,
        now_ms,
        LineageKeyPolicy::MatchRequest,
    )
}

fn verify_existing_lineage_state(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    expected_lineage_id: &str,
    now_ms: u64,
) -> Result<VerifiedLineageState, Error> {
    verify_lineage_state_with_key_policy(
        issuer,
        request,
        expected_lineage_id,
        now_ms,
        LineageKeyPolicy::PreserveSignedState,
    )
}

fn verify_lineage_state_with_key_policy(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    expected_lineage_id: &str,
    now_ms: u64,
    key_policy: LineageKeyPolicy,
) -> Result<VerifiedLineageState, Error> {
    let state = request.value.get("lineage_state").ok_or_else(|| {
        validation(
            "OFFLINE_V2_LINEAGE_STATE_REQUIRED",
            "Signed Offline Notes V2 lineage_state is required.",
        )
    })?;
    value_object_ref(state, "OFFLINE_V2_INVALID_LINEAGE_STATE")?;

    let lineage_id = required_string(state, "lineage_id")?;
    if lineage_id != expected_lineage_id {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_MISMATCH",
            "Offline Notes V2 lineage_state.lineage_id does not match lineage_id.",
        ));
    }
    if required_string(state, "account_id")? != request.account_literal {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_ACCOUNT_MISMATCH",
            "Offline Notes V2 lineage_state.account_id does not match account_id.",
        ));
    }
    if required_string(state, "device_id")? != request.device_id {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_DEVICE_MISMATCH",
            "Offline Notes V2 lineage_state.device_id does not match device_id.",
        ));
    }
    let state_offline_public_key = required_string(state, "offline_public_key")?;
    if matches!(key_policy, LineageKeyPolicy::MatchRequest)
        && state_offline_public_key != request.offline_public_key
    {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_KEY_MISMATCH",
            "Offline Notes V2 lineage_state.offline_public_key does not match offline_public_key.",
        ));
    }
    if required_string(state, "asset_definition_id")? != request.asset_definition_literal {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_ASSET_MISMATCH",
            "Offline Notes V2 lineage_state.asset_definition_id does not match asset_definition_id.",
        ));
    }

    let balance = parse_amount(required_string(state, "balance")?, "lineage_state.balance")?;
    let locked_balance = parse_amount(
        required_string(state, "locked_balance")?,
        "lineage_state.locked_balance",
    )?;
    if locked_balance != Numeric::zero() {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_LOCKED_BALANCE_UNSUPPORTED",
            "Offline Notes V2 issuer does not accept non-zero locked_balance.",
        ));
    }
    let revision = required_u64(state, "server_revision")?;
    if required_u64(state, "pending_local_revision")? != revision {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_REVISION_MISMATCH",
            "Offline Notes V2 lineage_state revision fields do not match.",
        ));
    }
    let expected_hash = lineage_state_hash(
        &request.account_literal,
        lineage_id,
        &request.device_id,
        state_offline_public_key,
        &request.asset_definition_literal,
        &balance.to_string(),
        &locked_balance.to_string(),
        revision,
    )?;
    if required_string(state, "server_state_hash")? != expected_hash {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_STATE_HASH_MISMATCH",
            "Offline Notes V2 lineage_state hash is invalid.",
        ));
    }

    let authorization = state.get("authorization").ok_or_else(|| {
        validation(
            "OFFLINE_V2_LINEAGE_AUTHORIZATION_REQUIRED",
            "Offline Notes V2 lineage_state.authorization is required.",
        )
    })?;
    value_object_ref(authorization, "OFFLINE_V2_INVALID_LINEAGE_AUTHORIZATION")?;
    let authorization_id = required_string(authorization, "authorization_id")?;
    if required_string(authorization, "account_id")? != request.account_literal
        || required_string(authorization, "lineage_id")? != lineage_id
    {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_AUTHORIZATION_MISMATCH",
            "Offline Notes V2 lineage authorization does not match lineage state.",
        ));
    }
    if required_string(authorization, "max_balance")? != issuer.max_balance.to_string()
        || required_string(authorization, "max_tx_value")? != issuer.max_tx_value.to_string()
    {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_AUTHORIZATION_POLICY_MISMATCH",
            "Offline Notes V2 lineage authorization no longer matches issuer policy.",
        ));
    }
    let auth_issued_at = required_u64(authorization, "issued_at_ms")?;
    let auth_refresh_at = required_u64(authorization, "refresh_at_ms")?;
    let auth_expires_at = required_u64(authorization, "expires_at_ms")?;
    if auth_issued_at > now_ms || auth_expires_at <= now_ms || auth_issued_at >= auth_expires_at {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_AUTHORIZATION_EXPIRED",
            "Offline Notes V2 lineage authorization is not currently valid.",
        ));
    }
    let auth_device_binding = authorization
        .get("device_binding")
        .cloned()
        .ok_or_else(|| {
            validation(
                "OFFLINE_V2_LINEAGE_AUTHORIZATION_DEVICE_BINDING_REQUIRED",
                "Offline Notes V2 lineage authorization device_binding is required.",
            )
        })?;
    if optional_string(&auth_device_binding, "device_id")
        .is_some_and(|device_id| device_id != request.device_id)
        || optional_string(&auth_device_binding, "offline_public_key")
            .is_some_and(|key| key != state_offline_public_key)
    {
        return Err(validation(
            "OFFLINE_V2_LINEAGE_AUTHORIZATION_DEVICE_MISMATCH",
            "Offline Notes V2 lineage authorization device binding does not match request.",
        ));
    }
    let auth_unsigned = authorization_unsigned_payload(
        &request.account_literal,
        authorization_id,
        lineage_id,
        required_string(authorization, "verdict_id")?,
        &issuer.max_balance.to_string(),
        &issuer.max_tx_value.to_string(),
        auth_issued_at,
        auth_refresh_at,
        auth_expires_at,
        auth_device_binding,
    );
    verify_json_signature(
        issuer.key_pair.public_key(),
        &auth_unsigned,
        required_string(authorization, "issuer_signature_base64")?,
        "offline_v2_authorization",
        "OFFLINE_V2_LINEAGE_AUTHORIZATION_SIGNATURE_INVALID",
        "Offline Notes V2 lineage authorization signature is invalid.",
    )?;

    let state_unsigned = lineage_state_unsigned_payload(
        &request.account_literal,
        lineage_id,
        &request.device_id,
        state_offline_public_key,
        &request.asset_definition_literal,
        &balance.to_string(),
        &locked_balance.to_string(),
        revision,
        authorization_id,
    )?;
    verify_json_signature(
        issuer.key_pair.public_key(),
        &state_unsigned,
        required_string(state, "issuer_signature_base64")?,
        "offline_v2_lineage_state",
        "OFFLINE_V2_LINEAGE_STATE_SIGNATURE_INVALID",
        "Offline Notes V2 lineage state signature is invalid.",
    )?;

    Ok(VerifiedLineageState { balance, revision })
}

fn build_lineage_state(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    lineage_id: &str,
    balance: &str,
    locked_balance: &str,
    revision: u64,
    now_ms: u64,
    key_certificate: Option<Value>,
) -> Result<Value, Error> {
    let authorization =
        build_authorization(issuer, request, lineage_id, now_ms, key_certificate.clone())?;
    let unsigned = lineage_state_unsigned_payload(
        &request.account_literal,
        lineage_id,
        &request.device_id,
        &request.offline_public_key,
        &request.asset_definition_literal,
        balance,
        locked_balance,
        revision,
        authorization
            .get("authorization_id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )?;
    let state_hash = lineage_state_hash(
        &request.account_literal,
        lineage_id,
        &request.device_id,
        &request.offline_public_key,
        &request.asset_definition_literal,
        balance,
        locked_balance,
        revision,
    )?;
    let signature = issuer.sign_json_base64(&unsigned, "offline_v2_lineage_state")?;
    Ok(json_object(vec![
        ("lineage_id", string_value(lineage_id)),
        ("account_id", string_value(&request.account_literal)),
        ("device_id", string_value(&request.device_id)),
        (
            "offline_public_key",
            string_value(&request.offline_public_key),
        ),
        (
            "asset_definition_id",
            string_value(&request.asset_definition_literal),
        ),
        ("balance", string_value(balance)),
        ("locked_balance", string_value(locked_balance)),
        ("server_revision", number_value(revision)),
        ("server_state_hash", string_value(state_hash)),
        ("pending_local_revision", number_value(revision)),
        ("authorization", authorization),
        ("issuer_signature_base64", string_value(signature)),
    ]))
}

fn build_authorization(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    lineage_id: &str,
    now_ms: u64,
    key_certificate: Option<Value>,
) -> Result<Value, Error> {
    let authorization_id = offline_v2_identifier(
        "auth",
        &format!(
            "{}:{}:{}",
            request.account_literal, lineage_id, request.device_id
        ),
    );
    let verdict_id = offline_v2_identifier(
        "verdict",
        &format!(
            "{}:{}:{}",
            request.account_literal, request.device_id, now_ms
        ),
    );
    let refresh_at = now_ms.saturating_add(duration_ms(issuer.authorization_refresh));
    let expires_at = now_ms.saturating_add(duration_ms(issuer.authorization_ttl));
    let unsigned = authorization_unsigned_payload(
        &request.account_literal,
        &authorization_id,
        lineage_id,
        &verdict_id,
        &issuer.max_balance.to_string(),
        &issuer.max_tx_value.to_string(),
        now_ms,
        refresh_at,
        expires_at,
        request.device_binding.clone(),
    );
    let signature = issuer.sign_json_base64(&unsigned, "offline_v2_authorization")?;
    let mut entries = vec![
        ("authorization_id", string_value(&authorization_id)),
        ("lineage_id", string_value(lineage_id)),
        ("account_id", string_value(&request.account_literal)),
        ("verdict_id", string_value(&verdict_id)),
        ("max_balance", string_value(issuer.max_balance.to_string())),
        (
            "max_tx_value",
            string_value(issuer.max_tx_value.to_string()),
        ),
        ("issued_at_ms", number_value(now_ms)),
        ("refresh_at_ms", number_value(refresh_at)),
        ("expires_at_ms", number_value(expires_at)),
        ("device_binding", request.device_binding.clone()),
    ];
    if let Some(certificate) = key_certificate {
        entries.push(("key_certificate", certificate));
    }
    entries.push(("issuer_signature_base64", string_value(signature)));
    Ok(json_object(entries))
}

fn build_key_certificate(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    attestation: &VerifiedDeviceAttestation,
    now_ms: u64,
) -> Result<Value, Error> {
    let chain = build_chain_certificate(issuer, request, attestation)?;
    let signing_bytes = chain
        .signing_bytes()
        .map_err(|source| Error::SerializationFailure {
            context: "offline_v2_key_certificate_payload",
            source: source.into(),
        })?;
    let signature = chain.issuer_signature.payload();
    let expires_at = now_ms.saturating_add(duration_ms(issuer.certificate_ttl));
    let usage_limit = assertion_usage_limit(request)?;
    Ok(json_object(vec![
        (
            "version",
            number_value(u64::from(OFFLINE_NOTE_KEY_CERTIFICATE_VERSION)),
        ),
        ("platform", string_value(&attestation.platform)),
        ("key_id", string_value(&attestation.key_id)),
        ("device_id", string_value(&request.device_id)),
        ("account_id", string_value(&request.account_literal)),
        ("public_key", string_value(&attestation.public_key_base64)),
        (
            "assertion_scheme",
            string_value(&attestation.assertion_scheme),
        ),
        (
            "assertion_key_algorithm",
            string_value(&attestation.assertion_key_algorithm),
        ),
        (
            "assertion_public_key",
            string_value(&attestation.assertion_public_key_base64),
        ),
        (
            "assertion_usage_count_limit",
            usage_limit
                .map(|value| number_value(u64::from(value)))
                .unwrap_or(Value::Null),
        ),
        ("one_use", Value::Bool(true)),
        ("issued_at_ms", number_value(now_ms)),
        ("expires_at_ms", number_value(expires_at)),
        (
            "app_attest_public_key_base64",
            string_value(&attestation.assertion_public_key_base64),
        ),
        (
            "ios_team_id",
            optional_string(&request.device_binding, "ios_team_id")
                .map(string_value)
                .unwrap_or(Value::Null),
        ),
        (
            "ios_bundle_id",
            optional_string(&request.device_binding, "ios_bundle_id")
                .map(string_value)
                .unwrap_or(Value::Null),
        ),
        (
            "ios_environment",
            optional_string(&request.device_binding, "ios_environment")
                .map(string_value)
                .unwrap_or(Value::Null),
        ),
        (
            "issuer_signature_base64",
            string_value(BASE64_STANDARD.encode(signature)),
        ),
        (
            "issuer_signature_payload_base64",
            string_value(BASE64_STANDARD.encode(signing_bytes)),
        ),
    ]))
}

fn build_chain_certificate(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    attestation: &VerifiedDeviceAttestation,
) -> Result<OfflineNoteKeyCertificate, Error> {
    let usage_limit = assertion_usage_limit(request)?;
    let mut certificate = OfflineNoteKeyCertificate {
        version: OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
        platform: attestation.platform.clone(),
        key_id: attestation.key_id.clone(),
        device_id: request.device_id.clone(),
        account_id: request.account_id.clone(),
        public_key: attestation.public_key.clone(),
        assertion_scheme: attestation.assertion_scheme.clone(),
        assertion_key_algorithm: attestation.assertion_key_algorithm.clone(),
        assertion_public_key: attestation.assertion_public_key.clone(),
        assertion_usage_count_limit: usage_limit,
        one_use: true,
        issuer_signature: Signature::from_bytes(&[0_u8; 64]),
    };
    let signing_bytes =
        certificate
            .signing_bytes()
            .map_err(|source| Error::SerializationFailure {
                context: "offline_v2_key_certificate_payload",
                source: source.into(),
            })?;
    certificate.issuer_signature =
        issuer.sign_bytes(&signing_bytes, "offline_v2_key_certificate")?;
    Ok(certificate)
}

fn parse_redemption(request: &ParsedOfflineRequest) -> Result<OfflineNoteRedeem, Error> {
    let value = request.value.get("redemption").ok_or_else(|| {
        validation(
            "OFFLINE_REDEMPTION_PROOF_REQUIRED",
            "Offline Notes V2 redemption requires a recursive proof payload.",
        )
    })?;
    let redemption = if let Some(encoded) = optional_string(value, "norito_base64") {
        let bytes = decode_canonical_base64(
            encoded,
            "redemption.norito_base64",
            "OFFLINE_V2_REDEMPTION_INVALID",
        )?;
        norito::decode_from_bytes::<OfflineNoteRedeem>(&bytes).map_err(|err| {
            validation_owned(
                "OFFLINE_V2_REDEMPTION_INVALID",
                format!("Offline Notes V2 redemption.norito_base64 is not canonical Norito: {err}"),
            )
        })?
    } else {
        parse_redemption_object(value, request)?
    };
    validate_redemption_matches_request(&redemption, request)?;
    Ok(redemption)
}

fn parse_redemption_object(
    value: &Value,
    request: &ParsedOfflineRequest,
) -> Result<OfflineNoteRedeem, Error> {
    value_object_ref(value, "OFFLINE_V2_REDEMPTION_INVALID")?;
    let source_note_commitment = parse_hash_field(value, "source_note_commitment")?;
    let input_nullifiers = required_string_array(value, "input_nullifiers")?
        .into_iter()
        .map(|raw| parse_hash_literal(raw, "input_nullifiers"))
        .collect::<Result<Vec<_>, _>>()?;
    let certificate = value
        .get("sender_key_certificate")
        .or_else(|| value.get("key_certificate"))
        .ok_or_else(|| {
            validation(
                "OFFLINE_V2_REDEMPTION_INVALID",
                "Offline Notes V2 redemption.sender_key_certificate is required.",
            )
        })?;
    let sender_key_certificate = parse_key_certificate(certificate)?;
    let amount = parse_positive_amount(required_string(value, "amount")?, "redemption.amount")?;
    let recursive_proof =
        parse_recursive_proof(value.get("recursive_proof").ok_or_else(|| {
            validation(
                "OFFLINE_V2_REDEMPTION_INVALID",
                "Offline Notes V2 redemption.recursive_proof is required.",
            )
        })?)?;
    Ok(OfflineNoteRedeem {
        source_note_commitment,
        input_nullifiers,
        sender_key_certificate,
        recipient: request.account_id.clone(),
        asset: AssetId::new(
            request.asset_definition_id.clone(),
            request.account_id.clone(),
        ),
        amount,
        recursive_proof,
    })
}

fn validate_redemption_matches_request(
    redemption: &OfflineNoteRedeem,
    request: &ParsedOfflineRequest,
) -> Result<(), Error> {
    validate_redemption_certificate(&redemption.sender_key_certificate)?;
    if redemption.recipient != request.account_id {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_ACCOUNT_MISMATCH",
            "Offline Notes V2 redemption recipient does not match the authenticated account.",
        ));
    }
    if redemption.asset.account() != &request.account_id
        || redemption.asset.definition() != &request.asset_definition_id
    {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_ASSET_MISMATCH",
            "Offline Notes V2 redemption asset does not match the request account and asset definition.",
        ));
    }
    if redemption.input_nullifiers.is_empty() || redemption.input_nullifiers.len() > 4 {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 redemption requires 1 to 4 input nullifiers.",
        ));
    }
    if redemption.amount <= Numeric::zero() {
        return Err(validation(
            "OFFLINE_V2_INVALID_AMOUNT",
            "Offline Notes V2 redemption amount must be greater than zero.",
        ));
    }
    Ok(())
}

fn validate_redemption_certificate(certificate: &OfflineNoteKeyCertificate) -> Result<(), Error> {
    if certificate.version != OFFLINE_NOTE_KEY_CERTIFICATE_VERSION {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate model version is not chain-admissible.",
        ));
    }
    if !certificate.one_use {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate must be one-use.",
        ));
    }
    if certificate
        .assertion_usage_count_limit
        .is_some_and(|limit| limit != 1)
    {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate hardware usage limit must be one when present.",
        ));
    }
    Ok(())
}

fn parse_key_certificate(value: &Value) -> Result<OfflineNoteKeyCertificate, Error> {
    value_object_ref(value, "OFFLINE_V2_REDEMPTION_INVALID")?;
    let version = required_u64(value, "version")?;
    let version = u16::try_from(version).map_err(|_| {
        validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate version exceeds u16.",
        )
    })?;
    if version != 2 && version != OFFLINE_NOTE_KEY_CERTIFICATE_VERSION {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate version is unsupported.",
        ));
    }
    let account_literal = required_string(value, "account_id")?;
    let parsed_account = AccountId::parse_encoded(account_literal).map_err(|err| {
        validation_owned(
            "OFFLINE_V2_REDEMPTION_INVALID",
            format!("Offline Notes V2 key certificate account_id is invalid: {err}"),
        )
    })?;
    let account_id = parsed_account.account_id().clone();
    let assertion_usage_count_limit = match value.get("assertion_usage_count_limit") {
        None | Some(Value::Null) => None,
        Some(raw) => {
            let value = raw.as_u64().ok_or_else(|| {
                validation(
                    "OFFLINE_V2_REDEMPTION_INVALID",
                    "Offline Notes V2 assertion_usage_count_limit must be null or an unsigned integer.",
                )
            })?;
            Some(u32::try_from(value).map_err(|_| {
                validation(
                    "OFFLINE_V2_REDEMPTION_INVALID",
                    "Offline Notes V2 assertion_usage_count_limit exceeds u32.",
                )
            })?)
        }
    };
    let issuer_signature = decode_signature_base64(
        required_string(value, "issuer_signature_base64")?,
        "OFFLINE_V2_REDEMPTION_INVALID",
        "Offline Notes V2 issuer_signature_base64 is invalid.",
    )?;
    let certificate = OfflineNoteKeyCertificate {
        version: OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
        platform: required_string(value, "platform")?.to_string(),
        key_id: required_string(value, "key_id")?.to_string(),
        device_id: required_string(value, "device_id")?.to_string(),
        account_id,
        public_key: decode_canonical_base64(
            required_string(value, "public_key")?,
            "public_key",
            "OFFLINE_V2_REDEMPTION_INVALID",
        )?,
        assertion_scheme: required_string(value, "assertion_scheme")?.to_string(),
        assertion_key_algorithm: required_string(value, "assertion_key_algorithm")?.to_string(),
        assertion_public_key: decode_canonical_base64(
            required_string(value, "assertion_public_key")?,
            "assertion_public_key",
            "OFFLINE_V2_REDEMPTION_INVALID",
        )?,
        assertion_usage_count_limit,
        one_use: required_bool(value, "one_use")?,
        issuer_signature,
    };
    if certificate.public_key.len() != 32 {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate public_key must encode 32 bytes.",
        ));
    }
    if certificate.assertion_public_key.is_empty() {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 key certificate assertion_public_key must not be empty.",
        ));
    }
    Ok(certificate)
}

fn parse_recursive_proof(value: &Value) -> Result<OfflineNoteRecursiveProof, Error> {
    value_object_ref(value, "OFFLINE_V2_REDEMPTION_INVALID")?;
    let backend = optional_string(value, "backend").unwrap_or("halo2/ipa");
    let verifier_key_id = VerifyingKeyId::new(
        backend,
        required_string(value, "verifier_key_id")
            .or_else(|_| required_string(value, "verifier_key_name"))?,
    );
    let public_inputs_hash = parse_hash_field(value, "public_inputs_hash_hex")
        .or_else(|_| parse_hash_field(value, "public_inputs_hash"))?;
    let proof_bytes = decode_canonical_base64(
        required_string(value, "proof_bytes_base64")?,
        "proof_bytes_base64",
        "OFFLINE_V2_REDEMPTION_INVALID",
    )?;
    if proof_bytes.is_empty() {
        return Err(validation(
            "OFFLINE_V2_REDEMPTION_INVALID",
            "Offline Notes V2 recursive proof bytes must not be empty.",
        ));
    }
    Ok(OfflineNoteRecursiveProof {
        verifier_key_id,
        public_inputs_hash,
        proof: ProofBox::new(backend.to_string(), proof_bytes),
    })
}

fn parse_hash_field(value: &Value, field: &'static str) -> Result<Hash, Error> {
    parse_hash_literal(required_string(value, field)?, field)
}

fn parse_hash_literal(raw: &str, field: &'static str) -> Result<Hash, Error> {
    Hash::from_str(raw).map_err(|err| {
        validation_owned(
            "OFFLINE_V2_REDEMPTION_INVALID",
            format!("Offline Notes V2 {field} must be a 32-byte hash hex string: {err}"),
        )
    })
}

fn required_string_array<'a>(value: &'a Value, field: &'static str) -> Result<Vec<&'a str>, Error> {
    let items = value.get(field).and_then(Value::as_array).ok_or_else(|| {
        validation_owned(
            "OFFLINE_V2_REDEMPTION_INVALID",
            format!("Offline Notes V2 field `{field}` must be a string array."),
        )
    })?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .ok_or_else(|| {
                    validation_owned(
                        "OFFLINE_V2_REDEMPTION_INVALID",
                        format!(
                            "Offline Notes V2 field `{field}` must contain only non-empty strings."
                        ),
                    )
                })
        })
        .collect()
}

fn decode_signature_base64(
    raw: &str,
    code: &'static str,
    message: &'static str,
) -> Result<Signature, Error> {
    let bytes = decode_canonical_base64(raw, "signature_base64", code)?;
    if bytes.len() != 64 {
        return Err(validation(code, message));
    }
    Ok(Signature::from_bytes(&bytes))
}

fn build_settlement(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    kind: &str,
    pre_balance: &str,
    post_balance: &str,
    amount: String,
    entry_hash: &str,
    chain_tx_hash: &str,
    now_ms: u64,
) -> Result<Value, Error> {
    let unsigned = json_object(vec![
        ("operation_id", string_value(&request.operation_id)),
        ("kind", string_value(kind)),
        ("account_id", string_value(&request.account_literal)),
        ("device_id", string_value(&request.device_id)),
        (
            "asset_definition_id",
            string_value(&request.asset_definition_literal),
        ),
        ("amount", string_value(amount)),
        ("pre_balance", string_value(pre_balance)),
        ("post_balance", string_value(post_balance)),
        ("entry_hash", string_value(entry_hash)),
        ("chain_tx_hash", string_value(chain_tx_hash)),
        ("block_height", number_value(0)),
        ("issued_at_ms", number_value(now_ms)),
    ]);
    let signature = issuer.sign_json_base64(&unsigned, "offline_v2_settlement")?;
    let mut map = value_object(unsigned)?;
    map.insert(
        "issuer_signature_base64".to_string(),
        string_value(signature),
    );
    Ok(Value::Object(map))
}

fn build_redeem_settlement(
    issuer: &OfflineV2IssuerRuntime,
    request: &ParsedOfflineRequest,
    amount: &str,
    source_note_commitment: &str,
    input_nullifiers: &[String],
    public_inputs_hash: &str,
    chain_tx_hash: &str,
    now_ms: u64,
) -> Result<Value, Error> {
    let unsigned = json_object(vec![
        ("operation_id", string_value(&request.operation_id)),
        ("kind", string_value("redeem")),
        ("account_id", string_value(&request.account_literal)),
        ("device_id", string_value(&request.device_id)),
        (
            "asset_definition_id",
            string_value(&request.asset_definition_literal),
        ),
        (
            "amount",
            string_value(parse_amount(amount, "amount")?.to_string()),
        ),
        (
            "source_note_commitment",
            string_value(source_note_commitment),
        ),
        (
            "input_nullifiers",
            Value::Array(input_nullifiers.iter().cloned().map(string_value).collect()),
        ),
        ("public_inputs_hash", string_value(public_inputs_hash)),
        ("chain_tx_hash", string_value(chain_tx_hash)),
        ("block_height", number_value(0)),
        ("issued_at_ms", number_value(now_ms)),
    ]);
    let signature = issuer.sign_json_base64(&unsigned, "offline_v2_redeem_settlement")?;
    let mut map = value_object(unsigned)?;
    map.insert(
        "issuer_signature_base64".to_string(),
        string_value(signature),
    );
    Ok(Value::Object(map))
}

fn authorization_unsigned_payload(
    account_id: &str,
    authorization_id: &str,
    lineage_id: &str,
    verdict_id: &str,
    max_balance: &str,
    max_tx_value: &str,
    issued_at: u64,
    refresh_at: u64,
    expires_at: u64,
    device_binding: Value,
) -> Value {
    json_object(vec![
        ("account_id", string_value(account_id)),
        ("authorization_id", string_value(authorization_id)),
        ("expires_at_ms", number_value(expires_at)),
        ("issued_at_ms", number_value(issued_at)),
        ("max_balance", string_value(max_balance)),
        ("max_tx_value", string_value(max_tx_value)),
        ("refresh_at_ms", number_value(refresh_at)),
        ("lineage_id", string_value(lineage_id)),
        ("verdict_id", string_value(verdict_id)),
        ("device_binding", device_binding),
    ])
}

fn lineage_state_unsigned_payload(
    account_id: &str,
    lineage_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: &str,
    balance: &str,
    locked_balance: &str,
    revision: u64,
    authorization_id: &str,
) -> Result<Value, Error> {
    Ok(json_object(vec![
        ("account_id", string_value(account_id)),
        ("authorization_id", string_value(authorization_id)),
        ("asset_definition_id", string_value(asset_definition_id)),
        (
            "balance",
            string_value(parse_amount(balance, "balance")?.to_string()),
        ),
        ("device_id", string_value(device_id)),
        ("offline_public_key", string_value(offline_public_key)),
        ("pending_local_revision", number_value(revision)),
        (
            "locked_balance",
            string_value(parse_amount(locked_balance, "locked_balance")?.to_string()),
        ),
        ("lineage_id", string_value(lineage_id)),
        ("server_revision", number_value(revision)),
        (
            "server_state_hash",
            string_value(lineage_state_hash(
                account_id,
                lineage_id,
                device_id,
                offline_public_key,
                asset_definition_id,
                balance,
                locked_balance,
                revision,
            )?),
        ),
    ]))
}

fn lineage_state_hash(
    account_id: &str,
    lineage_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: &str,
    balance: &str,
    locked_balance: &str,
    revision: u64,
) -> Result<String, Error> {
    let payload = json_object(vec![
        ("account_id", string_value(account_id)),
        ("asset_definition_id", string_value(asset_definition_id)),
        (
            "balance",
            string_value(parse_amount(balance, "balance")?.to_string()),
        ),
        ("device_id", string_value(device_id)),
        ("offline_public_key", string_value(offline_public_key)),
        (
            "locked_balance",
            string_value(parse_amount(locked_balance, "locked_balance")?.to_string()),
        ),
        ("lineage_id", string_value(lineage_id)),
        ("server_revision", number_value(revision)),
    ]);
    Ok(sha256_json_hex(&payload, "offline_v2_lineage_state_hash")?)
}

fn settlement_entry_hash(
    operation_id: &str,
    lineage_id: &str,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: &str,
    amount: &str,
    pre_balance: &str,
    post_balance: &str,
    revision: u64,
) -> Result<String, Error> {
    let payload = json_object(vec![
        (
            "domain",
            string_value("pk-retail-wallet-ios:offline-v2:settlement-entry"),
        ),
        ("account_id", string_value(account_id)),
        (
            "amount",
            string_value(parse_amount(amount, "amount")?.to_string()),
        ),
        ("asset_definition_id", string_value(asset_definition_id)),
        ("device_id", string_value(device_id)),
        ("operation_id", string_value(operation_id)),
        ("offline_public_key", string_value(offline_public_key)),
        ("lineage_id", string_value(lineage_id)),
        (
            "pre_balance",
            string_value(parse_amount(pre_balance, "pre_balance")?.to_string()),
        ),
        (
            "post_balance",
            string_value(parse_amount(post_balance, "post_balance")?.to_string()),
        ),
        ("local_revision", number_value(revision)),
    ]);
    sha256_json_hex(&payload, "offline_v2_settlement_entry")
}

fn required_string<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, Error> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            validation_owned(
                "OFFLINE_V2_MISSING_FIELD",
                format!("Offline Notes V2 field `{field}` is required."),
            )
        })
}

fn required_u64(value: &Value, field: &'static str) -> Result<u64, Error> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        validation_owned(
            "OFFLINE_V2_MISSING_FIELD",
            format!("Offline Notes V2 numeric field `{field}` is required."),
        )
    })
}

fn required_u64_with_code(
    value: &Value,
    field: &'static str,
    code: &'static str,
) -> Result<u64, Error> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        validation_owned(
            code,
            format!("Offline Notes V2 numeric field `{field}` is required."),
        )
    })
}

fn required_bool(value: &Value, field: &'static str) -> Result<bool, Error> {
    value.get(field).and_then(Value::as_bool).ok_or_else(|| {
        validation_owned(
            "OFFLINE_V2_MISSING_FIELD",
            format!("Offline Notes V2 boolean field `{field}` is required."),
        )
    })
}

fn optional_string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn parse_amount(raw: &str, field: &'static str) -> Result<Numeric, Error> {
    Numeric::from_str(raw.trim()).map_err(|err| {
        validation_owned(
            "OFFLINE_V2_INVALID_AMOUNT",
            format!("Invalid Offline Notes V2 {field}: {err}"),
        )
    })
}

fn parse_positive_amount(raw: &str, field: &'static str) -> Result<Numeric, Error> {
    let amount = parse_amount(raw, field)?;
    if amount <= Numeric::zero() {
        return Err(validation_owned(
            "OFFLINE_V2_INVALID_AMOUNT",
            format!("Offline Notes V2 {field} must be greater than zero."),
        ));
    }
    Ok(amount)
}

fn assertion_usage_limit(request: &ParsedOfflineRequest) -> Result<Option<u32>, Error> {
    let Some(value) = request.device_binding.get("assertion_usage_count_limit") else {
        return Ok(None);
    };
    let raw = value.as_u64().ok_or_else(|| {
        validation(
            "OFFLINE_V2_INVALID_ASSERTION_USAGE_LIMIT",
            "assertion_usage_count_limit must be an unsigned integer.",
        )
    })?;
    u32::try_from(raw).map(Some).map_err(|_| {
        validation(
            "OFFLINE_V2_INVALID_ASSERTION_USAGE_LIMIT",
            "assertion_usage_count_limit exceeds u32.",
        )
    })
}

fn decode_note_public_key(raw: &str) -> Result<Vec<u8>, Error> {
    let public_key = decode_key_material(raw).ok_or_else(|| {
        validation(
            "OFFLINE_V2_INVALID_NOTE_PUBLIC_KEY",
            "offline_public_key must be hex/base64 encoded key bytes.",
        )
    })?;
    if public_key.len() != 32 {
        return Err(validation(
            "OFFLINE_V2_INVALID_NOTE_PUBLIC_KEY",
            "offline_public_key must encode a 32-byte Ed25519 public key.",
        ));
    }
    Ok(public_key)
}

fn decode_key_material(raw: &str) -> Option<Vec<u8>> {
    let value = raw.trim();
    if value.is_empty() {
        return None;
    }
    if value.len() % 2 == 0 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        if let Ok(bytes) = hex::decode(value) {
            return Some(bytes);
        }
    }
    decode_base64_material(value)
}

fn decode_base64_material(raw: &str) -> Option<Vec<u8>> {
    BASE64_STANDARD
        .decode(raw)
        .ok()
        .or_else(|| URL_SAFE_NO_PAD.decode(raw).ok())
}

fn decode_canonical_base64(
    raw: &str,
    field: &'static str,
    code: &'static str,
) -> Result<Vec<u8>, Error> {
    let bytes = BASE64_STANDARD.decode(raw).map_err(|_| {
        validation_owned(
            code,
            format!("Offline Notes V2 {field} must be standard base64."),
        )
    })?;
    if BASE64_STANDARD.encode(&bytes) != raw {
        return Err(validation_owned(
            code,
            format!("Offline Notes V2 {field} must use canonical standard base64."),
        ));
    }
    Ok(bytes)
}

fn verify_optional_assertion_public_key(
    request: &ParsedOfflineRequest,
    expected: &[u8],
) -> Result<(), Error> {
    for field in [
        "assertion_public_key",
        "app_attest_public_key_base64",
        "device_public_key",
    ] {
        if let Some(raw) = optional_string(&request.device_binding, field) {
            let bytes = decode_key_material(raw)
                .filter(|bytes| !bytes.is_empty())
                .ok_or_else(|| {
                    validation(
                        "OFFLINE_V2_INVALID_ASSERTION_PUBLIC_KEY",
                        "Offline Notes V2 assertion public key must be hex/base64 key bytes.",
                    )
                })?;
            if bytes != expected {
                return Err(validation(
                    "OFFLINE_V2_ASSERTION_PUBLIC_KEY_MISMATCH",
                    "Offline Notes V2 assertion public key does not match attestation receipt.",
                ));
            }
        }
    }
    Ok(())
}

fn json_object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Map>(),
    )
}

fn value_object(value: Value) -> Result<Map, Error> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(Error::SerializationFailure {
            context: "offline_v2_json_object",
            source: json::Error::Message("expected JSON object".to_string()),
        }),
    }
}

fn value_object_ref<'a>(value: &'a Value, code: &'static str) -> Result<&'a Map, Error> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(validation(
            code,
            "Offline Notes V2 field must be a JSON object.",
        )),
    }
}

fn verify_json_signature(
    public_key: &PublicKey,
    payload: &Value,
    signature_base64: &str,
    context: &'static str,
    code: &'static str,
    message: &'static str,
) -> Result<(), Error> {
    let bytes =
        json::to_vec(payload).map_err(|source| Error::SerializationFailure { context, source })?;
    let signature_bytes = BASE64_STANDARD
        .decode(signature_base64)
        .map_err(|_| validation(code, message))?;
    Signature::from_bytes(&signature_bytes)
        .verify(public_key, &bytes)
        .map_err(|_| validation(code, message))
}

fn string_value(value: impl Into<String>) -> Value {
    Value::String(value.into())
}

fn number_value(value: u64) -> Value {
    json::to_value(&value).unwrap_or(Value::Null)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn now_ms() -> u64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn sha256_json_hex(value: &Value, context: &'static str) -> Result<String, Error> {
    let bytes =
        json::to_vec(value).map_err(|source| Error::SerializationFailure { context, source })?;
    Ok(sha256_hex(&bytes))
}

fn offline_v2_identifier(prefix: &str, value: &str) -> String {
    format!("{prefix}-{}", sha256_hex(value.as_bytes()))
}

fn validation(code: &'static str, message: &'static str) -> Error {
    validation_owned(code, message.to_string())
}

fn validation_owned(code: &'static str, message: String) -> Error {
    Error::AppQueryValidation { code, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Algorithm;
    use iroha_data_model::domain::DomainId;

    const NOW_MS: u64 = 1_700_000_000_000;
    const REPORT_BYTES: &[u8] = b"offline-v2-platform-attestation";

    fn sample_issuer() -> (OfflineV2IssuerRuntime, KeyPair) {
        let issuer_key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let verifier_key_pair = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
        let authority = AccountId::new(issuer_key_pair.public_key().clone());
        (
            OfflineV2IssuerRuntime {
                authority,
                key_pair: issuer_key_pair,
                attestation_verifier_public_key: verifier_key_pair.public_key().clone(),
                max_balance: "100".parse().expect("max balance"),
                max_tx_value: "25".parse().expect("max transaction value"),
                certificate_ttl: Duration::from_secs(300),
                authorization_refresh: Duration::from_secs(60),
                authorization_ttl: Duration::from_secs(600),
            },
            verifier_key_pair,
        )
    }

    fn sample_request(
        verifier: &KeyPair,
        note_key: [u8; 32],
        assertion_key: Vec<u8>,
    ) -> ParsedOfflineRequest {
        let account_key_pair = KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519);
        let account_id = AccountId::new(account_key_pair.public_key().clone());
        let account_literal = account_id.to_string();
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset_definition_literal = asset_definition_id.to_string();
        let offline_public_key = hex::encode(note_key);
        let assertion_key_hex = hex::encode(&assertion_key);
        let receipt = signed_attestation_receipt(
            verifier,
            &account_literal,
            "device-1",
            &note_key,
            &assertion_key,
            true,
        );
        let device_binding = json_object(vec![
            ("device_id", string_value("device-1")),
            ("offline_public_key", string_value(&offline_public_key)),
            ("assertion_public_key", string_value(assertion_key_hex)),
            ("assertion_usage_count_limit", number_value(1)),
            (
                "attestation_report_base64",
                string_value(BASE64_STANDARD.encode(REPORT_BYTES)),
            ),
            ("attestation_receipt", receipt),
        ]);
        let value = json_object(vec![
            ("account_id", string_value(&account_literal)),
            ("operation_id", string_value("operation-1")),
            ("device_id", string_value("device-1")),
            ("offline_public_key", string_value(&offline_public_key)),
            (
                "asset_definition_id",
                string_value(&asset_definition_literal),
            ),
            ("device_binding", device_binding.clone()),
        ]);
        ParsedOfflineRequest {
            value,
            account_id,
            account_literal,
            operation_id: "operation-1".to_string(),
            device_id: "device-1".to_string(),
            offline_public_key,
            asset_definition_id,
            asset_definition_literal,
            device_binding,
        }
    }

    fn signed_attestation_receipt(
        verifier: &KeyPair,
        account_id: &str,
        device_id: &str,
        note_key: &[u8],
        assertion_key: &[u8],
        hardware_one_use: bool,
    ) -> Value {
        let unsigned = json_object(vec![
            ("version", number_value(1)),
            ("platform", string_value("ios-app-attest")),
            ("account_id", string_value(account_id)),
            ("device_id", string_value(device_id)),
            (
                "offline_public_key_base64",
                string_value(BASE64_STANDARD.encode(note_key)),
            ),
            (
                "assertion_public_key_base64",
                string_value(BASE64_STANDARD.encode(assertion_key)),
            ),
            ("assertion_scheme", string_value("apple-app-attest-v1")),
            ("assertion_key_algorithm", string_value("ecdsa-p256-sha256")),
            ("attestation_key_id", string_value("attestation-key-1")),
            ("hardware_one_use", Value::Bool(hardware_one_use)),
            (
                "attestation_report_hash_hex",
                string_value(sha256_hex(REPORT_BYTES)),
            ),
            ("issued_at_ms", number_value(NOW_MS - 1_000)),
            ("expires_at_ms", number_value(NOW_MS + 60_000)),
        ]);
        let signature = {
            let bytes = json::to_vec(&unsigned).expect("receipt json");
            Signature::new(verifier.private_key(), &bytes)
        };
        let mut map = value_object(unsigned).expect("receipt object");
        map.insert(
            "signature_base64".to_string(),
            string_value(BASE64_STANDARD.encode(signature.payload())),
        );
        Value::Object(map)
    }

    fn insert_field(value: &mut Value, field: &str, field_value: Value) {
        let Value::Object(map) = value else {
            panic!("expected object");
        };
        map.insert(field.to_string(), field_value);
    }

    fn offline_v2_fixture() -> Value {
        json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/offline/interop_contract_v2.json"
        )))
        .expect("offline v2 fixture parses")
    }

    fn fixture_redeem_request() -> ParsedOfflineRequest {
        let fixture = offline_v2_fixture();
        let token = fixture.get("payment_token").expect("payment token");
        let recipient_certificate = token
            .get("recipient_key_certificate")
            .expect("recipient certificate");
        let account_literal = required_string(token, "recipient_account_id")
            .expect("recipient account")
            .to_string();
        let account_id = AccountId::parse_encoded(&account_literal)
            .expect("fixture account id")
            .account_id()
            .clone();
        let asset_definition_literal = required_string(token, "asset_definition_id")
            .expect("asset definition")
            .to_string();
        let asset_definition_id =
            AssetDefinitionId::from_str(&asset_definition_literal).expect("fixture asset id");
        let device_id = required_string(recipient_certificate, "device_id")
            .expect("device id")
            .to_string();
        let offline_public_key = required_string(recipient_certificate, "public_key")
            .expect("public key")
            .to_string();
        let device_binding = json_object(vec![
            ("device_id", string_value(&device_id)),
            ("offline_public_key", string_value(&offline_public_key)),
        ]);
        let value = json_object(vec![
            ("account_id", string_value(&account_literal)),
            ("operation_id", string_value("fixture-redeem")),
            ("device_id", string_value(&device_id)),
            ("offline_public_key", string_value(&offline_public_key)),
            (
                "asset_definition_id",
                string_value(&asset_definition_literal),
            ),
            ("device_binding", device_binding.clone()),
        ]);
        ParsedOfflineRequest {
            value,
            account_id,
            account_literal,
            operation_id: "fixture-redeem".to_string(),
            device_id,
            offline_public_key,
            asset_definition_id,
            asset_definition_literal,
            device_binding,
        }
    }

    fn fixture_redeem_model() -> OfflineNoteRedeem {
        let fixture = offline_v2_fixture();
        let encoded = required_string(
            fixture
                .get("chain_vectors")
                .and_then(|value| value.get("redeem"))
                .expect("redeem chain vector"),
            "norito_base64",
        )
        .expect("redeem norito");
        let bytes = BASE64_STANDARD
            .decode(encoded)
            .expect("decode redeem vector");
        norito::decode_from_bytes(&bytes).expect("decode redeem model")
    }

    fn chain_admissible_fixture_redeem_model() -> OfflineNoteRedeem {
        let model = fixture_redeem_model();
        assert_eq!(
            model.sender_key_certificate.version,
            OFFLINE_NOTE_KEY_CERTIFICATE_VERSION
        );
        model
    }

    fn certificate_json(certificate: &OfflineNoteKeyCertificate) -> Value {
        json_object(vec![
            ("version", number_value(u64::from(certificate.version))),
            ("platform", string_value(&certificate.platform)),
            ("key_id", string_value(&certificate.key_id)),
            ("device_id", string_value(&certificate.device_id)),
            (
                "account_id",
                string_value(certificate.account_id.to_string()),
            ),
            (
                "public_key",
                string_value(BASE64_STANDARD.encode(&certificate.public_key)),
            ),
            (
                "assertion_scheme",
                string_value(&certificate.assertion_scheme),
            ),
            (
                "assertion_key_algorithm",
                string_value(&certificate.assertion_key_algorithm),
            ),
            (
                "assertion_public_key",
                string_value(BASE64_STANDARD.encode(&certificate.assertion_public_key)),
            ),
            (
                "assertion_usage_count_limit",
                certificate
                    .assertion_usage_count_limit
                    .map(|value| number_value(u64::from(value)))
                    .unwrap_or(Value::Null),
            ),
            ("one_use", Value::Bool(certificate.one_use)),
            (
                "issuer_signature_base64",
                string_value(BASE64_STANDARD.encode(certificate.issuer_signature.payload())),
            ),
        ])
    }

    fn recursive_proof_json(proof: &OfflineNoteRecursiveProof) -> Value {
        json_object(vec![
            ("backend", string_value(&proof.proof.backend)),
            ("verifier_key_id", string_value(&proof.verifier_key_id.name)),
            (
                "public_inputs_hash_hex",
                string_value(proof.public_inputs_hash.to_string()),
            ),
            (
                "proof_bytes_base64",
                string_value(BASE64_STANDARD.encode(&proof.proof.bytes)),
            ),
        ])
    }

    fn redemption_json(redemption: &OfflineNoteRedeem) -> Value {
        json_object(vec![
            (
                "source_note_commitment",
                string_value(redemption.source_note_commitment.to_string()),
            ),
            (
                "input_nullifiers",
                Value::Array(
                    redemption
                        .input_nullifiers
                        .iter()
                        .map(ToString::to_string)
                        .map(string_value)
                        .collect(),
                ),
            ),
            (
                "sender_key_certificate",
                certificate_json(&redemption.sender_key_certificate),
            ),
            ("amount", string_value(redemption.amount.to_string())),
            (
                "recursive_proof",
                recursive_proof_json(&redemption.recursive_proof),
            ),
        ])
    }

    fn validation_code(result: Result<impl Sized, Error>) -> &'static str {
        match result {
            Err(Error::AppQueryValidation { code, .. }) => code,
            Err(error) => panic!("expected validation error, got {error:?}"),
            Ok(_) => panic!("expected validation error"),
        }
    }

    fn app_error_code(result: Result<impl Sized, Error>) -> &'static str {
        match result {
            Err(Error::AppQueryValidation { code, .. } | Error::AppForbidden { code, .. }) => code,
            Err(error) => panic!("expected app error, got {error:?}"),
            Ok(_) => panic!("expected app error"),
        }
    }

    #[test]
    fn redeem_route_accepts_chain_admissible_norito_redemption() {
        let mut request = fixture_redeem_request();
        let model = chain_admissible_fixture_redeem_model();
        let encoded = BASE64_STANDARD.encode(norito::to_bytes(&model).expect("encode redemption"));
        insert_field(
            &mut request.value,
            "redemption",
            json_object(vec![("norito_base64", string_value(&encoded))]),
        );

        let redemption = parse_redemption(&request).expect("redemption parses");

        assert_eq!(redemption.recipient, request.account_id);
        assert_eq!(redemption.asset.account(), &request.account_id);
        assert_eq!(redemption.asset.definition(), &request.asset_definition_id);
        assert_eq!(redemption.input_nullifiers.len(), 1);
        assert_eq!(
            redemption.recursive_proof.public_inputs_hash,
            redemption
                .public_inputs_hash()
                .expect("redemption public inputs hash")
        );
    }

    #[test]
    fn redeem_route_rejects_stale_fixture_certificate_version() {
        let mut request = fixture_redeem_request();
        let mut model = fixture_redeem_model();
        model.sender_key_certificate.version = OFFLINE_NOTE_KEY_CERTIFICATE_VERSION
            .checked_add(1)
            .expect("stale certificate version");
        model.recursive_proof.public_inputs_hash =
            model.public_inputs_hash().expect("public inputs hash");
        let encoded = BASE64_STANDARD.encode(norito::to_bytes(&model).expect("encode redemption"));
        insert_field(
            &mut request.value,
            "redemption",
            json_object(vec![("norito_base64", string_value(&encoded))]),
        );

        assert_eq!(
            validation_code(parse_redemption(&request)),
            "OFFLINE_V2_REDEMPTION_INVALID"
        );
    }

    #[test]
    fn redeem_route_accepts_structured_redemption_json() {
        let mut request = fixture_redeem_request();
        let model = chain_admissible_fixture_redeem_model();
        insert_field(&mut request.value, "redemption", redemption_json(&model));

        let parsed = parse_redemption(&request).expect("structured redemption parses");

        assert_eq!(parsed, model);
    }

    #[test]
    fn redeem_route_accepts_legacy_structured_certificate_json_version_two() {
        let mut request = fixture_redeem_request();
        let model = chain_admissible_fixture_redeem_model();
        let mut redemption = redemption_json(&model);
        let Value::Object(redemption_fields) = &mut redemption else {
            panic!("expected redemption object");
        };
        let certificate = redemption_fields
            .get_mut("sender_key_certificate")
            .expect("sender key certificate");
        insert_field(certificate, "version", number_value(2));
        insert_field(&mut request.value, "redemption", redemption);

        let parsed = parse_redemption(&request).expect("legacy structured redemption parses");

        assert_eq!(parsed, model);
        assert_eq!(
            parsed.sender_key_certificate.version,
            OFFLINE_NOTE_KEY_CERTIFICATE_VERSION
        );
    }

    #[test]
    fn redeem_route_rejects_redemption_for_different_authenticated_account() {
        let mut request = fixture_redeem_request();
        request.account_id = AccountId::new(
            KeyPair::from_seed(vec![0x44; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let model = chain_admissible_fixture_redeem_model();
        let encoded = BASE64_STANDARD.encode(norito::to_bytes(&model).expect("encode redemption"));
        insert_field(
            &mut request.value,
            "redemption",
            json_object(vec![("norito_base64", string_value(&encoded))]),
        );

        assert_eq!(
            validation_code(parse_redemption(&request)),
            "OFFLINE_V2_REDEMPTION_ACCOUNT_MISMATCH"
        );
    }

    #[test]
    fn verified_attestation_canonicalizes_certificate_key_bytes() {
        let (issuer, verifier) = sample_issuer();
        let note_key = [0xA5; 32];
        let assertion_key = vec![0xB6; 65];
        let request = sample_request(&verifier, note_key, assertion_key.clone());

        let attestation =
            verify_device_attestation(&issuer, &request, NOW_MS).expect("valid attestation");
        assert_eq!(attestation.public_key, note_key);
        assert_eq!(
            attestation.public_key_base64,
            BASE64_STANDARD.encode(note_key)
        );
        assert_eq!(attestation.assertion_public_key, assertion_key);
        assert_eq!(
            attestation.assertion_public_key_base64,
            BASE64_STANDARD.encode(&assertion_key)
        );

        let certificate =
            build_key_certificate(&issuer, &request, &attestation, NOW_MS).expect("certificate");
        assert_eq!(
            certificate.get("version").and_then(Value::as_u64),
            Some(u64::from(OFFLINE_NOTE_KEY_CERTIFICATE_VERSION))
        );
        assert_eq!(
            optional_string(&certificate, "public_key"),
            Some(BASE64_STANDARD.encode(note_key).as_str())
        );
        assert_eq!(
            optional_string(&certificate, "assertion_public_key"),
            Some(BASE64_STANDARD.encode(&assertion_key).as_str())
        );
        assert_eq!(
            certificate.get("one_use").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn attestation_receipt_is_required_before_one_use_certification() {
        let (issuer, verifier) = sample_issuer();
        let mut request = sample_request(&verifier, [0xA5; 32], vec![0xB6; 65]);
        let Value::Object(binding) = &mut request.device_binding else {
            panic!("expected binding object");
        };
        binding.remove("attestation_receipt");

        assert_eq!(
            validation_code(verify_device_attestation(&issuer, &request, NOW_MS)),
            "OFFLINE_V2_ATTESTATION_RECEIPT_REQUIRED"
        );
    }

    #[test]
    fn malformed_assertion_key_is_rejected_instead_of_falling_back() {
        let (issuer, verifier) = sample_issuer();
        let mut request = sample_request(&verifier, [0xA5; 32], vec![0xB6; 65]);
        insert_field(
            &mut request.device_binding,
            "assertion_public_key",
            string_value("#"),
        );

        assert_eq!(
            validation_code(verify_device_attestation(&issuer, &request, NOW_MS)),
            "OFFLINE_V2_INVALID_ASSERTION_PUBLIC_KEY"
        );
    }

    #[test]
    fn issue_lineage_state_uses_signed_balance_and_rejects_tampering() {
        let (issuer, verifier) = sample_issuer();
        let mut request = sample_request(&verifier, [0xA5; 32], vec![0xB6; 65]);
        let lineage_id = "lineage-signed-balance";
        let state = build_lineage_state(&issuer, &request, lineage_id, "12", "0", 3, NOW_MS, None)
            .expect("lineage state");
        insert_field(&mut request.value, "lineage_id", string_value(lineage_id));
        insert_field(&mut request.value, "lineage_state", state);

        let verified =
            verify_lineage_state(&issuer, &request, lineage_id, NOW_MS).expect("signed state");
        assert_eq!(verified.balance.to_string(), "12");
        assert_eq!(verified.revision, 3);

        let state = request
            .value
            .get_mut("lineage_state")
            .expect("lineage state");
        insert_field(state, "balance", string_value("0"));
        assert_eq!(
            validation_code(verify_lineage_state(&issuer, &request, lineage_id, NOW_MS)),
            "OFFLINE_V2_LINEAGE_STATE_HASH_MISMATCH"
        );
    }

    #[test]
    fn refill_existing_lineage_accepts_signed_old_key_state() {
        let (issuer, verifier) = sample_issuer();
        let old_request = sample_request(&verifier, [0xA5; 32], vec![0xB6; 65]);
        let lineage_id = "lineage-rekey";
        let state = build_lineage_state(
            &issuer,
            &old_request,
            lineage_id,
            "20",
            "0",
            4,
            NOW_MS,
            None,
        )
        .expect("lineage state");
        let mut new_request = sample_request(&verifier, [0xC7; 32], vec![0xD8; 65]);
        insert_field(&mut new_request.value, "lineage_state", state);

        assert_eq!(
            validation_code(verify_lineage_state(
                &issuer,
                &new_request,
                lineage_id,
                NOW_MS
            )),
            "OFFLINE_V2_LINEAGE_KEY_MISMATCH"
        );
        let verified = verify_existing_lineage_state(&issuer, &new_request, lineage_id, NOW_MS)
            .expect("existing lineage state");
        assert_eq!(verified.balance.to_string(), "20");
        assert_eq!(verified.revision, 4);
    }

    #[test]
    fn body_auth_rejects_legacy_iroha_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(
            app_auth::HEADER_SIGNATURE,
            axum::http::HeaderValue::from_static("legacy-signature"),
        );

        assert_eq!(
            app_error_code(reject_legacy_auth_headers(&headers)),
            "OFFLINE_V2_HEADER_AUTH_REJECTED"
        );
    }

    #[test]
    fn body_auth_removes_only_top_level_proof_fields() {
        let value = json_object(vec![
            ("account_id", string_value("account-1")),
            ("timestamp_ms", number_value(NOW_MS)),
            ("nonce", string_value("nonce-1")),
            ("signature_base64", string_value("top-level-signature")),
            (
                "device_binding",
                json_object(vec![
                    ("signature_base64", string_value("nested-signature")),
                    ("witness_base64", string_value("nested-witness")),
                ]),
            ),
        ]);

        let (auth, unsigned_body) = extract_body_auth(&value).expect("body auth");

        assert_eq!(auth.account_id, "account-1");
        assert_eq!(auth.timestamp_ms, NOW_MS);
        assert_eq!(auth.nonce, "nonce-1");
        match auth.proof {
            app_auth::CanonicalRequestBodyProof::SignatureBase64(signature) => {
                assert_eq!(signature, "top-level-signature");
            }
            app_auth::CanonicalRequestBodyProof::WitnessBase64(_) => {
                panic!("expected signature proof")
            }
        }
        let unsigned: Value = json::from_slice(&unsigned_body).expect("unsigned json");
        assert!(unsigned.get("signature_base64").is_none());
        assert!(unsigned.get("witness_base64").is_none());
        let nested = unsigned
            .get("device_binding")
            .expect("device binding")
            .as_object()
            .expect("device binding object");
        assert_eq!(
            nested.get("signature_base64").and_then(Value::as_str),
            Some("nested-signature")
        );
        assert_eq!(
            nested.get("witness_base64").and_then(Value::as_str),
            Some("nested-witness")
        );
    }

    #[test]
    fn body_auth_requires_exactly_one_body_proof() {
        let mut missing = json_object(vec![
            ("account_id", string_value("account-1")),
            ("timestamp_ms", number_value(NOW_MS)),
            ("nonce", string_value("nonce-1")),
        ]);
        assert_eq!(
            app_error_code(extract_body_auth(&missing)),
            "OFFLINE_V2_SIGNATURE_REQUIRED"
        );

        insert_field(
            &mut missing,
            "signature_base64",
            string_value("top-level-signature"),
        );
        insert_field(
            &mut missing,
            "witness_base64",
            string_value("top-level-witness"),
        );
        assert_eq!(
            app_error_code(extract_body_auth(&missing)),
            "OFFLINE_V2_SIGNATURE_INVALID"
        );
    }

    #[test]
    fn issued_note_commitment_uses_wallet_commitment_without_chain_reencoding() {
        let wallet_commitment = Hash::new(b"wallet-derived-note-commitment");
        let entry_hash = settlement_entry_hash(
            "operation-1",
            "lineage-1",
            "account-1",
            "device-1",
            "offline-key-1",
            "usd#offline",
            "5",
            "12",
            "17",
            4,
        )
        .expect("entry hash");
        let chain_commitment = Hash::new(entry_hash.as_bytes()).to_string();
        let request = json_object(vec![(
            "note_commitment",
            string_value(wallet_commitment.to_string()),
        )]);

        assert_eq!(
            parse_hash_field(&request, "note_commitment").expect("note commitment"),
            wallet_commitment
        );
        assert_ne!(wallet_commitment.to_string(), chain_commitment);
    }
}
