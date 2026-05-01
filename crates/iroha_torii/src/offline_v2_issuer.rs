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
use iroha_crypto::{Hash, KeyPair, Signature};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, IssueOfflineNoteV2},
    offline::{OfflineNoteIssueV2, OfflineNoteKeyCertificateV2},
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
            max_balance: config.max_balance,
            max_tx_value: config.max_tx_value,
            certificate_ttl: config.certificate_ttl,
            authorization_refresh: config.authorization_refresh,
            authorization_ttl: config.authorization_ttl,
        }
    }

    fn sign_bytes(&self, payload: &[u8]) -> Signature {
        Signature::new(self.key_pair.private_key(), payload)
    }

    fn sign_json_base64(&self, payload: &Value, context: &'static str) -> Result<String, Error> {
        let bytes = json::to_vec(payload)
            .map_err(|source| Error::SerializationFailure { context, source })?;
        Ok(BASE64_STANDARD.encode(self.sign_bytes(&bytes).payload()))
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
    let lineage_id = optional_string(&parsed.value, "existing_lineage_id")
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            offline_v2_identifier(
                "lineage",
                &format!(
                    "{}:{}:{}",
                    parsed.account_literal, parsed.device_id, parsed.offline_public_key
                ),
            )
        });
    let certificate = build_key_certificate(&issuer, &parsed, now_ms)?;
    let lineage_state = build_lineage_state(
        &issuer,
        &parsed,
        &lineage_id,
        "0",
        "0",
        parsed
            .value
            .get("local_revision")
            .and_then(Value::as_u64)
            .unwrap_or(0),
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
    let pre_balance = parse_amount(
        required_string(&parsed.value, "local_balance")?,
        "local_balance",
    )?;
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

    let now_ms = now_ms();
    let local_revision = parsed
        .value
        .get("local_revision")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .saturating_add(1);
    let request_hash = sha256_hex(body.as_ref());
    let entry_hash = settlement_entry_hash(
        &parsed.operation_id,
        lineage_id,
        &pre_balance.to_string(),
        &post_balance.to_string(),
        local_revision,
        &request_hash,
    )?;
    let certificate = build_key_certificate(&issuer, &parsed, now_ms)?;
    let chain_certificate = build_chain_certificate(&issuer, &parsed, now_ms)?;
    let issue = IssueOfflineNoteV2::new(OfflineNoteIssueV2 {
        note_commitment: Hash::new(entry_hash.as_bytes()),
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
        ("issued_note_commitment", string_value(entry_hash)),
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
    let _issuer = require_issuer(&app)?;
    let parsed = parse_and_authorize(
        app.as_ref(),
        method,
        uri,
        headers,
        body.as_ref(),
        ENDPOINT_NOTES_REDEEM,
    )?;
    if parsed.value.get("redemption").is_none() {
        return Err(validation(
            "OFFLINE_REDEMPTION_PROOF_REQUIRED",
            "Offline Notes V2 redemption requires a recursive proof payload.",
        ));
    }
    Err(validation(
        "OFFLINE_REDEMPTION_TORII_ISSUER_UNAVAILABLE",
        "Offline Notes V2 redemption proof submission is not implemented by this Torii issuer.",
    ))
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

fn parse_and_authorize(
    app: &AppState,
    method: &axum::http::Method,
    uri: &axum::http::Uri,
    headers: &HeaderMap,
    body: &[u8],
    endpoint: &'static str,
) -> Result<ParsedOfflineRequest, Error> {
    let value: Value = json::from_slice(body).map_err(|err| {
        validation_owned(
            "OFFLINE_V2_INVALID_JSON",
            format!("Offline Notes V2 request body is not valid JSON: {err}"),
        )
    })?;
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
    let Some(_) = app_auth::verify_canonical_request(
        &app.state,
        headers,
        method,
        uri,
        body,
        Some(&account_id),
    )
    .map_err(|err| Error::AppForbidden {
        code: "OFFLINE_V2_SIGNATURE_INVALID",
        message: err.to_string(),
    })?
    else {
        return Err(Error::AppForbidden {
            code: "OFFLINE_V2_SIGNATURE_REQUIRED",
            message:
                "Offline Notes V2 issuer requests require X-Iroha canonical request signatures."
                    .to_string(),
        });
    };

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
    now_ms: u64,
) -> Result<Value, Error> {
    let chain = build_chain_certificate(issuer, request, now_ms)?;
    let signing_bytes = chain
        .signing_bytes()
        .map_err(|source| Error::SerializationFailure {
            context: "offline_v2_key_certificate_payload",
            source: source.into(),
        })?;
    let signature = chain.issuer_signature.payload();
    let expires_at = now_ms.saturating_add(duration_ms(issuer.certificate_ttl));
    Ok(json_object(vec![
        ("version", number_value(2)),
        ("platform", string_value(certificate_platform(request))),
        ("key_id", string_value(certificate_key_id(request))),
        ("device_id", string_value(&request.device_id)),
        ("account_id", string_value(&request.account_literal)),
        ("public_key", string_value(&request.offline_public_key)),
        ("assertion_scheme", string_value(assertion_scheme(request))),
        (
            "assertion_key_algorithm",
            string_value(assertion_key_algorithm(request)),
        ),
        (
            "assertion_public_key",
            string_value(assertion_public_key_literal(request)),
        ),
        (
            "assertion_usage_count_limit",
            request
                .device_binding
                .get("assertion_usage_count_limit")
                .cloned()
                .unwrap_or(Value::Null),
        ),
        ("one_use", Value::Bool(true)),
        ("issued_at_ms", number_value(now_ms)),
        ("expires_at_ms", number_value(expires_at)),
        (
            "app_attest_public_key_base64",
            optional_string(&request.device_binding, "app_attest_public_key_base64")
                .or_else(|| optional_string(&request.device_binding, "assertion_public_key"))
                .map(string_value)
                .unwrap_or(Value::Null),
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
    _now_ms: u64,
) -> Result<OfflineNoteKeyCertificateV2, Error> {
    let public_key = decode_key_material(&request.offline_public_key).ok_or_else(|| {
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
    let assertion_public_key = decode_key_material(assertion_public_key_literal(request))
        .filter(|bytes| !bytes.is_empty())
        .unwrap_or_else(|| public_key.clone());
    let usage_limit = request
        .device_binding
        .get("assertion_usage_count_limit")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok());
    let mut certificate = OfflineNoteKeyCertificateV2 {
        version: 2,
        platform: certificate_platform(request).to_string(),
        key_id: certificate_key_id(request).to_string(),
        device_id: request.device_id.clone(),
        account_id: request.account_id.clone(),
        public_key,
        assertion_scheme: assertion_scheme(request).to_string(),
        assertion_key_algorithm: assertion_key_algorithm(request).to_string(),
        assertion_public_key,
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
    certificate.issuer_signature = issuer.sign_bytes(&signing_bytes);
    Ok(certificate)
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
    pre_balance: &str,
    post_balance: &str,
    revision: u64,
    request_hash: &str,
) -> Result<String, Error> {
    let payload = json_object(vec![
        (
            "domain",
            string_value("pk-retail-wallet-ios:offline-v2:settlement-entry"),
        ),
        ("operation_id", string_value(operation_id)),
        ("lineage_id", string_value(lineage_id)),
        ("pre_balance", string_value(pre_balance)),
        ("post_balance", string_value(post_balance)),
        ("local_revision", number_value(revision)),
        ("request_hash", string_value(request_hash)),
    ]);
    sha256_json_hex(&payload, "offline_v2_settlement_entry")
}

fn certificate_platform(request: &ParsedOfflineRequest) -> &str {
    optional_string(&request.device_binding, "platform").unwrap_or("offline-v2")
}

fn certificate_key_id(request: &ParsedOfflineRequest) -> &str {
    optional_string(&request.device_binding, "attestation_key_id")
        .or_else(|| optional_string(&request.device_binding, "assertion_key_id"))
        .or_else(|| optional_string(&request.value, "app_attest_key_id"))
        .unwrap_or(&request.device_id)
}

fn assertion_scheme(request: &ParsedOfflineRequest) -> &str {
    optional_string(&request.device_binding, "assertion_scheme").unwrap_or_else(|| {
        if certificate_platform(request)
            .to_ascii_lowercase()
            .contains("android")
        {
            "android-keymint-v1"
        } else {
            "apple-app-attest-v1"
        }
    })
}

fn assertion_key_algorithm(request: &ParsedOfflineRequest) -> &str {
    optional_string(&request.device_binding, "assertion_key_algorithm").unwrap_or_else(|| {
        if certificate_platform(request)
            .to_ascii_lowercase()
            .contains("android")
        {
            "ed25519"
        } else {
            "ecdsa-p256-sha256"
        }
    })
}

fn assertion_public_key_literal(request: &ParsedOfflineRequest) -> &str {
    optional_string(&request.device_binding, "assertion_public_key")
        .or_else(|| optional_string(&request.device_binding, "app_attest_public_key_base64"))
        .or_else(|| optional_string(&request.device_binding, "device_public_key"))
        .unwrap_or(&request.offline_public_key)
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
    BASE64_STANDARD
        .decode(value)
        .ok()
        .or_else(|| URL_SAFE_NO_PAD.decode(value).ok())
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
