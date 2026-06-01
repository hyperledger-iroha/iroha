use std::{
    str::FromStr,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{body::Bytes, http::HeaderMap, response::Response as AxResponse};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE_NO_PAD},
};
use iroha_config::parameters::actual;
use iroha_core::state::WorldReadOnly;
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::{InstructionBox, IssueOfflineNote, SetKeyValue, Transfer},
    name::Name,
    offline::{OFFLINE_NOTE_KEY_CERTIFICATE_VERSION, OfflineNoteIssue, OfflineNoteKeyCertificate},
    transaction::TransactionBuilder,
};
use iroha_primitives::json::Json;
use iroha_primitives::numeric::Numeric;
use norito::json::{self, Map, Value};
use p256::ecdsa::signature::Verifier as _;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};
use sha2::{Digest as _, Sha256};

use crate::{AppState, Error, SharedAppState, app_auth, json_ok, routing};

const ENDPOINT_KEYS_REFILL: &str = "v1/offline/keys/refill";
const ENDPOINT_NOTES_ISSUE: &str = "v1/offline/notes/issue";
const ENDPOINT_NOTES_REDEEM: &str = "v1/offline/notes/redeem";
const ENDPOINT_AUDIT: &str = "v1/offline/audit";
const PATH_KEYS_REFILL: &str = "/v1/offline/keys/refill";
const PATH_NOTES_ISSUE: &str = "/v1/offline/notes/issue";
const PATH_NOTES_REDEEM: &str = "/v1/offline/notes/redeem";
const PATH_AUDIT: &str = "/v1/offline/audit";
const OFFLINE_REVOCATION_BUNDLE_TTL_MS: u64 = 5 * 60 * 1_000;

#[derive(Debug, Clone)]
pub(crate) struct OfflineIssuerRuntime {
    authority: AccountId,
    key_pair: KeyPair,
    attestation_verifier_public_key: PublicKey,
    max_balance: Numeric,
    max_tx_value: Numeric,
    certificate_ttl: Duration,
    authorization_refresh: Duration,
    authorization_ttl: Duration,
    policy: Arc<RwLock<OfflinePolicyState>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct OfflinePolicyState {
    verdict_ids: Vec<String>,
    blacklisted_account_ids: Vec<String>,
    asset_send_limits: Vec<OfflineAssetSendLimitState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OfflineAssetSendLimitState {
    asset_definition_id: String,
    daily_send_limit: String,
    monthly_send_limit: String,
}

impl OfflineIssuerRuntime {
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
            policy: Arc::new(RwLock::new(OfflinePolicyState::default())),
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
        offline_identifier(
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
                "OFFLINE_LINEAGE_REVISION_OVERFLOW",
                "Offline Notes lineage revision overflowed.",
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
    let note_commitment = required_note_commitment(&parsed.value)?;
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
                "OFFLINE_LINEAGE_BALANCE_MISMATCH",
                "Offline Notes local_balance does not match signed lineage state.",
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

    if let Some(local_revision) = parsed.value.get("local_revision").and_then(Value::as_u64)
        && local_revision != lineage_state.revision
    {
        return Err(validation(
            "OFFLINE_LINEAGE_REVISION_MISMATCH",
            "Offline Notes local_revision does not match signed lineage state.",
        ));
    }
    let local_revision = lineage_state.revision.checked_add(1).ok_or_else(|| {
        validation(
            "OFFLINE_LINEAGE_REVISION_OVERFLOW",
            "Offline Notes lineage revision overflowed.",
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
    let (certificate, chain_certificate) =
        build_key_certificate_bundle(&issuer, &parsed, &attestation, now_ms)?;
    let issue = IssueOfflineNote::new(OfflineNoteIssue {
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
            "Offline Notes redemption requires a recursive proof payload.",
        ));
    }
    Err(validation(
        "OFFLINE_REDEMPTION_TORII_ISSUER_UNAVAILABLE",
        "Offline Notes redemption proof submission is not implemented by this Torii issuer.",
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
    if parsed
        .value
        .get("payment_tokens")
        .and_then(Value::as_array)
        .is_some_and(|tokens| !tokens.is_empty())
    {
        return Err(validation(
            "OFFLINE_AUDIT_TORII_ISSUER_UNAVAILABLE",
            "Offline Notes audit payment-token submission is not implemented by this Torii issuer; route audit mutations through the Core API issuer.",
        ));
    }
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

pub(crate) async fn handle_policy_update(
    app: SharedAppState,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let value: Value = json::from_slice(body.as_ref()).map_err(|err| {
        validation_owned(
            "OFFLINE_INVALID_JSON",
            format!("Offline policy payload is not valid JSON: {err}"),
        )
    })?;
    let policy = parse_policy_snapshot(&value)?;
    replace_policy_state(&issuer, policy.clone())?;
    json_ok(policy_response(policy))
}

pub(crate) async fn handle_revocations_list(app: SharedAppState) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let policy = policy_state_snapshot(&issuer)?;
    json_ok(policy_response(policy))
}

pub(crate) async fn handle_revocation_register(
    app: SharedAppState,
    body: Bytes,
) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let value: Value = json::from_slice(body.as_ref()).map_err(|err| {
        validation_owned(
            "OFFLINE_INVALID_JSON",
            format!("Offline revocation payload is not valid JSON: {err}"),
        )
    })?;
    let mut policy = policy_state_snapshot(&issuer)?;
    let mut changed = false;

    for verdict_id in collect_string_fields(&value, &["verdict_id", "verdict_ids"])? {
        if insert_sorted_unique(&mut policy.verdict_ids, verdict_id) {
            changed = true;
        }
    }
    for account_id in collect_string_fields(
        &value,
        &[
            "account_id",
            "account_ids",
            "blacklisted_account_id",
            "blacklisted_account_ids",
        ],
    )? {
        if insert_sorted_unique(&mut policy.blacklisted_account_ids, account_id) {
            changed = true;
        }
    }
    if let Some(asset_send_limits) = value.get("asset_send_limits") {
        policy.asset_send_limits = parse_asset_send_limits(asset_send_limits)?;
        changed = true;
    }
    if !changed {
        return Err(validation(
            "OFFLINE_REVOCATION_EMPTY",
            "Offline revocation payload must include account_id, verdict_id, or asset_send_limits.",
        ));
    }

    replace_policy_state(&issuer, policy.clone())?;
    json_ok(policy_response(policy))
}

pub(crate) async fn handle_revocation_bundle(app: SharedAppState) -> Result<AxResponse, Error> {
    let issuer = require_issuer(&app)?;
    let bundle = build_revocation_bundle(&issuer, now_ms())?;
    json_ok(bundle)
}

fn verify_ed25519_signature(
    public_key: &[u8],
    payload: &[u8],
    signature: &[u8],
) -> Result<(), &'static str> {
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| "ed25519_public_key_invalid")?;
    let signature: [u8; 64] = signature
        .try_into()
        .map_err(|_| "ed25519_signature_invalid")?;
    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&public_key)
        .map_err(|_| "ed25519_public_key_invalid")?;
    let signature = ed25519_dalek::Signature::from_bytes(&signature);
    verifying_key
        .verify_strict(payload, &signature)
        .map_err(|_| "signature_invalid")
}

fn verify_p256_signature(
    public_key: &[u8],
    payload: &[u8],
    signature: &[u8],
) -> Result<(), &'static str> {
    let verifying_key =
        P256VerifyingKey::from_sec1_bytes(public_key).map_err(|_| "p256_public_key_invalid")?;
    let signature = P256Signature::from_der(signature).map_err(|_| "p256_signature_invalid")?;
    verifying_key
        .verify(payload, &signature)
        .map_err(|_| "signature_invalid")
}

fn rejected_transfer(transfer_id: &str, reason: &'static str) -> Value {
    json_object(vec![
        ("transfer_id", string_value(transfer_id)),
        ("reason", string_value(reason)),
    ])
}

fn require_issuer(app: &AppState) -> Result<Arc<OfflineIssuerRuntime>, Error> {
    app.offline_issuer
        .clone()
        .ok_or_else(|| Error::AppServiceUnavailable {
            code: "OFFLINE_ISSUER_DISABLED",
            message: "Offline Notes issuer is not configured on this Torii node.".to_string(),
        })
}

fn policy_lock_unavailable() -> Error {
    Error::AppServiceUnavailable {
        code: "OFFLINE_POLICY_UNAVAILABLE",
        message: "Offline Notes policy state is unavailable.".to_string(),
    }
}

fn policy_state_snapshot(issuer: &OfflineIssuerRuntime) -> Result<OfflinePolicyState, Error> {
    issuer
        .policy
        .read()
        .map_err(|_| policy_lock_unavailable())
        .map(|policy| policy.clone())
}

fn replace_policy_state(
    issuer: &OfflineIssuerRuntime,
    mut policy: OfflinePolicyState,
) -> Result<(), Error> {
    canonicalize_policy_state(&mut policy);
    let mut guard = issuer
        .policy
        .write()
        .map_err(|_| policy_lock_unavailable())?;
    *guard = policy;
    Ok(())
}

fn canonicalize_policy_state(policy: &mut OfflinePolicyState) {
    policy.verdict_ids.sort();
    policy.verdict_ids.dedup();
    policy.blacklisted_account_ids.sort();
    policy.blacklisted_account_ids.dedup();
    policy
        .asset_send_limits
        .sort_by(|left, right| left.asset_definition_id.cmp(&right.asset_definition_id));
    policy
        .asset_send_limits
        .dedup_by(|left, right| left.asset_definition_id == right.asset_definition_id);
}

fn parse_policy_snapshot(value: &Value) -> Result<OfflinePolicyState, Error> {
    let _ = value_object_ref(value, "OFFLINE_POLICY_INVALID")?;
    Ok(OfflinePolicyState {
        verdict_ids: parse_string_array_field(value, "verdict_ids")?,
        blacklisted_account_ids: parse_string_array_field(value, "blacklisted_account_ids")?,
        asset_send_limits: value
            .get("asset_send_limits")
            .map(parse_asset_send_limits)
            .transpose()?
            .unwrap_or_default(),
    })
}

fn parse_string_array_field(value: &Value, field: &'static str) -> Result<Vec<String>, Error> {
    let Some(field_value) = value.get(field) else {
        return Ok(Vec::new());
    };
    let items = field_value.as_array().ok_or_else(|| {
        validation_owned(
            "OFFLINE_POLICY_INVALID",
            format!("Offline policy field {field} must be an array."),
        )
    })?;
    normalize_string_values(items.iter().map(|item| {
        item.as_str().ok_or_else(|| {
            validation_owned(
                "OFFLINE_POLICY_INVALID",
                format!("Offline policy field {field} must contain only strings."),
            )
        })
    }))
}

fn collect_string_fields(value: &Value, fields: &[&'static str]) -> Result<Vec<String>, Error> {
    let _ = value_object_ref(value, "OFFLINE_REVOCATION_INVALID")?;
    let mut values = Vec::new();
    for field in fields {
        let Some(field_value) = value.get(*field) else {
            continue;
        };
        match field_value {
            Value::String(item) => {
                values.extend(normalize_string_values(std::iter::once(Ok(item.as_str())))?);
            }
            Value::Array(items) => {
                values.extend(normalize_string_values(items.iter().map(|item| {
                    item.as_str().ok_or_else(|| {
                        validation_owned(
                            "OFFLINE_REVOCATION_INVALID",
                            format!("Offline revocation field {field} must contain only strings."),
                        )
                    })
                }))?);
            }
            _ => {
                return Err(validation_owned(
                    "OFFLINE_REVOCATION_INVALID",
                    format!("Offline revocation field {field} must be a string or string array."),
                ));
            }
        }
    }
    values.sort();
    values.dedup();
    Ok(values)
}

#[allow(single_use_lifetimes)]
fn normalize_string_values<T: AsRef<str>>(
    values: impl Iterator<Item = Result<T, Error>>,
) -> Result<Vec<String>, Error> {
    let mut normalized = Vec::new();
    for value in values {
        let value = value?;
        let value = value.as_ref().trim();
        if !value.is_empty() {
            normalized.push(value.to_string());
        }
    }
    normalized.sort();
    normalized.dedup();
    Ok(normalized)
}

fn parse_asset_send_limits(value: &Value) -> Result<Vec<OfflineAssetSendLimitState>, Error> {
    let items = value.as_array().ok_or_else(|| {
        validation(
            "OFFLINE_POLICY_INVALID",
            "Offline policy field asset_send_limits must be an array.",
        )
    })?;
    let mut limits = Vec::with_capacity(items.len());
    for item in items {
        let _ = value_object_ref(item, "OFFLINE_POLICY_INVALID")?;
        let asset_definition_id = required_string(item, "asset_definition_id")?
            .trim()
            .to_string();
        if asset_definition_id.is_empty() {
            return Err(validation(
                "OFFLINE_POLICY_INVALID",
                "Offline policy asset_definition_id must not be empty.",
            ));
        }
        let daily_send_limit = normalize_policy_amount(required_string(item, "daily_send_limit")?)?;
        let monthly_send_limit =
            normalize_policy_amount(required_string(item, "monthly_send_limit")?)?;
        limits.push(OfflineAssetSendLimitState {
            asset_definition_id,
            daily_send_limit,
            monthly_send_limit,
        });
    }
    limits.sort_by(|left, right| left.asset_definition_id.cmp(&right.asset_definition_id));
    limits.dedup_by(|left, right| left.asset_definition_id == right.asset_definition_id);
    Ok(limits)
}

fn normalize_policy_amount(value: &str) -> Result<String, Error> {
    let value = value.trim();
    if value.is_empty() {
        return Err(validation(
            "OFFLINE_POLICY_INVALID",
            "Offline policy amount must not be empty.",
        ));
    }
    let amount = Numeric::from_str(value).map_err(|err| {
        validation_owned(
            "OFFLINE_POLICY_INVALID",
            format!("Offline policy amount is invalid: {err}"),
        )
    })?;
    if amount <= Numeric::from(0_u32) {
        return Err(validation(
            "OFFLINE_POLICY_INVALID",
            "Offline policy amount must be greater than zero.",
        ));
    }
    Ok(amount.to_string())
}

fn insert_sorted_unique(values: &mut Vec<String>, value: String) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    if values.iter().any(|existing| existing == value) {
        return false;
    }
    values.push(value.to_string());
    values.sort();
    true
}

fn asset_send_limit_value(limit: &OfflineAssetSendLimitState) -> Value {
    json_object(vec![
        (
            "asset_definition_id",
            string_value(&limit.asset_definition_id),
        ),
        ("daily_send_limit", string_value(&limit.daily_send_limit)),
        (
            "monthly_send_limit",
            string_value(&limit.monthly_send_limit),
        ),
    ])
}

fn asset_send_limits_value(limits: &[OfflineAssetSendLimitState]) -> Value {
    Value::Array(limits.iter().map(asset_send_limit_value).collect())
}

fn policy_response(policy: OfflinePolicyState) -> Value {
    json_object(vec![
        (
            "verdict_ids",
            Value::Array(policy.verdict_ids.into_iter().map(string_value).collect()),
        ),
        (
            "blacklisted_account_ids",
            Value::Array(
                policy
                    .blacklisted_account_ids
                    .into_iter()
                    .map(string_value)
                    .collect(),
            ),
        ),
        (
            "asset_send_limits",
            asset_send_limits_value(&policy.asset_send_limits),
        ),
    ])
}

fn build_revocation_bundle(issuer: &OfflineIssuerRuntime, now_ms: u64) -> Result<Value, Error> {
    let policy = policy_state_snapshot(issuer)?;
    let unsigned = json_object(vec![
        ("issued_at_ms", number_value(now_ms)),
        (
            "expires_at_ms",
            number_value(now_ms.saturating_add(OFFLINE_REVOCATION_BUNDLE_TTL_MS)),
        ),
        (
            "verdict_ids",
            Value::Array(policy.verdict_ids.into_iter().map(string_value).collect()),
        ),
        (
            "blacklisted_account_ids",
            Value::Array(
                policy
                    .blacklisted_account_ids
                    .into_iter()
                    .map(string_value)
                    .collect(),
            ),
        ),
        (
            "asset_send_limits",
            asset_send_limits_value(&policy.asset_send_limits),
        ),
    ]);
    let signature = issuer.sign_json_base64(&unsigned, "offline_revocation_bundle")?;
    let mut map = value_object(unsigned)?;
    map.insert(
        "issuer_signature_base64".to_string(),
        string_value(signature),
    );
    Ok(Value::Object(map))
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
            "OFFLINE_INVALID_JSON",
            format!("Offline Notes request body is not valid JSON: {err}"),
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
            "OFFLINE_INVALID_ACCOUNT",
            format!("Invalid Offline Notes account_id: {}", err.reason()),
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
        code: "OFFLINE_SIGNATURE_INVALID",
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
                "OFFLINE_DEVICE_MISMATCH",
                "Offline Notes device_id does not match X-Device-Id.",
            ));
        }
    }
    let device_binding = value.get("device_binding").cloned().ok_or_else(|| {
        validation(
            "OFFLINE_DEVICE_BINDING_REQUIRED",
            "device_binding is required.",
        )
    })?;
    if optional_string(&device_binding, "device_id")
        .is_some_and(|binding_device| binding_device != device_id)
    {
        return Err(validation(
            "OFFLINE_DEVICE_BINDING_MISMATCH",
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
            "OFFLINE_DEVICE_BINDING_KEY_MISMATCH",
            "device_binding.offline_public_key does not match offline_public_key.",
        ));
    }
    let asset_literal = required_string(&value, "asset_definition_id")?.to_string();
    let world = app.state.world_view();
    let now = routing::asset_alias_observation_time_ms(app.state.as_ref());
    let asset_definition_id =
        routing::resolve_asset_definition_selector(&world, &asset_literal, now).map_err(|_| {
            validation_owned(
                "OFFLINE_INVALID_ASSET",
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
                code: "OFFLINE_HEADER_AUTH_REJECTED",
                message: "Offline Notes issuer requests must put account_id, timestamp_ms, nonce, and signature_base64 or witness_base64 in the JSON body; X-Iroha canonical auth headers are not accepted.".to_string(),
            });
        }
    }
    Ok(())
}

fn extract_body_auth(
    value: &Value,
) -> Result<(app_auth::CanonicalRequestBodyAuth<'_>, Vec<u8>), Error> {
    let account_id = required_string(value, "account_id")?;
    let timestamp_ms = required_u64_with_code(value, "timestamp_ms", "OFFLINE_SIGNATURE_REQUIRED")?;
    let nonce = required_string(value, "nonce")?;
    let signature_base64 = optional_string(value, "signature_base64");
    let witness_base64 = optional_string(value, "witness_base64");
    let proof = match (signature_base64, witness_base64) {
        (Some(signature), None) => app_auth::CanonicalRequestBodyProof::SignatureBase64(signature),
        (None, Some(witness)) => app_auth::CanonicalRequestBodyProof::WitnessBase64(witness),
        (None, None) => {
            return Err(Error::AppForbidden {
                code: "OFFLINE_SIGNATURE_REQUIRED",
                message: "Offline Notes issuer requests require exactly one body proof field: signature_base64 or witness_base64.".to_string(),
            });
        }
        (Some(_), Some(_)) => {
            return Err(Error::AppForbidden {
                code: "OFFLINE_SIGNATURE_INVALID",
                message: "Offline Notes issuer requests must not include both signature_base64 and witness_base64.".to_string(),
            });
        }
    };
    let mut unsigned = value.clone();
    let Value::Object(map) = &mut unsigned else {
        return Err(validation(
            "OFFLINE_INVALID_JSON",
            "Offline Notes request body must be a JSON object.",
        ));
    };
    map.remove("signature_base64");
    map.remove("witness_base64");
    let unsigned_body = json::to_vec(&unsigned).map_err(|source| Error::SerializationFailure {
        context: "offline_body_auth_unsigned_json",
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
    issuer: &OfflineIssuerRuntime,
    request: &ParsedOfflineRequest,
    now_ms: u64,
) -> Result<VerifiedDeviceAttestation, Error> {
    let receipt = request
        .device_binding
        .get("attestation_receipt")
        .ok_or_else(|| {
            validation(
                "OFFLINE_ATTESTATION_RECEIPT_REQUIRED",
                "device_binding.attestation_receipt is required.",
            )
        })?;
    let receipt_object = value_object_ref(receipt, "OFFLINE_INVALID_ATTESTATION_RECEIPT")?;
    let signature = required_string(receipt, "signature_base64")?;
    let mut unsigned_object = receipt_object.clone();
    unsigned_object.remove("signature_base64");
    let unsigned = Value::Object(unsigned_object);
    verify_json_signature(
        &issuer.attestation_verifier_public_key,
        &unsigned,
        signature,
        "offline_attestation_receipt",
        "OFFLINE_ATTESTATION_RECEIPT_INVALID",
        "Offline Notes attestation receipt signature is invalid.",
    )?;

    let version = required_u64(receipt, "version")?;
    if version != 1 {
        return Err(validation(
            "OFFLINE_ATTESTATION_RECEIPT_INVALID",
            "Offline Notes attestation receipt version is unsupported.",
        ));
    }
    if required_string(receipt, "account_id")? != request.account_literal {
        return Err(validation(
            "OFFLINE_ATTESTATION_ACCOUNT_MISMATCH",
            "Offline Notes attestation receipt account_id does not match request account_id.",
        ));
    }
    if required_string(receipt, "device_id")? != request.device_id {
        return Err(validation(
            "OFFLINE_ATTESTATION_DEVICE_MISMATCH",
            "Offline Notes attestation receipt device_id does not match request device_id.",
        ));
    }
    if !required_bool(receipt, "hardware_one_use")? {
        return Err(validation(
            "OFFLINE_ATTESTATION_NOT_ONE_USE",
            "Offline Notes attestation receipt does not certify hardware one-use semantics.",
        ));
    }
    let issued_at = required_u64(receipt, "issued_at_ms")?;
    let expires_at = required_u64(receipt, "expires_at_ms")?;
    if issued_at > now_ms || expires_at <= now_ms || issued_at >= expires_at {
        return Err(validation(
            "OFFLINE_ATTESTATION_RECEIPT_EXPIRED",
            "Offline Notes attestation receipt is not currently valid.",
        ));
    }

    let request_public_key = decode_note_public_key(&request.offline_public_key)?;
    let public_key_base64 = required_string(receipt, "offline_public_key_base64")?;
    let public_key = decode_canonical_base64(
        public_key_base64,
        "offline_public_key_base64",
        "OFFLINE_INVALID_NOTE_PUBLIC_KEY",
    )?;
    if public_key.len() != 32 || public_key != request_public_key {
        return Err(validation(
            "OFFLINE_ATTESTATION_KEY_MISMATCH",
            "Offline Notes attestation receipt note key does not match request offline_public_key.",
        ));
    }

    let assertion_public_key_base64 = required_string(receipt, "assertion_public_key_base64")?;
    let assertion_public_key = decode_canonical_base64(
        assertion_public_key_base64,
        "assertion_public_key_base64",
        "OFFLINE_INVALID_ASSERTION_PUBLIC_KEY",
    )?;
    if assertion_public_key.is_empty() {
        return Err(validation(
            "OFFLINE_INVALID_ASSERTION_PUBLIC_KEY",
            "Offline Notes assertion public key must not be empty.",
        ));
    }
    verify_optional_assertion_public_key(request, &assertion_public_key)?;

    let attestation_report_hash = required_string(receipt, "attestation_report_hash_hex")?;
    let report_hash_bytes = hex::decode(attestation_report_hash).map_err(|_| {
        validation(
            "OFFLINE_INVALID_ATTESTATION_REPORT_HASH",
            "Offline Notes attestation_report_hash_hex must be hex.",
        )
    })?;
    if report_hash_bytes.len() != Hash::LENGTH {
        return Err(validation(
            "OFFLINE_INVALID_ATTESTATION_REPORT_HASH",
            "Offline Notes attestation_report_hash_hex must encode 32 bytes.",
        ));
    }
    if let Some(report) = optional_string(&request.device_binding, "attestation_report_base64") {
        let report_bytes = decode_base64_material(report).ok_or_else(|| {
            validation(
                "OFFLINE_INVALID_ATTESTATION_REPORT",
                "Offline Notes attestation_report_base64 must be base64.",
            )
        })?;
        if !sha256_hex(&report_bytes).eq_ignore_ascii_case(attestation_report_hash) {
            return Err(validation(
                "OFFLINE_ATTESTATION_REPORT_MISMATCH",
                "Offline Notes attestation report hash does not match receipt.",
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
    issuer: &OfflineIssuerRuntime,
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
    issuer: &OfflineIssuerRuntime,
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
    issuer: &OfflineIssuerRuntime,
    request: &ParsedOfflineRequest,
    expected_lineage_id: &str,
    now_ms: u64,
    key_policy: LineageKeyPolicy,
) -> Result<VerifiedLineageState, Error> {
    let state = request.value.get("lineage_state").ok_or_else(|| {
        validation(
            "OFFLINE_LINEAGE_STATE_REQUIRED",
            "Signed Offline Notes lineage_state is required.",
        )
    })?;
    value_object_ref(state, "OFFLINE_INVALID_LINEAGE_STATE")?;

    let lineage_id = required_string(state, "lineage_id")?;
    if lineage_id != expected_lineage_id {
        return Err(validation(
            "OFFLINE_LINEAGE_MISMATCH",
            "Offline Notes lineage_state.lineage_id does not match lineage_id.",
        ));
    }
    if required_string(state, "account_id")? != request.account_literal {
        return Err(validation(
            "OFFLINE_LINEAGE_ACCOUNT_MISMATCH",
            "Offline Notes lineage_state.account_id does not match account_id.",
        ));
    }
    if required_string(state, "device_id")? != request.device_id {
        return Err(validation(
            "OFFLINE_LINEAGE_DEVICE_MISMATCH",
            "Offline Notes lineage_state.device_id does not match device_id.",
        ));
    }
    let state_offline_public_key = required_string(state, "offline_public_key")?;
    if matches!(key_policy, LineageKeyPolicy::MatchRequest)
        && state_offline_public_key != request.offline_public_key
    {
        return Err(validation(
            "OFFLINE_LINEAGE_KEY_MISMATCH",
            "Offline Notes lineage_state.offline_public_key does not match offline_public_key.",
        ));
    }
    if required_string(state, "asset_definition_id")? != request.asset_definition_literal {
        return Err(validation(
            "OFFLINE_LINEAGE_ASSET_MISMATCH",
            "Offline Notes lineage_state.asset_definition_id does not match asset_definition_id.",
        ));
    }

    let balance = parse_amount(required_string(state, "balance")?, "lineage_state.balance")?;
    let locked_balance = parse_amount(
        required_string(state, "locked_balance")?,
        "lineage_state.locked_balance",
    )?;
    if locked_balance != Numeric::zero() {
        return Err(validation(
            "OFFLINE_LINEAGE_LOCKED_BALANCE_UNSUPPORTED",
            "Offline Notes issuer does not accept non-zero locked_balance.",
        ));
    }
    let revision = required_u64(state, "server_revision")?;
    if required_u64(state, "pending_local_revision")? != revision {
        return Err(validation(
            "OFFLINE_LINEAGE_REVISION_MISMATCH",
            "Offline Notes lineage_state revision fields do not match.",
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
            "OFFLINE_LINEAGE_STATE_HASH_MISMATCH",
            "Offline Notes lineage_state hash is invalid.",
        ));
    }

    let authorization = state.get("authorization").ok_or_else(|| {
        validation(
            "OFFLINE_LINEAGE_AUTHORIZATION_REQUIRED",
            "Offline Notes lineage_state.authorization is required.",
        )
    })?;
    value_object_ref(authorization, "OFFLINE_INVALID_LINEAGE_AUTHORIZATION")?;
    let authorization_id = required_string(authorization, "authorization_id")?;
    if required_string(authorization, "account_id")? != request.account_literal
        || required_string(authorization, "lineage_id")? != lineage_id
    {
        return Err(validation(
            "OFFLINE_LINEAGE_AUTHORIZATION_MISMATCH",
            "Offline Notes lineage authorization does not match lineage state.",
        ));
    }
    if required_string(authorization, "max_balance")? != issuer.max_balance.to_string()
        || required_string(authorization, "max_tx_value")? != issuer.max_tx_value.to_string()
    {
        return Err(validation(
            "OFFLINE_LINEAGE_AUTHORIZATION_POLICY_MISMATCH",
            "Offline Notes lineage authorization no longer matches issuer policy.",
        ));
    }
    let auth_issued_at = required_u64(authorization, "issued_at_ms")?;
    let auth_refresh_at = required_u64(authorization, "refresh_at_ms")?;
    let auth_expires_at = required_u64(authorization, "expires_at_ms")?;
    if auth_issued_at > now_ms || auth_expires_at <= now_ms || auth_issued_at >= auth_expires_at {
        return Err(validation(
            "OFFLINE_LINEAGE_AUTHORIZATION_EXPIRED",
            "Offline Notes lineage authorization is not currently valid.",
        ));
    }
    let auth_device_binding = authorization
        .get("device_binding")
        .cloned()
        .ok_or_else(|| {
            validation(
                "OFFLINE_LINEAGE_AUTHORIZATION_DEVICE_BINDING_REQUIRED",
                "Offline Notes lineage authorization device_binding is required.",
            )
        })?;
    if optional_string(&auth_device_binding, "device_id")
        .is_some_and(|device_id| device_id != request.device_id)
        || optional_string(&auth_device_binding, "offline_public_key")
            .is_some_and(|key| key != state_offline_public_key)
    {
        return Err(validation(
            "OFFLINE_LINEAGE_AUTHORIZATION_DEVICE_MISMATCH",
            "Offline Notes lineage authorization device binding does not match request.",
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
        "offline_authorization",
        "OFFLINE_LINEAGE_AUTHORIZATION_SIGNATURE_INVALID",
        "Offline Notes lineage authorization signature is invalid.",
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
        "offline_lineage_state",
        "OFFLINE_LINEAGE_STATE_SIGNATURE_INVALID",
        "Offline Notes lineage state signature is invalid.",
    )?;

    Ok(VerifiedLineageState { balance, revision })
}

fn build_lineage_state(
    issuer: &OfflineIssuerRuntime,
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
    let signature = issuer.sign_json_base64(&unsigned, "offline_lineage_state")?;
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
    issuer: &OfflineIssuerRuntime,
    request: &ParsedOfflineRequest,
    lineage_id: &str,
    now_ms: u64,
    key_certificate: Option<Value>,
) -> Result<Value, Error> {
    let authorization_id = offline_identifier(
        "auth",
        &format!(
            "{}:{}:{}",
            request.account_literal, lineage_id, request.device_id
        ),
    );
    let verdict_id = offline_identifier(
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
    let signature = issuer.sign_json_base64(&unsigned, "offline_authorization")?;
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
    issuer: &OfflineIssuerRuntime,
    request: &ParsedOfflineRequest,
    attestation: &VerifiedDeviceAttestation,
    now_ms: u64,
) -> Result<Value, Error> {
    build_key_certificate_bundle(issuer, request, attestation, now_ms)
        .map(|(certificate, _chain_certificate)| certificate)
}

fn build_key_certificate_bundle(
    issuer: &OfflineIssuerRuntime,
    request: &ParsedOfflineRequest,
    attestation: &VerifiedDeviceAttestation,
    now_ms: u64,
) -> Result<(Value, OfflineNoteKeyCertificate), Error> {
    let chain = build_chain_certificate(issuer, request, attestation)?;
    let signing_bytes = chain
        .signing_bytes()
        .map_err(|source| Error::SerializationFailure {
            context: "offline_key_certificate_payload",
            source: source.into(),
        })?;
    let signature = chain.issuer_signature.payload();
    let expires_at = now_ms.saturating_add(duration_ms(issuer.certificate_ttl));
    let certificate = json_object(vec![
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
            chain
                .assertion_usage_count_limit
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
    ]);
    Ok((certificate, chain))
}

fn build_chain_certificate(
    issuer: &OfflineIssuerRuntime,
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
                context: "offline_key_certificate_payload",
                source: source.into(),
            })?;
    certificate.issuer_signature = issuer.sign_bytes(&signing_bytes);
    Ok(certificate)
}

fn build_settlement(
    issuer: &OfflineIssuerRuntime,
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
    let signature = issuer.sign_json_base64(&unsigned, "offline_settlement")?;
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
    Ok(sha256_json_hex(&payload, "offline_lineage_state_hash")?)
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
            string_value("pk-retail-wallet-ios:offline:settlement-entry"),
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
    sha256_json_hex(&payload, "offline_settlement_entry")
}

fn required_string<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, Error> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            validation_owned(
                "OFFLINE_MISSING_FIELD",
                format!("Offline Notes field `{field}` is required."),
            )
        })
}

fn required_note_commitment(value: &Value) -> Result<Hash, Error> {
    let raw = required_string(value, "note_commitment")?;
    if raw.len() != Hash::LENGTH * 2 || raw.starts_with("0x") || raw.starts_with("0X") {
        return Err(invalid_note_commitment());
    }
    Hash::from_str(raw).map_err(|_| invalid_note_commitment())
}

fn invalid_note_commitment() -> Error {
    validation(
        "OFFLINE_INVALID_NOTE_COMMITMENT",
        "Offline Notes note_commitment must be a bare 64-character Iroha hash hex string.",
    )
}

fn required_u64(value: &Value, field: &'static str) -> Result<u64, Error> {
    required_u64_with_code(value, field, "OFFLINE_MISSING_FIELD")
}

fn required_u64_with_code(
    value: &Value,
    field: &'static str,
    code: &'static str,
) -> Result<u64, Error> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| {
        validation_owned(
            code,
            format!("Offline Notes numeric field `{field}` is required."),
        )
    })
}

fn required_bool(value: &Value, field: &'static str) -> Result<bool, Error> {
    value.get(field).and_then(Value::as_bool).ok_or_else(|| {
        validation_owned(
            "OFFLINE_MISSING_FIELD",
            format!("Offline Notes boolean field `{field}` is required."),
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
            "OFFLINE_INVALID_AMOUNT",
            format!("Invalid Offline Notes {field}: {err}"),
        )
    })
}

fn parse_positive_amount(raw: &str, field: &'static str) -> Result<Numeric, Error> {
    let amount = parse_amount(raw, field)?;
    if amount <= Numeric::zero() {
        return Err(validation_owned(
            "OFFLINE_INVALID_AMOUNT",
            format!("Offline Notes {field} must be greater than zero."),
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
            "OFFLINE_INVALID_ASSERTION_USAGE_LIMIT",
            "assertion_usage_count_limit must be an unsigned integer.",
        )
    })?;
    let limit = u32::try_from(raw).map_err(|_| {
        validation(
            "OFFLINE_INVALID_ASSERTION_USAGE_LIMIT",
            "assertion_usage_count_limit exceeds u32.",
        )
    })?;
    if limit != 1 {
        return Err(validation(
            "OFFLINE_INVALID_ASSERTION_USAGE_LIMIT",
            "assertion_usage_count_limit must be exactly 1 for one-use Offline Notes keys.",
        ));
    }
    Ok(Some(limit))
}

fn decode_note_public_key(raw: &str) -> Result<Vec<u8>, Error> {
    let public_key = decode_key_material(raw).ok_or_else(|| {
        validation(
            "OFFLINE_INVALID_NOTE_PUBLIC_KEY",
            "offline_public_key must be hex/base64 encoded key bytes.",
        )
    })?;
    if public_key.len() != 32 {
        return Err(validation(
            "OFFLINE_INVALID_NOTE_PUBLIC_KEY",
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
            format!("Offline Notes {field} must be standard base64."),
        )
    })?;
    if BASE64_STANDARD.encode(&bytes) != raw {
        return Err(validation_owned(
            code,
            format!("Offline Notes {field} must use canonical standard base64."),
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
                        "OFFLINE_INVALID_ASSERTION_PUBLIC_KEY",
                        "Offline Notes assertion public key must be hex/base64 key bytes.",
                    )
                })?;
            if bytes != expected {
                return Err(validation(
                    "OFFLINE_ASSERTION_PUBLIC_KEY_MISMATCH",
                    "Offline Notes assertion public key does not match attestation receipt.",
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
            context: "offline_json_object",
            source: json::Error::Message("expected JSON object".to_string()),
        }),
    }
}

fn value_object_ref<'a>(value: &'a Value, code: &'static str) -> Result<&'a Map, Error> {
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(validation(
            code,
            "Offline Notes field must be a JSON object.",
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

fn offline_identifier(prefix: &str, value: &str) -> String {
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
    use axum::http::{HeaderMap, HeaderValue, Method, Uri};
    use iroha_core::prelude::World;
    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        Registrable,
        account::{Account, MultisigMember, MultisigPolicy},
        asset::AssetDefinition,
        domain::{Domain, DomainId},
        soracloud::{
            CANONICAL_REQUEST_WITNESS_VERSION_V1, CanonicalRequestSignatureWitnessV1,
            CanonicalRequestWitnessV1,
        },
    };

    const NOW_MS: u64 = 1_700_000_000_000;
    const REPORT_BYTES: &[u8] = b"offline-platform-attestation";

    fn sample_issuer() -> (OfflineIssuerRuntime, KeyPair) {
        let issuer_key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        let verifier_key_pair = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
        let authority = AccountId::new(issuer_key_pair.public_key().clone());
        (
            OfflineIssuerRuntime {
                authority,
                key_pair: issuer_key_pair,
                attestation_verifier_public_key: verifier_key_pair.public_key().clone(),
                max_balance: "100".parse().expect("max balance"),
                max_tx_value: "25".parse().expect("max transaction value"),
                certificate_ttl: Duration::from_secs(300),
                authorization_refresh: Duration::from_secs(60),
                authorization_ttl: Duration::from_secs(600),
                policy: Arc::new(RwLock::new(OfflinePolicyState::default())),
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

    #[test]
    fn revocation_bundle_is_issuer_signed_and_sorted() {
        let (issuer, _) = sample_issuer();
        replace_policy_state(
            &issuer,
            OfflinePolicyState {
                verdict_ids: vec!["verdict-b".to_string(), "verdict-a".to_string()],
                blacklisted_account_ids: vec!["i105b".to_string(), "i105a".to_string()],
                asset_send_limits: vec![
                    OfflineAssetSendLimitState {
                        asset_definition_id: "usd#offline".to_string(),
                        daily_send_limit: "25".to_string(),
                        monthly_send_limit: "100".to_string(),
                    },
                    OfflineAssetSendLimitState {
                        asset_definition_id: "eur#offline".to_string(),
                        daily_send_limit: "10".to_string(),
                        monthly_send_limit: "50".to_string(),
                    },
                ],
            },
        )
        .expect("policy update");

        let bundle = build_revocation_bundle(&issuer, NOW_MS).expect("revocation bundle");
        let mut unsigned = value_object(bundle).expect("bundle object");
        let signature = unsigned
            .remove("issuer_signature_base64")
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .expect("signature");
        assert_eq!(unsigned.get("issued_at_ms"), Some(&number_value(NOW_MS)));
        assert_eq!(
            unsigned.get("expires_at_ms"),
            Some(&number_value(NOW_MS + OFFLINE_REVOCATION_BUNDLE_TTL_MS))
        );
        assert_eq!(
            unsigned.get("verdict_ids"),
            Some(&Value::Array(vec![
                string_value("verdict-a"),
                string_value("verdict-b")
            ]))
        );
        assert_eq!(
            unsigned.get("blacklisted_account_ids"),
            Some(&Value::Array(vec![
                string_value("i105a"),
                string_value("i105b")
            ]))
        );
        let limits = unsigned
            .get("asset_send_limits")
            .and_then(Value::as_array)
            .expect("asset limits");
        assert_eq!(
            limits
                .first()
                .and_then(|value| value.get("asset_definition_id"))
                .and_then(Value::as_str),
            Some("eur#offline")
        );
        verify_json_signature(
            issuer.key_pair.public_key(),
            &Value::Object(unsigned),
            &signature,
            "offline_revocation_bundle_test",
            "OFFLINE_REVOCATION_SIGNATURE_INVALID",
            "invalid revocation signature",
        )
        .expect("revocation bundle signature");
    }

    #[test]
    fn policy_snapshot_normalizes_amounts_and_rejects_empty_revocation_registers() {
        let payload = json_object(vec![
            (
                "blacklisted_account_ids",
                Value::Array(vec![string_value(" i105b "), string_value("i105a")]),
            ),
            (
                "asset_send_limits",
                Value::Array(vec![json_object(vec![
                    ("asset_definition_id", string_value("usd#offline")),
                    ("daily_send_limit", string_value("10.00")),
                    ("monthly_send_limit", string_value("100.00")),
                ])]),
            ),
        ]);

        let policy = parse_policy_snapshot(&payload).expect("policy snapshot");
        assert_eq!(
            policy.blacklisted_account_ids,
            vec!["i105a".to_string(), "i105b".to_string()]
        );
        assert_eq!(policy.asset_send_limits[0].daily_send_limit, "10.00");
        assert_eq!(policy.asset_send_limits[0].monthly_send_limit, "100.00");
        assert_eq!(
            validation_code(
                collect_string_fields(&json_object(vec![]), &["account_id"]).and_then(|values| {
                    if values.is_empty() {
                        Err(validation(
                            "OFFLINE_REVOCATION_EMPTY",
                            "Offline revocation payload must include account_id.",
                        ))
                    } else {
                        Ok(values)
                    }
                },)
            ),
            "OFFLINE_REVOCATION_EMPTY"
        );
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

    fn app_error_message(result: Result<impl Sized, Error>) -> String {
        match result {
            Err(
                Error::AppQueryValidation { message, .. } | Error::AppForbidden { message, .. },
            ) => message,
            Err(error) => panic!("expected app error, got {error:?}"),
            Ok(_) => panic!("expected app error"),
        }
    }

    fn endpoint_cases() -> [(&'static str, &'static str); 4] {
        [
            (PATH_KEYS_REFILL, ENDPOINT_KEYS_REFILL),
            (PATH_NOTES_ISSUE, ENDPOINT_NOTES_ISSUE),
            (PATH_NOTES_REDEEM, ENDPOINT_NOTES_REDEEM),
            (PATH_AUDIT, ENDPOINT_AUDIT),
        ]
    }

    fn asset_definition_for_tests() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        )
    }

    fn app_with_account_and_asset(
        account_id: &AccountId,
        asset_definition_id: &AssetDefinitionId,
    ) -> SharedAppState {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(account_id);
        let account = Account::new(account_id.clone()).build(account_id);
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name(asset_definition_id.name().to_string())
            .build(account_id);
        let world = World::with([domain], [account], [asset_definition]);
        crate::tests_runtime_handlers::mk_app_state_for_tests_with_world(world)
    }

    fn signer_account(seed: u8) -> (KeyPair, AccountId, String) {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let account = AccountId::new(key_pair.public_key().clone());
        let literal = account.canonical_i105().expect("i105 account");
        (key_pair, account, literal)
    }

    fn non_ascii_signer_account() -> (KeyPair, AccountId, String) {
        for seed in 1_u8..=u8::MAX {
            let (key_pair, account, literal) = signer_account(seed);
            if literal.chars().any(|ch| !ch.is_ascii()) {
                return (key_pair, account, literal);
            }
        }
        panic!("expected at least one deterministic test key to produce non-ASCII I105");
    }

    fn minimal_parse_body(account_literal: &str, asset_literal: &str) -> Value {
        let offline_public_key = "a5".repeat(32);
        let device_binding = json_object(vec![
            ("device_id", string_value("device-1")),
            ("offline_public_key", string_value(&offline_public_key)),
            (
                "signature_base64",
                string_value("nested-device-signature-is-not-body-auth"),
            ),
        ]);
        json_object(vec![
            ("account_id", string_value(account_literal)),
            ("operation_id", string_value("operation-1")),
            ("device_id", string_value("device-1")),
            ("offline_public_key", string_value(offline_public_key)),
            ("asset_definition_id", string_value(asset_literal)),
            ("device_binding", device_binding),
        ])
    }

    fn add_body_freshness(
        value: &mut Value,
        account_literal: &str,
        timestamp_ms: u64,
        nonce: &str,
    ) {
        insert_field(value, "account_id", string_value(account_literal));
        insert_field(value, "timestamp_ms", number_value(timestamp_ms));
        insert_field(value, "nonce", string_value(nonce));
    }

    fn unsigned_body_bytes(value: &Value) -> Vec<u8> {
        let mut unsigned = value.clone();
        let Value::Object(map) = &mut unsigned else {
            panic!("expected object body");
        };
        map.remove("signature_base64");
        map.remove("witness_base64");
        json::to_vec(&unsigned).expect("unsigned body json")
    }

    fn sign_body_value(
        method: &Method,
        uri: &Uri,
        key_pair: &KeyPair,
        mut value: Value,
        account_literal: &str,
        timestamp_ms: u64,
        nonce: &str,
    ) -> Value {
        add_body_freshness(&mut value, account_literal, timestamp_ms, nonce);
        let unsigned = unsigned_body_bytes(&value);
        let message = app_auth::canonical_request_signature_message(
            method,
            uri,
            &unsigned,
            timestamp_ms,
            nonce,
        );
        let signature = Signature::new(key_pair.private_key(), &message);
        insert_field(
            &mut value,
            "signature_base64",
            string_value(BASE64_STANDARD.encode(signature.payload())),
        );
        value
    }

    fn encode_body(value: &Value) -> Vec<u8> {
        json::to_vec(value).expect("request body json")
    }

    fn parse_offline_request(
        app: &SharedAppState,
        method: &Method,
        uri: &Uri,
        headers: &HeaderMap,
        body: &Value,
        endpoint: &'static str,
    ) -> Result<ParsedOfflineRequest, Error> {
        let body = encode_body(body);
        parse_and_authorize(app.as_ref(), method, uri, headers, &body, endpoint)
    }

    fn multisig_witness_body_value(
        method: &Method,
        uri: &Uri,
        signers: &[&KeyPair],
        mut value: Value,
        account: &AccountId,
        account_literal: &str,
        timestamp_ms: u64,
        nonce: &str,
    ) -> Value {
        add_body_freshness(&mut value, account_literal, timestamp_ms, nonce);
        let unsigned = unsigned_body_bytes(&value);
        let mut witness = CanonicalRequestWitnessV1 {
            schema_version: CANONICAL_REQUEST_WITNESS_VERSION_V1,
            subject_account: account.clone(),
            timestamp_ms,
            nonce: nonce.to_string(),
            canonical_request_hash: app_auth::canonical_request_hash(method, uri, &unsigned),
            signatures: Vec::new(),
        };
        let message = app_auth::canonical_request_witness_message(&witness)
            .expect("canonical witness payload");
        witness.signatures = signers
            .iter()
            .map(|signer| CanonicalRequestSignatureWitnessV1 {
                signer: signer.public_key().clone(),
                signature: Signature::new(signer.private_key(), &message),
            })
            .collect();
        insert_field(
            &mut value,
            "witness_base64",
            string_value(app_auth::witness_header_value(&witness).expect("witness base64")),
        );
        value
    }

    #[test]
    fn body_auth_accepts_single_signature_for_all_issuer_endpoints() {
        let _guard = crate::tests_runtime_handlers::app_auth_test_guard(Default::default());
        let (key_pair, account, account_literal) = non_ascii_signer_account();
        let asset_definition_id = asset_definition_for_tests();
        let asset_literal = asset_definition_id.to_string();
        let app = app_with_account_and_asset(&account, &asset_definition_id);
        let method = Method::POST;
        let headers = HeaderMap::new();

        for (path, endpoint) in endpoint_cases() {
            let uri: Uri = path.parse().expect("uri");
            let body = sign_body_value(
                &method,
                &uri,
                &key_pair,
                minimal_parse_body(&account_literal, &asset_literal),
                &account_literal,
                now_ms(),
                &format!("valid-body-auth-{endpoint}"),
            );
            let parsed = parse_offline_request(&app, &method, &uri, &headers, &body, endpoint)
                .expect("valid body auth");
            assert_eq!(parsed.account_id, account);
            assert_eq!(parsed.account_literal, account_literal);
        }
    }

    #[test]
    fn body_auth_accepts_multisig_witness() {
        let _guard = crate::tests_runtime_handlers::app_auth_test_guard(Default::default());
        let signer_one = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
        let signer_two = KeyPair::from_seed(vec![0x42; 32], Algorithm::Ed25519);
        let policy = MultisigPolicy::new(
            2,
            vec![
                MultisigMember::new(signer_one.public_key().clone(), 1).expect("member"),
                MultisigMember::new(signer_two.public_key().clone(), 1).expect("member"),
            ],
        )
        .expect("policy");
        let account = AccountId::new_multisig(policy);
        let account_literal = account.canonical_i105().expect("i105 account");
        let asset_definition_id = asset_definition_for_tests();
        let asset_literal = asset_definition_id.to_string();
        let app = app_with_account_and_asset(&account, &asset_definition_id);
        let method = Method::POST;
        let uri: Uri = PATH_AUDIT.parse().expect("uri");
        let body = multisig_witness_body_value(
            &method,
            &uri,
            &[&signer_one, &signer_two],
            minimal_parse_body(&account_literal, &asset_literal),
            &account,
            &account_literal,
            now_ms(),
            "valid-body-witness",
        );

        let parsed = parse_offline_request(
            &app,
            &method,
            &uri,
            &HeaderMap::new(),
            &body,
            ENDPOINT_AUDIT,
        )
        .expect("valid multisig body auth");
        assert_eq!(parsed.account_id, account);
    }

    #[test]
    fn body_auth_rejects_missing_and_ambiguous_proofs() {
        let _guard = crate::tests_runtime_handlers::app_auth_test_guard(Default::default());
        let (key_pair, account, account_literal) = signer_account(0x43);
        let asset_definition_id = asset_definition_for_tests();
        let asset_literal = asset_definition_id.to_string();
        let app = app_with_account_and_asset(&account, &asset_definition_id);
        let method = Method::POST;
        let uri: Uri = PATH_KEYS_REFILL.parse().expect("uri");
        let headers = HeaderMap::new();

        let mut missing = minimal_parse_body(&account_literal, &asset_literal);
        add_body_freshness(&mut missing, &account_literal, now_ms(), "missing-proof");
        assert_eq!(
            app_error_code(parse_offline_request(
                &app,
                &method,
                &uri,
                &headers,
                &missing,
                ENDPOINT_KEYS_REFILL,
            )),
            "OFFLINE_SIGNATURE_REQUIRED"
        );

        let mut both = sign_body_value(
            &method,
            &uri,
            &key_pair,
            minimal_parse_body(&account_literal, &asset_literal),
            &account_literal,
            now_ms(),
            "both-proofs",
        );
        insert_field(&mut both, "witness_base64", string_value("AA=="));
        assert_eq!(
            app_error_code(parse_offline_request(
                &app,
                &method,
                &uri,
                &headers,
                &both,
                ENDPOINT_KEYS_REFILL,
            )),
            "OFFLINE_SIGNATURE_INVALID"
        );
    }

    #[test]
    fn body_auth_rejects_stale_replayed_and_tampered_requests() {
        let _guard = crate::tests_runtime_handlers::app_auth_test_guard(Default::default());
        let (key_pair, account, account_literal) = signer_account(0x44);
        let asset_definition_id = asset_definition_for_tests();
        let asset_literal = asset_definition_id.to_string();
        let app = app_with_account_and_asset(&account, &asset_definition_id);
        let method = Method::POST;
        let uri: Uri = PATH_NOTES_ISSUE.parse().expect("uri");
        let headers = HeaderMap::new();

        let stale = sign_body_value(
            &method,
            &uri,
            &key_pair,
            minimal_parse_body(&account_literal, &asset_literal),
            &account_literal,
            1,
            "stale-body-auth",
        );
        let stale_message = app_error_message(parse_offline_request(
            &app,
            &method,
            &uri,
            &headers,
            &stale,
            ENDPOINT_NOTES_ISSUE,
        ));
        assert!(stale_message.contains("timestamp outside allowed skew"));

        let replayed = sign_body_value(
            &method,
            &uri,
            &key_pair,
            minimal_parse_body(&account_literal, &asset_literal),
            &account_literal,
            now_ms(),
            "replayed-body-auth",
        );
        parse_offline_request(
            &app,
            &method,
            &uri,
            &headers,
            &replayed,
            ENDPOINT_NOTES_ISSUE,
        )
        .expect("first request");
        let replay_message = app_error_message(parse_offline_request(
            &app,
            &method,
            &uri,
            &headers,
            &replayed,
            ENDPOINT_NOTES_ISSUE,
        ));
        assert!(replay_message.contains("nonce already used"));

        let mut tampered = sign_body_value(
            &method,
            &uri,
            &key_pair,
            minimal_parse_body(&account_literal, &asset_literal),
            &account_literal,
            now_ms(),
            "tampered-body-auth",
        );
        insert_field(
            &mut tampered,
            "operation_id",
            string_value("operation-tampered"),
        );
        let tampered_message = app_error_message(parse_offline_request(
            &app,
            &method,
            &uri,
            &headers,
            &tampered,
            ENDPOINT_NOTES_ISSUE,
        ));
        assert!(tampered_message.contains("signature failed verification"));
    }

    #[test]
    fn body_auth_rejects_legacy_x_iroha_auth_headers() {
        let _guard = crate::tests_runtime_handlers::app_auth_test_guard(Default::default());
        let (key_pair, account, account_literal) = signer_account(0x45);
        let asset_definition_id = asset_definition_for_tests();
        let asset_literal = asset_definition_id.to_string();
        let app = app_with_account_and_asset(&account, &asset_definition_id);
        let method = Method::POST;
        let uri: Uri = PATH_NOTES_REDEEM.parse().expect("uri");
        let body = sign_body_value(
            &method,
            &uri,
            &key_pair,
            minimal_parse_body(&account_literal, &asset_literal),
            &account_literal,
            now_ms(),
            "legacy-header-rejected",
        );
        let mut headers = HeaderMap::new();
        headers.insert(app_auth::HEADER_ACCOUNT, HeaderValue::from_static("legacy"));

        assert_eq!(
            app_error_code(parse_offline_request(
                &app,
                &method,
                &uri,
                &headers,
                &body,
                ENDPOINT_NOTES_REDEEM,
            )),
            "OFFLINE_HEADER_AUTH_REJECTED"
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

        let (certificate, chain_certificate) =
            build_key_certificate_bundle(&issuer, &request, &attestation, NOW_MS)
                .expect("certificate");
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
        assert_eq!(
            optional_string(&certificate, "issuer_signature_base64"),
            Some(
                BASE64_STANDARD
                    .encode(chain_certificate.issuer_signature.payload())
                    .as_str()
            )
        );
        let chain_signing_bytes = chain_certificate
            .signing_bytes()
            .expect("chain certificate signing bytes");
        assert_eq!(
            optional_string(&certificate, "issuer_signature_payload_base64"),
            Some(BASE64_STANDARD.encode(chain_signing_bytes).as_str())
        );
    }

    #[test]
    fn one_use_certificate_usage_limit_must_be_one_when_present() {
        let (issuer, verifier) = sample_issuer();
        let mut request = sample_request(&verifier, [0xA5; 32], vec![0xB6; 65]);
        let attestation =
            verify_device_attestation(&issuer, &request, NOW_MS).expect("valid attestation");

        let certificate =
            build_key_certificate(&issuer, &request, &attestation, NOW_MS).expect("certificate");
        assert_eq!(
            certificate
                .get("assertion_usage_count_limit")
                .and_then(Value::as_u64),
            Some(1)
        );

        for invalid in [
            number_value(0),
            number_value(2),
            number_value(u64::from(u32::MAX) + 1),
            string_value("1"),
            Value::Bool(true),
        ] {
            insert_field(
                &mut request.device_binding,
                "assertion_usage_count_limit",
                invalid,
            );
            assert_eq!(
                validation_code(build_key_certificate(
                    &issuer,
                    &request,
                    &attestation,
                    NOW_MS
                )),
                "OFFLINE_INVALID_ASSERTION_USAGE_LIMIT"
            );
        }

        let Value::Object(binding) = &mut request.device_binding else {
            panic!("expected binding object");
        };
        binding.remove("assertion_usage_count_limit");
        let certificate =
            build_key_certificate(&issuer, &request, &attestation, NOW_MS).expect("certificate");
        assert_eq!(
            certificate.get("assertion_usage_count_limit"),
            Some(&Value::Null)
        );
        let chain_certificate =
            build_chain_certificate(&issuer, &request, &attestation).expect("chain certificate");
        assert_eq!(chain_certificate.assertion_usage_count_limit, None);
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
            "OFFLINE_ATTESTATION_RECEIPT_REQUIRED"
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
            "OFFLINE_INVALID_ASSERTION_PUBLIC_KEY"
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
            "OFFLINE_LINEAGE_STATE_HASH_MISMATCH"
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
            "OFFLINE_LINEAGE_KEY_MISMATCH"
        );
        let verified = verify_existing_lineage_state(&issuer, &new_request, lineage_id, NOW_MS)
            .expect("existing lineage state");
        assert_eq!(verified.balance.to_string(), "20");
        assert_eq!(verified.revision, 4);
    }

    #[test]
    fn issue_note_commitment_accepts_wallet_hash() {
        let note_commitment = Hash::new(b"wallet-derived-note-commitment");
        let request = json_object(vec![(
            "note_commitment",
            string_value(note_commitment.to_string()),
        )]);

        assert_eq!(
            required_note_commitment(&request).expect("note commitment"),
            note_commitment
        );
    }

    #[test]
    fn issue_note_commitment_rejects_missing_and_malformed_values() {
        let missing = json_object(Vec::new());
        assert_eq!(
            validation_code(required_note_commitment(&missing)),
            "OFFLINE_MISSING_FIELD"
        );

        for invalid in [
            "0x0000000000000000000000000000000000000000000000000000000000000001",
            "000000000000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "not-a-hash",
        ] {
            let request = json_object(vec![("note_commitment", string_value(invalid))]);
            assert_eq!(
                validation_code(required_note_commitment(&request)),
                "OFFLINE_INVALID_NOTE_COMMITMENT"
            );
        }
    }

    #[test]
    fn settlement_entry_hash_remains_lineage_metadata() {
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

        assert_eq!(
            entry_hash,
            settlement_entry_hash(
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
            .expect("repeat entry hash")
        );
    }
}
