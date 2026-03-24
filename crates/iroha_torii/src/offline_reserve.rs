#![cfg(feature = "app_api")]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write as _,
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ciborium::{de::from_reader, value::Value as CborValue};
use ed25519_dalek::{Signature as DalekSignature, Verifier, VerifyingKey};
use iroha_core::state::WorldReadOnly;
use iroha_crypto::{Hash, PrivateKey, Signature};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    isi::offline::{
        RefundOfflineEscrowBalance, RegisterOfflineVerdictRevocation, ReserveOfflineEscrowBalance,
    },
    offline::{OfflineVerdictRevocation, OfflineVerdictRevocationReason},
    prelude::{InstructionBox, Numeric, TransactionBuilder},
};
use mv::storage::StorageReadOnly;
use norito::json::{self};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{AppState, Error, OfflineIssuerSigner, routing};

const TRANSFER_PREFIX: &str = "wallet-offline-transfer:";
fn store_path() -> PathBuf {
    crate::data_dir::base_dir().join("offline_reserves.json")
}

fn load_store_state() -> Result<OfflineReserveStoreState, Error> {
    let path = store_path();
    if !path.exists() {
        return Ok(OfflineReserveStoreState::default());
    }
    let bytes = fs::read(&path).map_err(|err| {
        conversion_error(format!(
            "failed to read offline reserve registry at {}: {err}",
            path.display()
        ))
    })?;
    json::from_slice(&bytes).map_err(|err| {
        conversion_error(format!(
            "failed to decode offline reserve registry at {}: {err}",
            path.display()
        ))
    })
}

fn persist_store_state(state: &OfflineReserveStoreState) -> Result<(), Error> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            conversion_error(format!(
                "failed to create offline reserve registry directory {}: {err}",
                parent.display()
            ))
        })?;
    }
    let payload = json::to_vec_pretty(state).map_err(|err| {
        conversion_error(format!("failed to encode offline reserve registry: {err}"))
    })?;
    let tmp_dir = path
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(crate::data_dir::base_dir);
    let mut tmp = tempfile::NamedTempFile::new_in(&tmp_dir).map_err(|err| {
        conversion_error(format!(
            "failed to create temp reserve registry file: {err}"
        ))
    })?;
    tmp.write_all(&payload)
        .and_then(|_| tmp.flush())
        .map_err(|err| conversion_error(format!("failed to write reserve registry: {err}")))?;
    tmp.persist(&path).map_err(|err| {
        conversion_error(format!(
            "failed to persist offline reserve registry at {}: {err}",
            path.display()
        ))
    })?;
    Ok(())
}

pub(crate) struct OfflineReserveStore {
    inner: RwLock<OfflineReserveStoreState>,
}

impl Default for OfflineReserveStore {
    fn default() -> Self {
        Self {
            inner: RwLock::new(load_store_state().unwrap_or_else(|err| {
                iroha_logger::warn!(%err, "failed to load offline reserve registry");
                OfflineReserveStoreState::default()
            })),
        }
    }
}

#[derive(Default, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct OfflineReserveStoreState {
    #[norito(default)]
    lineage_to_reserve_id: BTreeMap<String, String>,
    #[norito(default)]
    reserves: BTreeMap<String, StoredReserve>,
}

#[derive(Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct StoredReserve {
    reserve_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    asset_definition_id: String,
    balance: Numeric,
    parked_balance: Numeric,
    server_revision: u64,
    server_state_hash: String,
    pending_local_revision: u64,
    authorization: OfflineSpendAuthorization,
    app_attest_key_id: String,
    #[norito(default)]
    counter_book: BTreeMap<String, u64>,
    #[norito(default)]
    seen_transfer_ids: BTreeSet<String>,
    #[norito(default)]
    seen_sender_states: BTreeSet<String>,
    #[norito(default)]
    operation_results: BTreeMap<String, OfflineReserveEnvelope>,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineDeviceAttestation {
    pub key_id: String,
    pub counter: u64,
    pub assertion_base64: String,
    pub challenge_hash_hex: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineSpendAuthorization {
    pub authorization_id: String,
    pub reserve_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub verdict_id: String,
    pub max_balance: String,
    pub max_tx_value: String,
    pub issued_at_ms: u64,
    pub refresh_at_ms: u64,
    pub expires_at_ms: u64,
    pub app_attest_key_id: String,
    pub issuer_signature_base64: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveState {
    pub reserve_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub asset_definition_id: String,
    pub balance: String,
    pub parked_balance: String,
    pub server_revision: u64,
    pub server_state_hash: String,
    pub pending_local_revision: u64,
    pub authorization: OfflineSpendAuthorization,
    pub issuer_signature_base64: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveEnvelope {
    pub reserve_state: OfflineReserveState,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineRevocationBundle {
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub verdict_ids: Vec<String>,
    pub issuer_signature_base64: String,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineTransferReceipt {
    pub version: i32,
    pub transfer_id: String,
    pub direction: String,
    pub reserve_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub pre_balance: String,
    pub post_balance: String,
    pub pre_parked_balance: String,
    pub post_parked_balance: String,
    pub pre_state_hash: String,
    pub post_state_hash: String,
    pub local_revision: u64,
    pub counterparty_reserve_id: String,
    pub counterparty_account_id: String,
    pub counterparty_device_id: String,
    pub counterparty_offline_public_key: String,
    pub amount: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub authorization: Option<OfflineSpendAuthorization>,
    pub attestation: OfflineDeviceAttestation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub source_payload: Option<String>,
    pub sender_signature_base64: String,
    pub created_at_ms: u64,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineOutgoingTransferPayload {
    pub version: i32,
    pub anchor: OfflineReserveState,
    pub ancestry_receipts: Vec<OfflineTransferReceipt>,
    pub receipt: OfflineTransferReceipt,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveSetupRequest {
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub asset_definition_id: String,
    pub app_attest_key_id: String,
    pub attestation: OfflineDeviceAttestation,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveTopUpRequest {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub reserve_id: Option<String>,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub asset_definition_id: String,
    pub app_attest_key_id: String,
    pub amount: String,
    pub attestation: OfflineDeviceAttestation,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveRenewRequest {
    pub operation_id: String,
    pub reserve_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub app_attest_key_id: String,
    pub attestation: OfflineDeviceAttestation,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveSyncRequest {
    pub operation_id: String,
    pub reserve_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub receipts: Vec<OfflineTransferReceipt>,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveDefundRequest {
    pub operation_id: String,
    pub reserve_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub amount: String,
    pub receipts: Vec<OfflineTransferReceipt>,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineReserveRevocationRequest {
    pub verdict_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub note: Option<String>,
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflineRevocationList {
    pub verdict_ids: Vec<String>,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AuthorizationUnsignedPayload<'a> {
    authorization_id: &'a str,
    reserve_id: &'a str,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    verdict_id: &'a str,
    max_balance: &'a str,
    max_tx_value: &'a str,
    issued_at_ms: u64,
    refresh_at_ms: u64,
    expires_at_ms: u64,
    app_attest_key_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct ReserveStateUnsignedPayload<'a> {
    reserve_id: &'a str,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    asset_definition_id: &'a str,
    balance: &'a str,
    parked_balance: &'a str,
    server_revision: u64,
    server_state_hash: &'a str,
    pending_local_revision: u64,
    authorization_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct RevocationBundleUnsignedPayload {
    issued_at_ms: u64,
    expires_at_ms: u64,
    verdict_ids: Vec<String>,
}

#[derive(crate::json_macros::JsonSerialize)]
struct TransferReceiptUnsignedPayload {
    version: i32,
    transfer_id: String,
    direction: String,
    reserve_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    pre_balance: String,
    post_balance: String,
    pre_parked_balance: String,
    post_parked_balance: String,
    pre_state_hash: String,
    post_state_hash: String,
    local_revision: u64,
    counterparty_reserve_id: String,
    counterparty_account_id: String,
    counterparty_device_id: String,
    counterparty_offline_public_key: String,
    amount: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    authorization: Option<OfflineSpendAuthorization>,
    attestation: OfflineDeviceAttestation,
    #[norito(skip_serializing_if = "Option::is_none")]
    source_payload: Option<String>,
    created_at_ms: u64,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LocalStateHashPayload<'a> {
    reserve_id: &'a str,
    previous_state_hash: &'a str,
    transfer_id: &'a str,
    direction: &'a str,
    counterparty_reserve_id: &'a str,
    amount: &'a str,
    local_revision: u64,
    post_balance: &'a str,
    post_parked_balance: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AttestationSendPayload<'a> {
    reserve_id: &'a str,
    transfer_id: &'a str,
    amount: &'a str,
    receiver_reserve_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AttestationReceivePayload<'a> {
    reserve_id: &'a str,
    transfer_id: &'a str,
    amount: &'a str,
    sender_reserve_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AttestationChallengePayload<'a> {
    account_id: &'a str,
    reserve_id: &'a str,
    operation: &'a str,
    payload_hash: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct ReserveTopUpAttestationPayload<'a> {
    reserve_id: &'a str,
    amount: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct ReserveRenewAttestationPayload<'a> {
    reserve_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct ReserveSetupAttestationPayload<'a> {
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct ReserveAnchorHashPayload<'a> {
    reserve_id: &'a str,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    asset_definition_id: &'a str,
    balance: &'a str,
    parked_balance: &'a str,
    server_revision: u64,
    pending_local_revision: u64,
    authorization_id: &'a str,
}

pub(crate) fn setup_reserve(
    app: &AppState,
    req: OfflineReserveSetupRequest,
) -> Result<OfflineReserveEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.asset_definition_id, "asset_definition_id")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;

    let issuer = issuer(app)?;
    let mut store = app.offline_reserves.inner.write();
    let lineage = reserve_lineage_key(&req.account_id, &req.device_id, &req.offline_public_key);
    if let Some(reserve_id) = store.lineage_to_reserve_id.get(&lineage).cloned() {
        let reserve = store
            .reserves
            .get_mut(&reserve_id)
            .ok_or_else(|| conversion_error("reserve lineage is corrupted".to_owned()))?;
        if reserve.app_attest_key_id != req.app_attest_key_id {
            return Err(conversion_error(
                "app_attest_key_id does not match the existing reserve lineage".to_owned(),
            ));
        }
        if reserve.asset_definition_id != req.asset_definition_id {
            return Err(conversion_error(
                "asset_definition_id does not match the existing reserve lineage".to_owned(),
            ));
        }
        validate_setup_attestation(&req, &mut reserve.counter_book, &reserve.app_attest_key_id)?;
        let envelope = envelope_from_record(issuer, reserve)?;
        persist_store_state(&store)?;
        return Ok(envelope);
    }

    let reserve_id = deterministic_id(
        "reserve",
        &[
            req.account_id.as_str(),
            req.device_id.as_str(),
            req.offline_public_key.as_str(),
        ],
    );
    let verdict_id = deterministic_id(
        "verdict",
        &[
            req.account_id.as_str(),
            req.device_id.as_str(),
            req.offline_public_key.as_str(),
        ],
    );
    let now_ms = now_ms();
    let account_id = req.account_id.clone();
    let device_id = req.device_id.clone();
    let offline_public_key = req.offline_public_key.clone();
    let authorization = signed_authorization(
        issuer,
        AuthorizationDraft {
            reserve_id: reserve_id.clone(),
            account_id: account_id.clone(),
            device_id: device_id.clone(),
            offline_public_key: offline_public_key.clone(),
            verdict_id,
            app_attest_key_id: req.app_attest_key_id.clone(),
            issued_at_ms: now_ms,
        },
    )?;
    let record = StoredReserve {
        reserve_id: reserve_id.clone(),
        account_id: account_id.clone(),
        device_id: device_id.clone(),
        offline_public_key: offline_public_key.clone(),
        asset_definition_id: req.asset_definition_id.clone(),
        balance: Numeric::zero(),
        parked_balance: Numeric::zero(),
        server_revision: 0,
        server_state_hash: reserve_anchor_hash(
            &reserve_id,
            &account_id,
            &device_id,
            &offline_public_key,
            &req.asset_definition_id,
            "0",
            "0",
            0,
            0,
            &authorization.authorization_id,
        )?,
        pending_local_revision: 0,
        authorization,
        app_attest_key_id: req.app_attest_key_id.clone(),
        counter_book: BTreeMap::new(),
        seen_transfer_ids: BTreeSet::new(),
        seen_sender_states: BTreeSet::new(),
        operation_results: BTreeMap::new(),
    };
    let mut record = record;
    validate_setup_attestation(&req, &mut record.counter_book, &record.app_attest_key_id)?;
    let envelope = envelope_from_record(issuer, &record)?;
    store
        .lineage_to_reserve_id
        .insert(lineage, reserve_id.clone());
    store.reserves.insert(reserve_id, record);
    persist_store_state(&store)?;
    Ok(envelope)
}

pub(crate) async fn top_up_reserve(
    app: &AppState,
    req: OfflineReserveTopUpRequest,
) -> Result<OfflineReserveEnvelope, Error> {
    let issuer = issuer(app)?;
    let amount = parse_amount(&req.amount)?;
    let operation_key = operation_key("topup", &req.operation_id);
    let (reserve_id, asset_definition_id) = {
        let mut store = app.offline_reserves.inner.write();
        let (reserve_id, asset_definition_id) = {
            let reserve = reserve_for_topup(&mut store, issuer, &req)?;
            if let Some(existing) = reserve.operation_results.get(&operation_key) {
                return Ok(existing.clone());
            }
            validate_reserve_attestation(
                &req.account_id,
                &reserve.reserve_id,
                "topup",
                canonical_json_bytes(&ReserveTopUpAttestationPayload {
                    reserve_id: &reserve.reserve_id,
                    amount: &canonical_amount_string(&amount),
                })?,
                &req.app_attest_key_id,
                &req.attestation,
                &mut reserve.counter_book,
            )?;
            (
                reserve.reserve_id.clone(),
                reserve.asset_definition_id.clone(),
            )
        };
        persist_store_state(&store)?;
        (reserve_id, asset_definition_id)
    };
    let authority = operator_authority(issuer)?;
    let asset = controller_asset_id(&req.account_id, &asset_definition_id)?;
    submit_signed_instruction(
        app,
        authority,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(ReserveOfflineEscrowBalance {
            asset,
            amount: amount.clone(),
        }),
        "/v1/offline/reserve/topup",
    )
    .await?;

    let mut store = app.offline_reserves.inner.write();
    let reserve = reserve_for_mutation(
        &mut store,
        &reserve_id,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        Some(&asset_definition_id),
        Some(&req.app_attest_key_id),
    )?;
    if let Some(existing) = reserve.operation_results.get(&operation_key) {
        return Ok(existing.clone());
    }
    reserve.balance = reserve
        .balance
        .clone()
        .checked_add(amount)
        .ok_or_else(|| conversion_error("reserve balance overflow".to_owned()))?;
    reserve.parked_balance = parse_numeric(&minimum_required_parked_balance(
        &canonical_amount_string(&reserve.balance),
        Some(&reserve.authorization),
        now_ms(),
    )?)?;
    reserve.server_revision = reserve.server_revision.saturating_add(1);
    reserve.server_state_hash = reserve_anchor_hash(
        &reserve.reserve_id,
        &reserve.account_id,
        &reserve.device_id,
        &reserve.offline_public_key,
        &reserve.asset_definition_id,
        &canonical_amount_string(&reserve.balance),
        &canonical_amount_string(&reserve.parked_balance),
        reserve.server_revision,
        reserve.pending_local_revision,
        &reserve.authorization.authorization_id,
    )?;
    let envelope = envelope_from_record(issuer, reserve)?;
    reserve
        .operation_results
        .insert(operation_key, envelope.clone());
    persist_store_state(&store)?;
    Ok(envelope)
}

pub(crate) async fn renew_reserve(
    app: &AppState,
    req: OfflineReserveRenewRequest,
) -> Result<OfflineReserveEnvelope, Error> {
    let issuer = issuer(app)?;
    let mut store = app.offline_reserves.inner.write();
    let reserve = reserve_for_mutation(
        &mut store,
        &req.reserve_id,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        None,
        Some(&req.app_attest_key_id),
    )?;
    let operation_key = operation_key("renew", &req.operation_id);
    if let Some(existing) = reserve.operation_results.get(&operation_key) {
        return Ok(existing.clone());
    }
    validate_reserve_attestation(
        &req.account_id,
        &reserve.reserve_id,
        "renew",
        canonical_json_bytes(&ReserveRenewAttestationPayload {
            reserve_id: &reserve.reserve_id,
        })?,
        &req.app_attest_key_id,
        &req.attestation,
        &mut reserve.counter_book,
    )?;

    reserve.authorization = signed_authorization(
        issuer,
        AuthorizationDraft {
            reserve_id: reserve.reserve_id.clone(),
            account_id: reserve.account_id.clone(),
            device_id: reserve.device_id.clone(),
            offline_public_key: reserve.offline_public_key.clone(),
            verdict_id: reserve.authorization.verdict_id.clone(),
            app_attest_key_id: reserve.app_attest_key_id.clone(),
            issued_at_ms: now_ms(),
        },
    )?;
    reserve.parked_balance = parse_numeric(&minimum_required_parked_balance(
        &canonical_amount_string(&reserve.balance),
        Some(&reserve.authorization),
        now_ms(),
    )?)?;
    reserve.server_revision = reserve.server_revision.saturating_add(1);
    reserve.server_state_hash = reserve_anchor_hash(
        &reserve.reserve_id,
        &reserve.account_id,
        &reserve.device_id,
        &reserve.offline_public_key,
        &reserve.asset_definition_id,
        &canonical_amount_string(&reserve.balance),
        &canonical_amount_string(&reserve.parked_balance),
        reserve.server_revision,
        reserve.pending_local_revision,
        &reserve.authorization.authorization_id,
    )?;
    let envelope = envelope_from_record(issuer, reserve)?;
    reserve
        .operation_results
        .insert(operation_key, envelope.clone());
    persist_store_state(&store)?;
    Ok(envelope)
}

pub(crate) fn sync_reserve(
    app: &AppState,
    req: OfflineReserveSyncRequest,
) -> Result<OfflineReserveEnvelope, Error> {
    let issuer = issuer(app)?;
    let mut store = app.offline_reserves.inner.write();
    let reserve = reserve_for_mutation(
        &mut store,
        &req.reserve_id,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        None,
        None,
    )?;
    let operation_key = operation_key("sync", &req.operation_id);
    if let Some(existing) = reserve.operation_results.get(&operation_key) {
        return Ok(existing.clone());
    }

    apply_receipts(app, issuer, reserve, &req.receipts)?;
    let envelope = envelope_from_record(issuer, reserve)?;
    reserve
        .operation_results
        .insert(operation_key, envelope.clone());
    persist_store_state(&store)?;
    Ok(envelope)
}

pub(crate) async fn defund_reserve(
    app: &AppState,
    req: OfflineReserveDefundRequest,
) -> Result<OfflineReserveEnvelope, Error> {
    let issuer = issuer(app)?;
    let amount = parse_amount(&req.amount)?;
    let operation_key = operation_key("defund", &req.operation_id);
    let asset_definition_id = {
        let mut store = app.offline_reserves.inner.write();
        let asset_definition_id = {
            let reserve = reserve_for_mutation(
                &mut store,
                &req.reserve_id,
                &req.account_id,
                &req.device_id,
                &req.offline_public_key,
                None,
                None,
            )?;
            if let Some(existing) = reserve.operation_results.get(&operation_key) {
                return Ok(existing.clone());
            }
            apply_receipts(app, issuer, reserve, &req.receipts)?;
            reserve.asset_definition_id.clone()
        };
        persist_store_state(&store)?;
        asset_definition_id
    };
    let authority = operator_authority(issuer)?;
    let asset = controller_asset_id(&req.account_id, &asset_definition_id)?;
    submit_signed_instruction(
        app,
        authority,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(RefundOfflineEscrowBalance {
            asset,
            amount: amount.clone(),
        }),
        "/v1/offline/reserve/defund",
    )
    .await?;

    let mut store = app.offline_reserves.inner.write();
    let reserve = reserve_for_mutation(
        &mut store,
        &req.reserve_id,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        Some(&asset_definition_id),
        None,
    )?;
    if let Some(existing) = reserve.operation_results.get(&operation_key) {
        return Ok(existing.clone());
    }
    reserve.balance =
        reserve.balance.clone().checked_sub(amount).ok_or_else(|| {
            conversion_error("insufficient reserve balance for defund".to_owned())
        })?;
    reserve.parked_balance = parse_numeric(&minimum_required_parked_balance(
        &canonical_amount_string(&reserve.balance),
        Some(&reserve.authorization),
        now_ms(),
    )?)?;
    reserve.server_revision = reserve.server_revision.saturating_add(1);
    reserve.server_state_hash = reserve_anchor_hash(
        &reserve.reserve_id,
        &reserve.account_id,
        &reserve.device_id,
        &reserve.offline_public_key,
        &reserve.asset_definition_id,
        &canonical_amount_string(&reserve.balance),
        &canonical_amount_string(&reserve.parked_balance),
        reserve.server_revision,
        reserve.pending_local_revision,
        &reserve.authorization.authorization_id,
    )?;
    let envelope = envelope_from_record(issuer, reserve)?;
    reserve
        .operation_results
        .insert(operation_key, envelope.clone());
    persist_store_state(&store)?;
    Ok(envelope)
}

pub(crate) fn revocation_bundle(app: &AppState) -> Result<OfflineRevocationBundle, Error> {
    let issuer = issuer(app)?;
    let mut verdict_ids = revoked_verdict_ids(app).into_iter().collect::<Vec<_>>();
    verdict_ids.sort();
    verdict_ids.dedup();

    let issued_at_ms = now_ms();
    let expires_at_ms =
        issued_at_ms.saturating_add(issuer.reserve_policy.revocation_ttl.as_millis() as u64);
    let mut bundle = OfflineRevocationBundle {
        issued_at_ms,
        expires_at_ms,
        verdict_ids,
        issuer_signature_base64: String::new(),
    };
    let signature_payload = canonical_json_bytes(&RevocationBundleUnsignedPayload {
        issued_at_ms: bundle.issued_at_ms,
        expires_at_ms: bundle.expires_at_ms,
        verdict_ids: bundle.verdict_ids.clone(),
    })?;
    bundle.issuer_signature_base64 = sign_base64(issuer, &signature_payload);
    Ok(bundle)
}

pub(crate) fn revocation_list(app: &AppState) -> Result<OfflineRevocationList, Error> {
    let mut verdict_ids = revoked_verdict_ids(app).into_iter().collect::<Vec<_>>();
    verdict_ids.sort();
    verdict_ids.dedup();
    Ok(OfflineRevocationList { verdict_ids })
}

pub(crate) async fn register_revocation(
    app: &AppState,
    req: OfflineReserveRevocationRequest,
) -> Result<OfflineRevocationBundle, Error> {
    let issuer = issuer(app)?;
    let authority = operator_authority(issuer)?;
    let verdict_id = Hash::from_str(req.verdict_id.trim())
        .map_err(|err| conversion_error(format!("invalid verdict_id: {err}")))?;
    let reason = req
        .reason
        .as_deref()
        .map(OfflineVerdictRevocationReason::from_str)
        .transpose()
        .map_err(|err| conversion_error(format!("invalid revocation reason: {err}")))?
        .unwrap_or_default();
    let revocation = OfflineVerdictRevocation {
        verdict_id,
        issuer: authority.clone(),
        revoked_at_ms: 0,
        reason,
        note: req.note,
        metadata: Default::default(),
    };
    submit_signed_instruction(
        app,
        authority,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(RegisterOfflineVerdictRevocation { revocation }),
        "/v1/offline/revocations",
    )
    .await?;
    revocation_bundle(app)
}

fn reserve_for_topup<'a>(
    store: &'a mut OfflineReserveStoreState,
    issuer: &OfflineIssuerSigner,
    req: &OfflineReserveTopUpRequest,
) -> Result<&'a mut StoredReserve, Error> {
    if let Some(reserve_id) = req.reserve_id.as_ref() {
        return reserve_for_mutation(
            store,
            reserve_id,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
            Some(&req.asset_definition_id),
            Some(&req.app_attest_key_id),
        );
    }

    let lineage = reserve_lineage_key(&req.account_id, &req.device_id, &req.offline_public_key);
    if let Some(reserve_id) = store.lineage_to_reserve_id.get(&lineage).cloned() {
        return reserve_for_mutation(
            store,
            &reserve_id,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
            Some(&req.asset_definition_id),
            Some(&req.app_attest_key_id),
        );
    }

    let reserve_id = deterministic_id(
        "reserve",
        &[
            req.account_id.as_str(),
            req.device_id.as_str(),
            req.offline_public_key.as_str(),
        ],
    );
    let verdict_id = deterministic_id(
        "verdict",
        &[
            req.account_id.as_str(),
            req.device_id.as_str(),
            req.offline_public_key.as_str(),
        ],
    );
    let authorization = signed_authorization(
        issuer,
        AuthorizationDraft {
            reserve_id: reserve_id.clone(),
            account_id: req.account_id.clone(),
            device_id: req.device_id.clone(),
            offline_public_key: req.offline_public_key.clone(),
            verdict_id,
            app_attest_key_id: req.app_attest_key_id.clone(),
            issued_at_ms: now_ms(),
        },
    )?;
    store
        .lineage_to_reserve_id
        .insert(lineage, reserve_id.clone());
    let reserve = StoredReserve {
        reserve_id: reserve_id.clone(),
        account_id: req.account_id.clone(),
        device_id: req.device_id.clone(),
        offline_public_key: req.offline_public_key.clone(),
        asset_definition_id: req.asset_definition_id.clone(),
        balance: Numeric::zero(),
        parked_balance: Numeric::zero(),
        server_revision: 0,
        server_state_hash: reserve_anchor_hash(
            &reserve_id,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
            &req.asset_definition_id,
            "0",
            "0",
            0,
            0,
            &authorization.authorization_id,
        )?,
        pending_local_revision: 0,
        authorization,
        app_attest_key_id: req.app_attest_key_id.clone(),
        counter_book: BTreeMap::new(),
        seen_transfer_ids: BTreeSet::new(),
        seen_sender_states: BTreeSet::new(),
        operation_results: BTreeMap::new(),
    };
    store.reserves.insert(reserve_id.clone(), reserve);
    store
        .reserves
        .get_mut(&reserve_id)
        .ok_or_else(|| conversion_error("failed to create reserve".to_owned()))
}

fn reserve_for_mutation<'a>(
    store: &'a mut OfflineReserveStoreState,
    reserve_id: &str,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: Option<&str>,
    app_attest_key_id: Option<&str>,
) -> Result<&'a mut StoredReserve, Error> {
    let reserve = store
        .reserves
        .get_mut(reserve_id)
        .ok_or_else(|| conversion_error("offline reserve not found".to_owned()))?;
    if reserve.account_id != account_id
        || reserve.device_id != device_id
        || reserve.offline_public_key != offline_public_key
    {
        return Err(conversion_error(
            "offline reserve lineage does not match the request".to_owned(),
        ));
    }
    if let Some(key_id) = app_attest_key_id {
        if reserve.app_attest_key_id != key_id {
            return Err(conversion_error(
                "app_attest_key_id does not match the reserve lineage".to_owned(),
            ));
        }
    }
    if let Some(definition_id) = asset_definition_id {
        if reserve.asset_definition_id != definition_id {
            return Err(conversion_error(
                "asset_definition_id does not match the reserve lineage".to_owned(),
            ));
        }
    }
    Ok(reserve)
}

fn apply_receipts(
    app: &AppState,
    issuer: &OfflineIssuerSigner,
    reserve: &mut StoredReserve,
    receipts: &[OfflineTransferReceipt],
) -> Result<(), Error> {
    if receipts.is_empty() {
        return Ok(());
    }
    let issuer_public_key = issuer_public_key_base64(issuer);
    let mut current_balance = canonical_amount_string(&reserve.balance);
    let mut current_parked = canonical_amount_string(&reserve.parked_balance);
    let mut current_hash = reserve.server_state_hash.clone();
    let mut current_revision = reserve.pending_local_revision;
    let revoked_verdict_ids = revoked_verdict_ids(app);

    let mut ordered = receipts.to_vec();
    ordered.sort_by_key(|receipt| receipt.local_revision);
    let mut applied_any = false;

    for receipt in ordered {
        if receipt.local_revision <= current_revision {
            continue;
        }
        validate_receipt_signature(&receipt)?;
        validate_attestation_hash(&receipt)?;
        validate_counter(&receipt.attestation, &mut reserve.counter_book)?;
        let expected_post_balance = validate_local_continuity(
            &receipt,
            &reserve.reserve_id,
            &reserve.offline_public_key,
            &current_balance,
            &current_parked,
            &current_hash,
            current_revision,
            &issuer_public_key,
            &revoked_verdict_ids,
        )?;
        if !reserve
            .seen_transfer_ids
            .insert(receipt.transfer_id.clone())
        {
            return Err(conversion_error(
                "duplicate transfer_id in reserve sync".to_owned(),
            ));
        }
        if !reserve.seen_sender_states.insert(sender_state_key(
            &receipt.reserve_id,
            receipt.local_revision,
        )) {
            return Err(conversion_error(
                "duplicate sender state in reserve sync".to_owned(),
            ));
        }
        current_balance = expected_post_balance;
        current_parked = receipt.post_parked_balance.clone();
        current_hash = receipt.post_state_hash.clone();
        current_revision = receipt.local_revision;
        applied_any = true;
    }

    if applied_any {
        reserve.balance = parse_amount(&current_balance)?;
        reserve.parked_balance = parse_numeric(&current_parked)?;
        reserve.pending_local_revision = current_revision;
        reserve.server_revision = reserve.server_revision.saturating_add(1);
        reserve.server_state_hash = current_hash;
    }

    Ok(())
}

fn validate_local_continuity(
    receipt: &OfflineTransferReceipt,
    expected_reserve_id: &str,
    expected_offline_public_key: &str,
    current_balance: &str,
    current_parked: &str,
    current_hash: &str,
    current_revision: u64,
    issuer_public_key_base64: &str,
    revoked_verdict_ids: &BTreeSet<String>,
) -> Result<String, Error> {
    if receipt.reserve_id != expected_reserve_id
        || receipt.offline_public_key != expected_offline_public_key
        || receipt.local_revision != current_revision.saturating_add(1)
        || receipt.pre_balance != current_balance
        || receipt.pre_parked_balance != current_parked
        || receipt.pre_state_hash != current_hash
    {
        return Err(conversion_error(
            "offline reserve continuity proof is invalid".to_owned(),
        ));
    }

    let expected_post_balance = match receipt.direction.as_str() {
        "outgoing" => {
            validate_receipt_authorization(
                receipt,
                true,
                issuer_public_key_base64,
                revoked_verdict_ids,
            )?;
            let spendable = subtract_amounts(current_balance, current_parked)?;
            if compare_amounts(&receipt.amount, &spendable)?.is_gt() {
                return Err(conversion_error(
                    "offline outgoing receipt exceeds sender spendable balance".to_owned(),
                ));
            }
            subtract_amounts(current_balance, &receipt.amount)?
        }
        "incoming" => {
            validate_receipt_authorization(
                receipt,
                false,
                issuer_public_key_base64,
                revoked_verdict_ids,
            )?;
            let source_payload = receipt.source_payload.as_deref().ok_or_else(|| {
                conversion_error("incoming receipt is missing source_payload".to_owned())
            })?;
            validate_source_payload(
                source_payload,
                &receipt.reserve_id,
                &receipt.amount,
                issuer_public_key_base64,
                revoked_verdict_ids,
            )?;
            add_amounts(current_balance, &receipt.amount)?
        }
        _ => {
            return Err(conversion_error(
                "offline receipt direction must be incoming or outgoing".to_owned(),
            ));
        }
    };
    validate_parked_continuity(receipt, &expected_post_balance)?;

    let expected_post_hash = next_local_state_hash(
        &receipt.reserve_id,
        current_hash,
        &receipt.transfer_id,
        &receipt.direction,
        &receipt.counterparty_reserve_id,
        &receipt.amount,
        receipt.local_revision,
        &expected_post_balance,
        &receipt.post_parked_balance,
    )?;
    if receipt.post_balance != expected_post_balance
        || receipt.post_state_hash != expected_post_hash
    {
        return Err(conversion_error(
            "offline reserve continuity proof is invalid".to_owned(),
        ));
    }
    Ok(expected_post_balance)
}

fn validate_source_payload(
    raw_payload: &str,
    recipient_reserve_id: &str,
    amount: &str,
    issuer_public_key_base64: &str,
    revoked_verdict_ids: &BTreeSet<String>,
) -> Result<(), Error> {
    let payload = decode_transfer_payload(raw_payload)?;
    validate_issuer_signature(
        authorization_unsigned_payload(&payload.anchor.authorization)?,
        &payload.anchor.authorization.issuer_signature_base64,
        issuer_public_key_base64,
    )?;
    validate_issuer_signature(
        reserve_state_unsigned_payload(&payload.anchor)?,
        &payload.anchor.issuer_signature_base64,
        issuer_public_key_base64,
    )?;

    let mut current_balance = payload.anchor.balance.clone();
    let mut current_parked = minimum_required_parked_balance(
        &current_balance,
        Some(&payload.anchor.authorization),
        payload
            .ancestry_receipts
            .first()
            .map(|receipt| receipt.created_at_ms)
            .unwrap_or(payload.receipt.created_at_ms),
    )?;
    let mut current_hash = payload.anchor.server_state_hash.clone();
    let mut current_revision = payload.anchor.pending_local_revision;
    let mut counter_book = BTreeMap::new();
    let mut seen_sender_states = BTreeSet::new();

    let mut ancestry = payload.ancestry_receipts.clone();
    ancestry.sort_by_key(|receipt| receipt.local_revision);
    for receipt in ancestry {
        if !seen_sender_states.insert(sender_state_key(
            &receipt.reserve_id,
            receipt.local_revision,
        )) {
            return Err(conversion_error(
                "duplicate sender state in ancestry receipts".to_owned(),
            ));
        }
        validate_receipt_signature(&receipt)?;
        validate_attestation_hash(&receipt)?;
        validate_counter(&receipt.attestation, &mut counter_book)?;
        current_balance = validate_local_continuity(
            &receipt,
            &payload.anchor.reserve_id,
            &payload.anchor.offline_public_key,
            &current_balance,
            &current_parked,
            &current_hash,
            current_revision,
            issuer_public_key_base64,
            revoked_verdict_ids,
        )?;
        current_parked = receipt.post_parked_balance.clone();
        current_hash = receipt.post_state_hash.clone();
        current_revision = receipt.local_revision;
    }

    if !seen_sender_states.insert(sender_state_key(
        &payload.receipt.reserve_id,
        payload.receipt.local_revision,
    )) {
        return Err(conversion_error(
            "duplicate sender state in outgoing payload".to_owned(),
        ));
    }
    validate_receipt_signature(&payload.receipt)?;
    validate_attestation_hash(&payload.receipt)?;
    validate_counter(&payload.receipt.attestation, &mut counter_book)?;
    let _ = validate_local_continuity(
        &payload.receipt,
        &payload.anchor.reserve_id,
        &payload.anchor.offline_public_key,
        &current_balance,
        &current_parked,
        &current_hash,
        current_revision,
        issuer_public_key_base64,
        revoked_verdict_ids,
    )?;
    if payload.receipt.direction != "outgoing"
        || payload.receipt.counterparty_reserve_id != recipient_reserve_id
        || canonical_amount_string(&parse_amount(&payload.receipt.amount)?)
            != canonical_amount_string(&parse_amount(amount)?)
    {
        return Err(conversion_error(
            "source payload does not target the expected reserve".to_owned(),
        ));
    }
    Ok(())
}

fn validate_attestation_hash(receipt: &OfflineTransferReceipt) -> Result<(), Error> {
    let operation = match receipt.direction.as_str() {
        "incoming" => "receive",
        "outgoing" => "send",
        _ => {
            return Err(conversion_error(
                "offline receipt direction must be incoming or outgoing".to_owned(),
            ));
        }
    };
    let transfer_payload = if receipt.direction == "incoming" {
        canonical_json_bytes(&AttestationReceivePayload {
            reserve_id: &receipt.reserve_id,
            transfer_id: &receipt.transfer_id,
            amount: &receipt.amount,
            sender_reserve_id: &receipt.counterparty_reserve_id,
        })?
    } else {
        canonical_json_bytes(&AttestationSendPayload {
            reserve_id: &receipt.reserve_id,
            transfer_id: &receipt.transfer_id,
            amount: &receipt.amount,
            receiver_reserve_id: &receipt.counterparty_reserve_id,
        })?
    };
    let payload_hash = sha256_hex(&transfer_payload);
    let challenge_seed = canonical_json_bytes(&AttestationChallengePayload {
        account_id: &receipt.account_id,
        reserve_id: &receipt.reserve_id,
        operation,
        payload_hash: &payload_hash,
    })?;
    let expected = sha256_hex(&challenge_seed);
    if receipt.attestation.challenge_hash_hex != expected {
        return Err(conversion_error(
            "offline transfer attestation challenge hash is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_counter(
    attestation: &OfflineDeviceAttestation,
    counter_book: &mut BTreeMap<String, u64>,
) -> Result<(), Error> {
    let previous = counter_book.get(&attestation.key_id).copied().unwrap_or(0);
    if attestation.counter <= previous {
        return Err(conversion_error(
            "offline transfer counter replay detected".to_owned(),
        ));
    }
    counter_book.insert(attestation.key_id.clone(), attestation.counter);
    Ok(())
}

fn validate_receipt_signature(receipt: &OfflineTransferReceipt) -> Result<(), Error> {
    let payload = transfer_receipt_unsigned_payload(receipt)?;
    validate_signature(
        &payload,
        &receipt.sender_signature_base64,
        &receipt.offline_public_key,
    )
}

fn validate_signature(
    payload: &[u8],
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<(), Error> {
    let public_key_bytes = BASE64_STANDARD
        .decode(public_key_base64)
        .map_err(|err| conversion_error(format!("invalid base64 public key: {err}")))?;
    let signature_bytes = BASE64_STANDARD
        .decode(signature_base64)
        .map_err(|err| conversion_error(format!("invalid base64 signature: {err}")))?;
    let verifying_key = VerifyingKey::from_bytes(
        &public_key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| conversion_error("ed25519 public key must be 32 bytes".to_owned()))?,
    )
    .map_err(|err| conversion_error(format!("invalid ed25519 public key: {err}")))?;
    let signature = DalekSignature::from_slice(&signature_bytes)
        .map_err(|err| conversion_error(format!("invalid ed25519 signature: {err}")))?;
    verifying_key
        .verify(payload, &signature)
        .map_err(|err| conversion_error(format!("invalid offline transfer signature: {err}")))?;
    Ok(())
}

fn validate_issuer_signature(
    payload: Vec<u8>,
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<(), Error> {
    validate_signature(&payload, signature_base64, public_key_base64)
}

fn transfer_receipt_unsigned_payload(receipt: &OfflineTransferReceipt) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&TransferReceiptUnsignedPayload {
        version: receipt.version,
        transfer_id: receipt.transfer_id.clone(),
        direction: receipt.direction.clone(),
        reserve_id: receipt.reserve_id.clone(),
        account_id: receipt.account_id.clone(),
        device_id: receipt.device_id.clone(),
        offline_public_key: receipt.offline_public_key.clone(),
        pre_balance: canonical_amount_string(&parse_numeric(&receipt.pre_balance)?),
        post_balance: canonical_amount_string(&parse_numeric(&receipt.post_balance)?),
        pre_parked_balance: canonical_amount_string(&parse_numeric(&receipt.pre_parked_balance)?),
        post_parked_balance: canonical_amount_string(&parse_numeric(&receipt.post_parked_balance)?),
        pre_state_hash: receipt.pre_state_hash.clone(),
        post_state_hash: receipt.post_state_hash.clone(),
        local_revision: receipt.local_revision,
        counterparty_reserve_id: receipt.counterparty_reserve_id.clone(),
        counterparty_account_id: receipt.counterparty_account_id.clone(),
        counterparty_device_id: receipt.counterparty_device_id.clone(),
        counterparty_offline_public_key: receipt.counterparty_offline_public_key.clone(),
        amount: canonical_amount_string(&parse_numeric(&receipt.amount)?),
        authorization: receipt.authorization.clone(),
        attestation: receipt.attestation.clone(),
        source_payload: receipt.source_payload.clone(),
        created_at_ms: receipt.created_at_ms,
    })
}

fn authorization_unsigned_payload(
    authorization: &OfflineSpendAuthorization,
) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&AuthorizationUnsignedPayload {
        authorization_id: &authorization.authorization_id,
        reserve_id: &authorization.reserve_id,
        account_id: &authorization.account_id,
        device_id: &authorization.device_id,
        offline_public_key: &authorization.offline_public_key,
        verdict_id: &authorization.verdict_id,
        max_balance: &canonical_amount_string(&parse_amount(&authorization.max_balance)?),
        max_tx_value: &canonical_amount_string(&parse_amount(&authorization.max_tx_value)?),
        issued_at_ms: authorization.issued_at_ms,
        refresh_at_ms: authorization.refresh_at_ms,
        expires_at_ms: authorization.expires_at_ms,
        app_attest_key_id: &authorization.app_attest_key_id,
    })
}

fn reserve_state_unsigned_payload(reserve_state: &OfflineReserveState) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&ReserveStateUnsignedPayload {
        reserve_id: &reserve_state.reserve_id,
        account_id: &reserve_state.account_id,
        device_id: &reserve_state.device_id,
        offline_public_key: &reserve_state.offline_public_key,
        asset_definition_id: &reserve_state.asset_definition_id,
        balance: &canonical_amount_string(&parse_numeric(&reserve_state.balance)?),
        parked_balance: &canonical_amount_string(&parse_numeric(&reserve_state.parked_balance)?),
        server_revision: reserve_state.server_revision,
        server_state_hash: &reserve_state.server_state_hash,
        pending_local_revision: reserve_state.pending_local_revision,
        authorization_id: &reserve_state.authorization.authorization_id,
    })
}

fn next_local_state_hash(
    reserve_id: &str,
    previous_state_hash: &str,
    transfer_id: &str,
    direction: &str,
    counterparty_reserve_id: &str,
    amount: &str,
    local_revision: u64,
    post_balance: &str,
    post_parked_balance: &str,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(&LocalStateHashPayload {
        reserve_id,
        previous_state_hash,
        transfer_id,
        direction,
        counterparty_reserve_id,
        amount: &canonical_amount_string(&parse_amount(amount)?),
        local_revision,
        post_balance: &canonical_amount_string(&parse_amount(post_balance)?),
        post_parked_balance: &canonical_amount_string(&parse_numeric(post_parked_balance)?),
    })?))
}

fn envelope_from_record(
    issuer: &OfflineIssuerSigner,
    record: &StoredReserve,
) -> Result<OfflineReserveEnvelope, Error> {
    let mut reserve_state = OfflineReserveState {
        reserve_id: record.reserve_id.clone(),
        account_id: record.account_id.clone(),
        device_id: record.device_id.clone(),
        offline_public_key: record.offline_public_key.clone(),
        asset_definition_id: record.asset_definition_id.clone(),
        balance: canonical_amount_string(&record.balance),
        parked_balance: canonical_amount_string(&record.parked_balance),
        server_revision: record.server_revision,
        server_state_hash: record.server_state_hash.clone(),
        pending_local_revision: record.pending_local_revision,
        authorization: record.authorization.clone(),
        issuer_signature_base64: String::new(),
    };
    reserve_state.issuer_signature_base64 =
        sign_base64(issuer, &reserve_state_unsigned_payload(&reserve_state)?);
    Ok(OfflineReserveEnvelope { reserve_state })
}

struct AuthorizationDraft {
    reserve_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    verdict_id: String,
    app_attest_key_id: String,
    issued_at_ms: u64,
}

fn signed_authorization(
    issuer: &OfflineIssuerSigner,
    draft: AuthorizationDraft,
) -> Result<OfflineSpendAuthorization, Error> {
    let authorization_id = deterministic_id(
        "authorization",
        &[
            draft.reserve_id.as_str(),
            draft.account_id.as_str(),
            draft.device_id.as_str(),
            draft.offline_public_key.as_str(),
            draft.verdict_id.as_str(),
            &draft.issued_at_ms.to_string(),
        ],
    );
    let mut authorization = OfflineSpendAuthorization {
        authorization_id,
        reserve_id: draft.reserve_id,
        account_id: draft.account_id,
        device_id: draft.device_id,
        offline_public_key: draft.offline_public_key,
        verdict_id: draft.verdict_id,
        max_balance: canonical_amount_string(&parse_amount(&issuer.reserve_policy.max_balance)?),
        max_tx_value: canonical_amount_string(&parse_amount(&issuer.reserve_policy.max_tx_value)?),
        issued_at_ms: draft.issued_at_ms,
        refresh_at_ms: draft
            .issued_at_ms
            .saturating_add(issuer.reserve_policy.authorization_refresh.as_millis() as u64),
        expires_at_ms: draft
            .issued_at_ms
            .saturating_add(issuer.reserve_policy.authorization_ttl.as_millis() as u64),
        app_attest_key_id: draft.app_attest_key_id,
        issuer_signature_base64: String::new(),
    };
    authorization.issuer_signature_base64 =
        sign_base64(issuer, &authorization_unsigned_payload(&authorization)?);
    Ok(authorization)
}

fn reserve_anchor_hash(
    reserve_id: &str,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: &str,
    balance: &str,
    parked_balance: &str,
    server_revision: u64,
    pending_local_revision: u64,
    authorization_id: &str,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &ReserveAnchorHashPayload {
            reserve_id,
            account_id,
            device_id,
            offline_public_key,
            asset_definition_id,
            balance,
            parked_balance,
            server_revision,
            pending_local_revision,
            authorization_id,
        },
    )?))
}

fn issuer(app: &AppState) -> Result<&OfflineIssuerSigner, Error> {
    app.offline_issuer.as_ref().ok_or_else(|| {
        conversion_error("torii.offline_issuer must be configured for reserve routes".to_owned())
    })
}

fn operator_authority(issuer: &OfflineIssuerSigner) -> Result<AccountId, Error> {
    issuer.operator_authority.clone().ok_or_else(|| {
        conversion_error(
            "torii.offline_issuer.operator_authority must be configured for reserve routes"
                .to_owned(),
        )
    })
}

fn controller_asset_id(account_id: &str, asset_definition_id: &str) -> Result<AssetId, Error> {
    let definition = asset_definition_id
        .trim()
        .parse::<AssetDefinitionId>()
        .map_err(|err| conversion_error(format!("invalid asset_definition_id: {err}")))?;
    let authority = AccountId::parse_encoded(account_id.trim())
        .map_err(|err| conversion_error(format!("invalid account_id: {err}")))?
        .into_account_id();
    Ok(AssetId::new(definition, authority))
}

async fn submit_signed_instruction(
    app: &AppState,
    authority: AccountId,
    private_key: PrivateKey,
    instruction: InstructionBox,
    endpoint: &'static str,
) -> Result<(), Error> {
    let tx = TransactionBuilder::new((*app.chain_id).clone(), authority)
        .with_instructions([instruction])
        .sign(&private_key);
    routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry_handle(),
        endpoint,
    )
    .await?;
    Ok(())
}

fn issuer_public_key_base64(issuer: &OfflineIssuerSigner) -> String {
    let (_, public_key_bytes) = issuer.operator_keypair.public_key().to_bytes();
    BASE64_STANDARD.encode(public_key_bytes)
}

fn sign_base64(issuer: &OfflineIssuerSigner, payload: &[u8]) -> String {
    let signature = Signature::new(issuer.operator_keypair.private_key(), payload);
    BASE64_STANDARD.encode(signature.payload())
}

fn parse_amount(raw: &str) -> Result<Numeric, Error> {
    let amount = parse_numeric(raw)?;
    if amount <= Numeric::zero() {
        return Err(conversion_error(
            "offline reserve amount must be greater than zero".to_owned(),
        ));
    }
    Ok(amount)
}

fn parse_numeric(raw: &str) -> Result<Numeric, Error> {
    let normalized = raw.trim().replace(',', "");
    if normalized.is_empty() {
        return Err(conversion_error(
            "offline reserve amount is required".to_owned(),
        ));
    }
    normalized
        .parse::<Numeric>()
        .map_err(|err| conversion_error(format!("invalid offline reserve amount: {err}")))
}

fn add_amounts(lhs: &str, rhs: &str) -> Result<String, Error> {
    let left = parse_numeric(lhs)?;
    let right = parse_numeric(rhs)?;
    Ok(canonical_amount_string(
        &left
            .checked_add(right)
            .ok_or_else(|| conversion_error("offline reserve amount overflow".to_owned()))?,
    ))
}

fn subtract_amounts(lhs: &str, rhs: &str) -> Result<String, Error> {
    let left = parse_numeric(lhs)?;
    let right = parse_numeric(rhs)?;
    Ok(canonical_amount_string(
        &left
            .checked_sub(right)
            .ok_or_else(|| conversion_error("insufficient offline reserve balance".to_owned()))?,
    ))
}

fn compare_amounts(lhs: &str, rhs: &str) -> Result<std::cmp::Ordering, Error> {
    let left = parse_numeric(lhs)?;
    let right = parse_numeric(rhs)?;
    Ok(left.cmp(&right))
}

fn minimum_required_parked_balance(
    total_balance: &str,
    authorization: Option<&OfflineSpendAuthorization>,
    now_ms: u64,
) -> Result<String, Error> {
    let canonical_total = canonical_amount_string(&parse_numeric(total_balance)?);
    let Some(authorization) = authorization else {
        return Ok(canonical_total);
    };
    if now_ms < authorization.issued_at_ms || now_ms > authorization.expires_at_ms {
        return Ok(canonical_total);
    }
    if compare_amounts(&canonical_total, &authorization.max_balance)?.is_le() {
        return Ok("0".to_owned());
    }
    subtract_amounts(&canonical_total, &authorization.max_balance)
}

fn validate_parked_continuity(
    receipt: &OfflineTransferReceipt,
    expected_post_balance: &str,
) -> Result<(), Error> {
    let authorization = receipt.authorization.as_ref().ok_or_else(|| {
        conversion_error("offline transfer receipt is missing an authorization snapshot".to_owned())
    })?;
    let minimum_pre_parked = minimum_required_parked_balance(
        &receipt.pre_balance,
        Some(authorization),
        receipt.created_at_ms,
    )?;
    let minimum_post_parked = minimum_required_parked_balance(
        expected_post_balance,
        Some(authorization),
        receipt.created_at_ms,
    )?;
    match receipt.direction.as_str() {
        "outgoing" => {
            if receipt.pre_parked_balance != minimum_pre_parked
                || receipt.post_parked_balance != minimum_post_parked
            {
                return Err(conversion_error(
                    "offline reserve parked-balance continuity is invalid".to_owned(),
                ));
            }
        }
        "incoming" => {
            if compare_amounts(&receipt.pre_parked_balance, &minimum_pre_parked)?.is_lt()
                || compare_amounts(&receipt.post_parked_balance, &minimum_post_parked)?.is_lt()
                || compare_amounts(&receipt.pre_parked_balance, &receipt.pre_balance)?.is_gt()
                || compare_amounts(&receipt.post_parked_balance, expected_post_balance)?.is_gt()
            {
                return Err(conversion_error(
                    "offline reserve parked-balance continuity is invalid".to_owned(),
                ));
            }
        }
        _ => {
            return Err(conversion_error(
                "offline receipt direction must be incoming or outgoing".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_receipt_authorization(
    receipt: &OfflineTransferReceipt,
    requires_active_authorization: bool,
    issuer_public_key_base64: &str,
    revoked_verdict_ids: &BTreeSet<String>,
) -> Result<(), Error> {
    let authorization = receipt.authorization.as_ref().ok_or_else(|| {
        conversion_error("offline transfer receipt is missing an authorization snapshot".to_owned())
    })?;
    validate_issuer_signature(
        authorization_unsigned_payload(authorization)?,
        &authorization.issuer_signature_base64,
        issuer_public_key_base64,
    )?;
    if authorization.reserve_id != receipt.reserve_id
        || authorization.account_id != receipt.account_id
        || authorization.device_id != receipt.device_id
        || authorization.offline_public_key != receipt.offline_public_key
        || authorization.app_attest_key_id != receipt.attestation.key_id
    {
        return Err(conversion_error(
            "offline transfer authorization does not match the sender reserve lineage".to_owned(),
        ));
    }
    if revoked_verdict_ids.contains(&authorization.verdict_id.to_lowercase()) {
        return Err(conversion_error(
            "offline transfer authorization has been revoked".to_owned(),
        ));
    }
    if requires_active_authorization {
        if receipt.created_at_ms < authorization.issued_at_ms
            || receipt.created_at_ms > authorization.expires_at_ms
        {
            return Err(conversion_error(
                "offline transfer authorization is expired".to_owned(),
            ));
        }
        if compare_amounts(&receipt.amount, &authorization.max_tx_value)?.is_gt() {
            return Err(conversion_error(
                "offline transfer exceeds the sender authorization policy".to_owned(),
            ));
        }
    }
    Ok(())
}

fn setup_challenge_payload(req: &OfflineReserveSetupRequest) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&ReserveSetupAttestationPayload {
        account_id: &req.account_id,
        device_id: &req.device_id,
        offline_public_key: &req.offline_public_key,
    })
}

fn validate_setup_attestation(
    req: &OfflineReserveSetupRequest,
    counter_book: &mut BTreeMap<String, u64>,
    expected_app_attest_key_id: &str,
) -> Result<(), Error> {
    validate_reserve_attestation(
        &req.account_id,
        "setup",
        "setup",
        setup_challenge_payload(req)?,
        expected_app_attest_key_id,
        &req.attestation,
        counter_book,
    )
}

fn validate_reserve_attestation(
    account_id: &str,
    reserve_id: &str,
    operation: &str,
    payload: Vec<u8>,
    expected_app_attest_key_id: &str,
    attestation: &OfflineDeviceAttestation,
    counter_book: &mut BTreeMap<String, u64>,
) -> Result<(), Error> {
    if attestation.key_id != expected_app_attest_key_id {
        return Err(conversion_error(
            "app_attest_key_id does not match the attestation proof".to_owned(),
        ));
    }
    let challenge_seed = canonical_json_bytes(&AttestationChallengePayload {
        account_id,
        reserve_id,
        operation,
        payload_hash: &sha256_hex(&payload),
    })?;
    if attestation.challenge_hash_hex != sha256_hex(&challenge_seed) {
        return Err(conversion_error(
            "offline reserve attestation challenge hash is invalid".to_owned(),
        ));
    }
    if extract_assertion_counter(&attestation.assertion_base64)? != attestation.counter {
        return Err(conversion_error(
            "offline reserve attestation counter does not match assertion data".to_owned(),
        ));
    }
    validate_counter(attestation, counter_book)
}

fn extract_assertion_counter(assertion_base64: &str) -> Result<u64, Error> {
    let bytes = BASE64_STANDARD
        .decode(assertion_base64)
        .map_err(|err| conversion_error(format!("invalid base64 attestation assertion: {err}")))?;
    let value: CborValue = from_reader(bytes.as_slice())
        .map_err(|_| conversion_error("attestation assertion must be CBOR".to_owned()))?;
    let map = match value {
        CborValue::Map(map) => map,
        _ => {
            return Err(conversion_error(
                "attestation assertion must be a CBOR map".to_owned(),
            ));
        }
    };
    let auth_data = map
        .iter()
        .find_map(|(key, value)| match (key, value) {
            (CborValue::Text(label), CborValue::Bytes(bytes)) if label == "authenticatorData" => {
                Some(bytes.clone())
            }
            _ => None,
        })
        .ok_or_else(|| {
            conversion_error("attestation assertion is missing authenticatorData".to_owned())
        })?;
    if auth_data.len() < 37 {
        return Err(conversion_error(
            "attestation authenticatorData is too short".to_owned(),
        ));
    }
    Ok(u64::from(u32::from_be_bytes(
        auth_data[33..37]
            .try_into()
            .map_err(|_| conversion_error("invalid attestation counter bytes".to_owned()))?,
    )))
}

fn sender_state_key(reserve_id: &str, local_revision: u64) -> String {
    format!("{reserve_id}:{local_revision}")
}

fn reserve_lineage_key(account_id: &str, device_id: &str, offline_public_key: &str) -> String {
    format!("{account_id}\u{1f}{device_id}\u{1f}{offline_public_key}")
}

fn revoked_verdict_ids(app: &AppState) -> BTreeSet<String> {
    app.state
        .world_view()
        .offline_verdict_revocations()
        .iter()
        .map(|(_, record)| hex::encode(record.verdict_id.as_ref()))
        .collect()
}

fn canonical_amount_string(amount: &Numeric) -> String {
    amount.to_string()
}

fn deterministic_id(prefix: &str, fields: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    for field in fields {
        hasher.update(b"|");
        hasher.update(field.as_bytes());
    }
    format!("{prefix}_{}", hex::encode(hasher.finalize()))
}

fn operation_key(kind: &str, operation_id: &str) -> String {
    format!("{kind}:{operation_id}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn decode_transfer_payload(raw_payload: &str) -> Result<OfflineOutgoingTransferPayload, Error> {
    let trimmed = raw_payload.trim();
    let encoded = trimmed
        .strip_prefix(TRANSFER_PREFIX)
        .unwrap_or(trimmed)
        .trim();

    if let Ok(decoded) = decode_base64url(encoded) {
        if let Ok(payload) = json::from_slice::<OfflineOutgoingTransferPayload>(&decoded) {
            return Ok(payload);
        }
    }
    json::from_str::<OfflineOutgoingTransferPayload>(encoded)
        .map_err(|err| conversion_error(format!("invalid offline transfer payload: {err}")))
}

fn decode_base64url(raw: &str) -> Result<Vec<u8>, Error> {
    let mut normalized = raw.replace('-', "+").replace('_', "/");
    while normalized.len() % 4 != 0 {
        normalized.push('=');
    }
    BASE64_STANDARD
        .decode(normalized)
        .map_err(|err| conversion_error(format!("invalid base64url payload: {err}")))
}

fn canonical_json_bytes<T: json::JsonSerialize + ?Sized>(value: &T) -> Result<Vec<u8>, Error> {
    let value = json::to_value(value)
        .map_err(|err| conversion_error(format!("failed to encode canonical JSON: {err}")))?;
    let sorted = sort_json(value);
    json::to_vec(&sorted)
        .map_err(|err| conversion_error(format!("failed to serialize canonical JSON: {err}")))
}

fn sort_json(value: json::Value) -> json::Value {
    match value {
        json::Value::Array(items) => json::Value::Array(items.into_iter().map(sort_json).collect()),
        json::Value::Object(map) => {
            let mut sorted = json::Map::new();
            let mut keys: Vec<_> = map.into_iter().collect();
            keys.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
            for (key, value) in keys {
                sorted.insert(key, sort_json(value));
            }
            json::Value::Object(sorted)
        }
        other => other,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

fn ensure_non_empty(value: &str, field_name: &str) -> Result<(), Error> {
    if value.trim().is_empty() {
        return Err(conversion_error(format!("{field_name} is required")));
    }
    Ok(())
}

fn conversion_error(message: String) -> Error {
    routing::conversion_error(message)
}
