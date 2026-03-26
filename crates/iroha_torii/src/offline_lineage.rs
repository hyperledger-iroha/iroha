#![cfg(feature = "app_api")]

use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
    sync::LazyLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use ciborium::{de::from_reader, value::Value as CborValue};
use ed25519_dalek::{Signature as DalekSignature, Verifier, VerifyingKey};
use iroha_core::{
    queue,
    smartcontracts::isi::offline::{
        LineageAppleAppAttestVerification, verify_lineage_apple_app_attest,
    },
    state::WorldReadOnly,
    zk_stark::{
        STARK_HASH_SHA256_V1, StarkFriParamsV1, StarkVerifyEnvelopeV1,
        synthesize_stark_fri_envelope_bytes,
    },
};
use iroha_crypto::{Hash, PrivateKey, Signature};
use iroha_data_model::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    events::{
        EventBox,
        pipeline::{PipelineEventBox, TransactionStatus},
    },
    isi::offline::{
        CommitOfflineLineageOperation, LoadOfflineEscrowBalance, RedeemOfflineEscrowBalance,
        RegisterOfflineLineage, RegisterOfflineVerdictRevocation,
    },
    metadata::Metadata,
    name::Name,
    offline::{
        OfflineAppleAppAttestBinding as SharedOfflineAppleAppAttestBinding,
        OfflineCashDeviceBinding as SharedOfflineCashDeviceBinding,
        OfflineLineageEnvelope as SharedOfflineLineageEnvelope,
        OfflineLineageOperationResult as SharedOfflineLineageOperationResult,
        OfflineLineageRecord as SharedOfflineLineageRecord,
        OfflineLineageState as SharedOfflineLineageState,
        OfflineMutationSettlement as SharedOfflineMutationSettlement,
        OfflineSpendAuthorization as SharedOfflineSpendAuthorization,
        OfflineTransparentZkProof as SharedOfflineTransparentZkProof, OfflineVerdictRevocation,
        OfflineVerdictRevocationReason,
    },
    prelude::{InstructionBox, Numeric, TransactionBuilder},
    transaction::SignedTransaction,
};
use iroha_version::codec::{DecodeVersioned, EncodeVersioned};
use mv::storage::StorageReadOnly;
use norito::json::{self};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x509_parser::{certificate::X509Certificate, prelude::FromDer, time::ASN1Time};

use crate::{AppState, Error, OfflineIssuerSigner, routing};

const TRANSFER_PREFIX: &str = "wallet-offline-transfer:";

static GOOGLE_ATTESTATION_ROOT_RSA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../certs/google_attestation_root_rsa.der"
));
static GOOGLE_ATTESTATION_ROOT_ECDSA: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../certs/google_attestation_root_ecdsa.der"
));
static ANDROID_ROOT_ANCHORS: LazyLock<Box<[&'static [u8]]>> =
    LazyLock::new(|| Box::new([GOOGLE_ATTESTATION_ROOT_RSA, GOOGLE_ATTESTATION_ROOT_ECDSA]));
const OFFLINE_CASH_TX_COMMIT_TIMEOUT: Duration = Duration::from_secs(5);
const OFFLINE_SETTLEMENT_PROOF_BACKEND: &str = "stark/fri/sha256-goldilocks";
const OFFLINE_SETTLEMENT_CIRCUIT_ID: &str = "offline-bearer-settlement-v1";
const OFFLINE_REDEEM_REQUEST_CIRCUIT_ID: &str = "offline-bearer-redeem-request-v1";
const OFFLINE_STARK_DOMAIN_LOG2: u8 = 4;
const OFFLINE_STARK_BLOWUP_LOG2: u8 = 3;
const OFFLINE_STARK_QUERY_COUNT: u16 = 8;

#[derive(Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct StoredLineage {
    lineage_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    asset_definition_id: String,
    balance: Numeric,
    locked_balance: Numeric,
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
    apple_app_attest_binding: Option<StoredAppleAppAttestBinding>,
}

#[derive(Clone, crate::json_macros::JsonSerialize, crate::json_macros::JsonDeserialize)]
struct StoredAppleAppAttestBinding {
    attestation_report_base64: String,
    ios_team_id: String,
    ios_bundle_id: String,
    ios_environment: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub attestation_report_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub ios_team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub ios_bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub ios_environment: Option<String>,
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
    pub lineage_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub verdict_id: String,
    pub max_balance: String,
    pub max_tx_value: String,
    pub issued_at_ms: u64,
    pub refresh_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub device_binding: Option<OfflineCashAndroidDeviceBinding>,
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
pub struct OfflineLineageState {
    pub lineage_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub asset_definition_id: String,
    pub balance: String,
    pub locked_balance: String,
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
pub struct OfflineLineageEnvelope {
    pub lineage_state: OfflineLineageState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub settlement: Option<OfflineMutationSettlement>,
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
pub struct OfflineTransparentZkProof {
    pub backend: String,
    pub circuit_id: String,
    pub recursion_depth: u8,
    pub public_inputs_hex: String,
    pub envelope: StarkVerifyEnvelopeV1,
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
pub struct OfflineMutationSettlement {
    pub kind: String,
    pub operation_id: String,
    pub chain_tx_hash: String,
    pub entry_hash: String,
    pub block_height: u64,
    pub pre_state_hash: String,
    pub post_state_hash: String,
    pub settlement_commitment_hex: String,
    pub proof: OfflineTransparentZkProof,
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
pub struct OfflineRedeemRequestProof {
    pub backend: String,
    pub circuit_id: String,
    pub recursion_depth: u8,
    pub public_inputs_hex: String,
    pub envelope: StarkVerifyEnvelopeV1,
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
    #[serde(default)]
    #[norito(default)]
    pub blacklisted_account_ids: Vec<String>,
    #[serde(default)]
    #[norito(default)]
    pub asset_send_limits: Vec<OfflineAssetSendLimit>,
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
pub struct OfflineAssetSendLimit {
    pub asset_definition_id: String,
    pub daily_send_limit: String,
    pub monthly_send_limit: String,
}

#[derive(
    Debug,
    Clone,
    Default,
    Serialize,
    Deserialize,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct OfflinePolicySnapshot {
    #[serde(default)]
    #[norito(default)]
    pub blacklisted_account_ids: Vec<String>,
    #[serde(default)]
    #[norito(default)]
    pub asset_send_limits: Vec<OfflineAssetSendLimit>,
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
    pub lineage_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub pre_balance: String,
    pub post_balance: String,
    pub pre_locked_balance: String,
    pub post_locked_balance: String,
    pub pre_state_hash: String,
    pub post_state_hash: String,
    pub local_revision: u64,
    pub counterparty_lineage_id: String,
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
    pub anchor: OfflineLineageState,
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
pub struct OfflineLineageSetupRequest {
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
pub struct OfflineLineageLoadRequest {
    pub operation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub lineage_id: Option<String>,
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
pub struct OfflineLineageRefreshRequest {
    pub operation_id: String,
    pub lineage_id: String,
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
pub struct OfflineLineageSyncRequest {
    pub operation_id: String,
    pub lineage_id: String,
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
pub struct OfflineLineageRedeemRequest {
    pub operation_id: String,
    pub lineage_id: String,
    pub account_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub amount: String,
    pub receipts: Vec<OfflineTransferReceipt>,
    pub redeem_proof: OfflineRedeemRequestProof,
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
pub struct OfflineCashAndroidDeviceBinding {
    pub platform: String,
    pub attestation_key_id: String,
    pub device_id: String,
    pub offline_public_key: String,
    pub attestation_report_base64: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios_team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios_bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ios_environment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, crate::json_macros::JsonDeserialize)]
pub struct OfflineCashAndroidDeviceProof {
    pub platform: String,
    pub attestation_key_id: String,
    pub challenge_hash_hex: String,
    pub assertion_base64: String,
    #[serde(default)]
    pub counter: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum OfflineCashAttestationMode {
    AppleAttest {
        binding: OfflineCashAndroidDeviceBinding,
        proof: OfflineCashAndroidDeviceProof,
    },
    Android {
        binding: OfflineCashAndroidDeviceBinding,
        proof: OfflineCashAndroidDeviceProof,
    },
}

pub(crate) fn offline_cash_device_attestation(
    binding: &OfflineCashAndroidDeviceBinding,
    proof: &OfflineCashAndroidDeviceProof,
) -> Result<OfflineDeviceAttestation, Error> {
    ensure_non_empty(&binding.platform, "device_binding.platform")?;
    ensure_non_empty(
        &binding.attestation_key_id,
        "device_binding.attestation_key_id",
    )?;
    ensure_non_empty(&binding.device_id, "device_binding.device_id")?;
    ensure_non_empty(
        &binding.offline_public_key,
        "device_binding.offline_public_key",
    )?;
    ensure_non_empty(&proof.platform, "device_proof.platform")?;
    ensure_non_empty(&proof.attestation_key_id, "device_proof.attestation_key_id")?;
    ensure_non_empty(&proof.challenge_hash_hex, "device_proof.challenge_hash_hex")?;
    ensure_non_empty(&proof.assertion_base64, "device_proof.assertion_base64")?;
    if binding.attestation_key_id != proof.attestation_key_id {
        return Err(conversion_error(
            "device_proof does not match device_binding".to_owned(),
        ));
    }
    Ok(OfflineDeviceAttestation {
        key_id: proof.attestation_key_id.clone(),
        counter: proof.counter.unwrap_or(0),
        assertion_base64: proof.assertion_base64.clone(),
        challenge_hash_hex: proof.challenge_hash_hex.clone(),
        attestation_report_base64: match binding.attestation_report_base64.trim() {
            "" => None,
            value => Some(value.to_owned()),
        },
        ios_team_id: binding
            .ios_team_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        ios_bundle_id: binding
            .ios_bundle_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        ios_environment: binding
            .ios_environment
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
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
pub struct OfflineLineageRevocationRequest {
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
struct LineageLoadRequestHashPayload<'a> {
    #[norito(default)]
    lineage_id: Option<&'a str>,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    asset_definition_id: &'a str,
    app_attest_key_id: &'a str,
    amount: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LineageRedeemRequestHashPayload<'a> {
    lineage_id: &'a str,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    amount: &'a str,
    receipt_keys: Vec<String>,
    redeem_public_inputs_hex: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct SettlementCommitmentPayload<'a> {
    operation_id: &'a str,
    kind: &'a str,
    account_id: &'a str,
    lineage_id: &'a str,
    asset_definition_id: &'a str,
    amount: &'a str,
    offline_public_key: &'a str,
    authorization_id: &'a str,
    pre_state_hash: &'a str,
    post_state_hash: &'a str,
    chain_tx_hash: &'a str,
    entry_hash: &'a str,
    block_height: u64,
}

#[derive(crate::json_macros::JsonSerialize)]
struct RedeemRequestCommitmentPayload<'a> {
    operation_id: &'a str,
    kind: &'a str,
    account_id: &'a str,
    lineage_id: &'a str,
    asset_definition_id: &'a str,
    amount: &'a str,
    offline_public_key: &'a str,
    authorization_id: &'a str,
    pre_state_hash: &'a str,
    receipt_keys: Vec<String>,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AuthorizationUnsignedPayload<'a> {
    authorization_id: &'a str,
    lineage_id: &'a str,
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
struct CashAuthorizationUnsignedPayload<'a> {
    authorization_id: &'a str,
    lineage_id: &'a str,
    account_id: &'a str,
    verdict_id: &'a str,
    max_balance: &'a str,
    max_tx_value: &'a str,
    issued_at_ms: u64,
    refresh_at_ms: u64,
    expires_at_ms: u64,
    device_binding: &'a OfflineCashAndroidDeviceBinding,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LineageStateUnsignedPayload<'a> {
    lineage_id: &'a str,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    asset_definition_id: &'a str,
    balance: &'a str,
    locked_balance: &'a str,
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
    blacklisted_account_ids: Vec<String>,
    asset_send_limits: Vec<OfflineAssetSendLimit>,
}

#[derive(crate::json_macros::JsonSerialize)]
struct TransferReceiptUnsignedPayload {
    version: i32,
    transfer_id: String,
    direction: String,
    lineage_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    pre_balance: String,
    post_balance: String,
    pre_locked_balance: String,
    post_locked_balance: String,
    pre_state_hash: String,
    post_state_hash: String,
    local_revision: u64,
    counterparty_lineage_id: String,
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

#[derive(Clone, crate::json_macros::JsonSerialize)]
struct CashTransferReceiptAuthorizationPayload {
    authorization_id: String,
    lineage_id: String,
    account_id: String,
    verdict_id: String,
    max_balance: String,
    max_tx_value: String,
    issued_at_ms: u64,
    refresh_at_ms: u64,
    expires_at_ms: u64,
    device_binding: OfflineCashAndroidDeviceBinding,
    issuer_signature_base64: String,
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashTransferReceiptUnsignedPayload {
    version: i32,
    transfer_id: String,
    direction: String,
    lineage_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    pre_balance: String,
    post_balance: String,
    pre_locked_balance: String,
    post_locked_balance: String,
    pre_state_hash: String,
    post_state_hash: String,
    local_revision: u64,
    counterparty_lineage_id: String,
    counterparty_account_id: String,
    counterparty_device_id: String,
    counterparty_offline_public_key: String,
    amount: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    authorization: Option<CashTransferReceiptAuthorizationPayload>,
    attestation: OfflineDeviceAttestation,
    #[norito(skip_serializing_if = "Option::is_none")]
    source_payload: Option<String>,
    created_at_ms: u64,
}

#[derive(Clone, crate::json_macros::JsonSerialize)]
struct LegacyCashTransferReceiptAuthorizationPayload {
    authorization_id: String,
    lineage_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    verdict_id: String,
    max_balance: String,
    max_tx_value: String,
    issued_at_ms: u64,
    refresh_at_ms: u64,
    expires_at_ms: u64,
    app_attest_key_id: String,
    issuer_signature_base64: String,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LegacyCashTransferReceiptUnsignedPayload {
    version: i32,
    transfer_id: String,
    direction: String,
    lineage_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    pre_balance: String,
    post_balance: String,
    pre_locked_balance: String,
    post_locked_balance: String,
    pre_state_hash: String,
    post_state_hash: String,
    local_revision: u64,
    counterparty_lineage_id: String,
    counterparty_account_id: String,
    counterparty_device_id: String,
    counterparty_offline_public_key: String,
    amount: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    authorization: Option<LegacyCashTransferReceiptAuthorizationPayload>,
    attestation: OfflineDeviceAttestation,
    #[norito(skip_serializing_if = "Option::is_none")]
    source_payload: Option<String>,
    created_at_ms: u64,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LocalStateHashPayload<'a> {
    lineage_id: &'a str,
    previous_state_hash: &'a str,
    transfer_id: &'a str,
    direction: &'a str,
    counterparty_lineage_id: &'a str,
    amount: &'a str,
    local_revision: u64,
    post_balance: &'a str,
    post_locked_balance: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashLocalStateHashPayload<'a> {
    lineage_id: &'a str,
    previous_state_hash: &'a str,
    transfer_id: &'a str,
    direction: &'a str,
    counterparty_lineage_id: &'a str,
    amount: &'a str,
    local_revision: u64,
    post_balance: &'a str,
    post_locked_balance: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AttestationSendPayload<'a> {
    lineage_id: &'a str,
    transfer_id: &'a str,
    amount: &'a str,
    receiver_lineage_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashAttestationSendPayload<'a> {
    lineage_id: &'a str,
    transfer_id: &'a str,
    amount: &'a str,
    receiver_lineage_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AttestationReceivePayload<'a> {
    lineage_id: &'a str,
    transfer_id: &'a str,
    amount: &'a str,
    sender_lineage_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashAttestationReceivePayload<'a> {
    lineage_id: &'a str,
    transfer_id: &'a str,
    amount: &'a str,
    sender_lineage_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AttestationChallengePayload<'a> {
    account_id: &'a str,
    lineage_id: &'a str,
    operation: &'a str,
    payload_hash: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashAttestationChallengePayload<'a> {
    account_id: &'a str,
    lineage_id: &'a str,
    operation: &'a str,
    payload_hash: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LineageLoadAttestationPayload<'a> {
    lineage_id: &'a str,
    amount: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LineageRefreshAttestationPayload<'a> {
    lineage_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LineageSetupAttestationPayload<'a> {
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct LineageAnchorHashPayload<'a> {
    lineage_id: &'a str,
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    asset_definition_id: &'a str,
    balance: &'a str,
    locked_balance: &'a str,
    server_revision: u64,
    pending_local_revision: u64,
    authorization_id: &'a str,
}

fn shared_authorization_from_local(
    authorization: &OfflineSpendAuthorization,
) -> SharedOfflineSpendAuthorization {
    SharedOfflineSpendAuthorization {
        authorization_id: authorization.authorization_id.clone(),
        lineage_id: authorization.lineage_id.clone(),
        account_id: authorization.account_id.clone(),
        device_id: authorization.device_id.clone(),
        offline_public_key: authorization.offline_public_key.clone(),
        verdict_id: authorization.verdict_id.clone(),
        max_balance: authorization.max_balance.clone(),
        max_tx_value: authorization.max_tx_value.clone(),
        issued_at_ms: authorization.issued_at_ms,
        refresh_at_ms: authorization.refresh_at_ms,
        expires_at_ms: authorization.expires_at_ms,
        device_binding: authorization
            .device_binding
            .as_ref()
            .map(shared_cash_device_binding_from_local),
        app_attest_key_id: authorization.app_attest_key_id.clone(),
        issuer_signature_base64: authorization.issuer_signature_base64.clone(),
    }
}

fn local_authorization_from_shared(
    authorization: &SharedOfflineSpendAuthorization,
) -> OfflineSpendAuthorization {
    OfflineSpendAuthorization {
        authorization_id: authorization.authorization_id.clone(),
        lineage_id: authorization.lineage_id.clone(),
        account_id: authorization.account_id.clone(),
        device_id: authorization.device_id.clone(),
        offline_public_key: authorization.offline_public_key.clone(),
        verdict_id: authorization.verdict_id.clone(),
        max_balance: authorization.max_balance.clone(),
        max_tx_value: authorization.max_tx_value.clone(),
        issued_at_ms: authorization.issued_at_ms,
        refresh_at_ms: authorization.refresh_at_ms,
        expires_at_ms: authorization.expires_at_ms,
        device_binding: authorization
            .device_binding
            .as_ref()
            .map(local_cash_device_binding_from_shared),
        app_attest_key_id: authorization.app_attest_key_id.clone(),
        issuer_signature_base64: authorization.issuer_signature_base64.clone(),
    }
}

fn shared_cash_device_binding_from_local(
    binding: &OfflineCashAndroidDeviceBinding,
) -> SharedOfflineCashDeviceBinding {
    SharedOfflineCashDeviceBinding {
        platform: binding.platform.clone(),
        attestation_key_id: binding.attestation_key_id.clone(),
        device_id: binding.device_id.clone(),
        offline_public_key: binding.offline_public_key.clone(),
        attestation_report_base64: binding.attestation_report_base64.clone(),
        ios_team_id: binding.ios_team_id.clone(),
        ios_bundle_id: binding.ios_bundle_id.clone(),
        ios_environment: binding.ios_environment.clone(),
    }
}

fn local_cash_device_binding_from_shared(
    binding: &SharedOfflineCashDeviceBinding,
) -> OfflineCashAndroidDeviceBinding {
    OfflineCashAndroidDeviceBinding {
        platform: binding.platform.clone(),
        attestation_key_id: binding.attestation_key_id.clone(),
        device_id: binding.device_id.clone(),
        offline_public_key: binding.offline_public_key.clone(),
        attestation_report_base64: binding.attestation_report_base64.clone(),
        ios_team_id: binding.ios_team_id.clone(),
        ios_bundle_id: binding.ios_bundle_id.clone(),
        ios_environment: binding.ios_environment.clone(),
    }
}

fn shared_settlement_proof_from_local(
    proof: &OfflineTransparentZkProof,
) -> Result<SharedOfflineTransparentZkProof, Error> {
    let envelope_bytes = norito::to_bytes(&proof.envelope)
        .map_err(|err| conversion_error(format!("failed to encode settlement proof: {err}")))?;
    Ok(SharedOfflineTransparentZkProof {
        backend: proof.backend.clone(),
        circuit_id: proof.circuit_id.clone(),
        recursion_depth: proof.recursion_depth,
        public_inputs_hex: proof.public_inputs_hex.clone(),
        envelope_bytes,
    })
}

fn local_settlement_proof_from_shared(
    proof: &SharedOfflineTransparentZkProof,
) -> Result<OfflineTransparentZkProof, Error> {
    let envelope = norito::decode_from_bytes::<StarkVerifyEnvelopeV1>(&proof.envelope_bytes)
        .map_err(|err| conversion_error(format!("failed to decode settlement proof: {err}")))?;
    Ok(OfflineTransparentZkProof {
        backend: proof.backend.clone(),
        circuit_id: proof.circuit_id.clone(),
        recursion_depth: proof.recursion_depth,
        public_inputs_hex: proof.public_inputs_hex.clone(),
        envelope,
    })
}

fn shared_settlement_from_local(
    settlement: &OfflineMutationSettlement,
) -> Result<SharedOfflineMutationSettlement, Error> {
    Ok(SharedOfflineMutationSettlement {
        kind: settlement.kind.clone(),
        operation_id: settlement.operation_id.clone(),
        chain_tx_hash: settlement.chain_tx_hash.clone(),
        entry_hash: settlement.entry_hash.clone(),
        block_height: settlement.block_height,
        pre_state_hash: settlement.pre_state_hash.clone(),
        post_state_hash: settlement.post_state_hash.clone(),
        settlement_commitment_hex: settlement.settlement_commitment_hex.clone(),
        proof: shared_settlement_proof_from_local(&settlement.proof)?,
    })
}

fn local_settlement_from_shared(
    settlement: &SharedOfflineMutationSettlement,
) -> Result<OfflineMutationSettlement, Error> {
    Ok(OfflineMutationSettlement {
        kind: settlement.kind.clone(),
        operation_id: settlement.operation_id.clone(),
        chain_tx_hash: settlement.chain_tx_hash.clone(),
        entry_hash: settlement.entry_hash.clone(),
        block_height: settlement.block_height,
        pre_state_hash: settlement.pre_state_hash.clone(),
        post_state_hash: settlement.post_state_hash.clone(),
        settlement_commitment_hex: settlement.settlement_commitment_hex.clone(),
        proof: local_settlement_proof_from_shared(&settlement.proof)?,
    })
}

fn shared_envelope_from_local(
    envelope: &OfflineLineageEnvelope,
) -> Result<SharedOfflineLineageEnvelope, Error> {
    Ok(SharedOfflineLineageEnvelope {
        lineage_state: SharedOfflineLineageState {
            lineage_id: envelope.lineage_state.lineage_id.clone(),
            account_id: envelope.lineage_state.account_id.clone(),
            device_id: envelope.lineage_state.device_id.clone(),
            offline_public_key: envelope.lineage_state.offline_public_key.clone(),
            asset_definition_id: envelope.lineage_state.asset_definition_id.clone(),
            balance: envelope.lineage_state.balance.clone(),
            locked_balance: envelope.lineage_state.locked_balance.clone(),
            server_revision: envelope.lineage_state.server_revision,
            server_state_hash: envelope.lineage_state.server_state_hash.clone(),
            pending_local_revision: envelope.lineage_state.pending_local_revision,
            authorization: shared_authorization_from_local(&envelope.lineage_state.authorization),
            issuer_signature_base64: envelope.lineage_state.issuer_signature_base64.clone(),
        },
        settlement: envelope
            .settlement
            .as_ref()
            .map(shared_settlement_from_local)
            .transpose()?,
    })
}

fn local_envelope_from_shared(
    envelope: &SharedOfflineLineageEnvelope,
) -> Result<OfflineLineageEnvelope, Error> {
    Ok(OfflineLineageEnvelope {
        lineage_state: OfflineLineageState {
            lineage_id: envelope.lineage_state.lineage_id.clone(),
            account_id: envelope.lineage_state.account_id.clone(),
            device_id: envelope.lineage_state.device_id.clone(),
            offline_public_key: envelope.lineage_state.offline_public_key.clone(),
            asset_definition_id: envelope.lineage_state.asset_definition_id.clone(),
            balance: envelope.lineage_state.balance.clone(),
            locked_balance: envelope.lineage_state.locked_balance.clone(),
            server_revision: envelope.lineage_state.server_revision,
            server_state_hash: envelope.lineage_state.server_state_hash.clone(),
            pending_local_revision: envelope.lineage_state.pending_local_revision,
            authorization: local_authorization_from_shared(&envelope.lineage_state.authorization),
            issuer_signature_base64: envelope.lineage_state.issuer_signature_base64.clone(),
        },
        settlement: envelope
            .settlement
            .as_ref()
            .map(local_settlement_from_shared)
            .transpose()?,
    })
}

fn shared_binding_from_local(
    binding: &StoredAppleAppAttestBinding,
) -> SharedOfflineAppleAppAttestBinding {
    SharedOfflineAppleAppAttestBinding {
        attestation_report_base64: binding.attestation_report_base64.clone(),
        ios_team_id: binding.ios_team_id.clone(),
        ios_bundle_id: binding.ios_bundle_id.clone(),
        ios_environment: binding.ios_environment.clone(),
    }
}

fn local_binding_from_shared(
    binding: &SharedOfflineAppleAppAttestBinding,
) -> StoredAppleAppAttestBinding {
    StoredAppleAppAttestBinding {
        attestation_report_base64: binding.attestation_report_base64.clone(),
        ios_team_id: binding.ios_team_id.clone(),
        ios_bundle_id: binding.ios_bundle_id.clone(),
        ios_environment: binding.ios_environment.clone(),
    }
}

fn shared_record_from_local(
    issuer: &OfflineIssuerSigner,
    record: &StoredLineage,
) -> Result<SharedOfflineLineageRecord, Error> {
    let envelope = envelope_from_record(issuer, record)?;
    Ok(SharedOfflineLineageRecord {
        lineage_state: shared_envelope_from_local(&envelope)?.lineage_state,
        app_attest_key_id: record.app_attest_key_id.clone(),
        counter_book: record.counter_book.clone(),
        seen_transfer_ids: record.seen_transfer_ids.clone(),
        seen_sender_states: record.seen_sender_states.clone(),
        apple_app_attest_binding: record
            .apple_app_attest_binding
            .as_ref()
            .map(shared_binding_from_local),
    })
}

fn local_record_from_shared(record: &SharedOfflineLineageRecord) -> Result<StoredLineage, Error> {
    Ok(StoredLineage {
        lineage_id: record.lineage_state.lineage_id.clone(),
        account_id: record.lineage_state.account_id.clone(),
        device_id: record.lineage_state.device_id.clone(),
        offline_public_key: record.lineage_state.offline_public_key.clone(),
        asset_definition_id: record.lineage_state.asset_definition_id.clone(),
        balance: parse_numeric(&record.lineage_state.balance)?,
        locked_balance: parse_numeric(&record.lineage_state.locked_balance)?,
        server_revision: record.lineage_state.server_revision,
        server_state_hash: record.lineage_state.server_state_hash.clone(),
        pending_local_revision: record.lineage_state.pending_local_revision,
        authorization: local_authorization_from_shared(&record.lineage_state.authorization),
        app_attest_key_id: record.app_attest_key_id.clone(),
        counter_book: record.counter_book.clone(),
        seen_transfer_ids: record.seen_transfer_ids.clone(),
        seen_sender_states: record.seen_sender_states.clone(),
        apple_app_attest_binding: record
            .apple_app_attest_binding
            .as_ref()
            .map(local_binding_from_shared),
    })
}

fn load_shared_lineage(app: &AppState, lineage_id: &str) -> Result<Option<StoredLineage>, Error> {
    app.state
        .world_view()
        .offline_lineages()
        .get(&lineage_id.to_owned())
        .map(local_record_from_shared)
        .transpose()
}

fn load_shared_lineage_by_lineage(
    app: &AppState,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
) -> Result<Option<StoredLineage>, Error> {
    load_shared_lineage(
        app,
        &deterministic_id("lineage", &[account_id, device_id, offline_public_key]),
    )
}

fn lineage_has_active_state(lineage: &StoredLineage) -> bool {
    !lineage.balance.is_zero()
        || !lineage.locked_balance.is_zero()
        || lineage.pending_local_revision != 0
}

fn ensure_no_conflicting_active_lineage(
    app: &AppState,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
) -> Result<(), Error> {
    for (_, shared) in app.state.world_view().offline_lineages().iter() {
        let lineage = local_record_from_shared(shared)?;
        if lineage.account_id != account_id {
            continue;
        }
        if lineage.device_id == device_id && lineage.offline_public_key == offline_public_key {
            continue;
        }
        if lineage_has_active_state(&lineage) {
            return Err(conversion_error(
                "lineage_conflict: offline cash lineage is already bound to a different device"
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn load_operation_result(
    app: &AppState,
    operation_key: &str,
) -> Option<SharedOfflineLineageOperationResult> {
    app.state
        .world_view()
        .offline_lineage_operation_results()
        .get(&operation_key.to_owned())
        .cloned()
}

pub(crate) async fn setup_lineage(
    app: &AppState,
    req: OfflineLineageSetupRequest,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.asset_definition_id, "asset_definition_id")?;
    ensure_canonical_asset_definition_id_literal(&req.asset_definition_id, "asset_definition_id")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;

    let issuer = issuer(app)?;
    if let Some(existing) = load_shared_lineage_by_lineage(
        app,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
    )? {
        validate_lineage_request(
            &existing,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
            Some(&req.asset_definition_id),
            Some(&req.app_attest_key_id),
        )?;
        let mut counter_book = existing.counter_book.clone();
        let _ =
            validate_setup_attestation(app, &req, &mut counter_book, &existing.app_attest_key_id)?;
        return envelope_from_record(issuer, &existing);
    }
    ensure_no_conflicting_active_lineage(
        app,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
    )?;

    let mut record = new_local_lineage(
        issuer,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        &req.asset_definition_id,
        &req.app_attest_key_id,
        None,
    )?;
    record.apple_app_attest_binding = validate_setup_attestation(
        app,
        &req,
        &mut record.counter_book,
        &record.app_attest_key_id,
    )?;
    let envelope = envelope_from_record(issuer, &record)?;
    submit_signed_instruction(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(RegisterOfflineLineage {
            lineage: shared_record_from_local(issuer, &record)?,
        }),
        "/v1/offline/cash/setup",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn load_lineage(
    app: &AppState,
    req: OfflineLineageLoadRequest,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_offline_recursive_stark_ready()?;
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.asset_definition_id, "asset_definition_id")?;
    ensure_canonical_asset_definition_id_literal(&req.asset_definition_id, "asset_definition_id")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;
    let issuer = issuer(app)?;
    let amount = parse_amount(&req.amount)?;
    let amount_string = canonical_amount_string(&amount);
    let operation_key = operation_key("load", &req.operation_id);
    let request_hash_hex = load_request_hash_hex(&req, &amount)?;
    if let Some(existing) = load_operation_result(app, &operation_key) {
        if existing.request_hash_hex != request_hash_hex {
            return Err(conversion_error(
                "offline cash operation_id is already bound to a different request".to_owned(),
            ));
        }
        if existing.envelope.settlement.is_none() {
            return Err(conversion_error(
                "offline cash operation is finalizing its settlement proof; retry".to_owned(),
            ));
        }
        return local_envelope_from_shared(&existing.envelope);
    }

    let mut lineage = if let Some(lineage_id) = req.lineage_id.as_ref() {
        load_shared_lineage(app, lineage_id)?
            .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?
    } else {
        match load_shared_lineage_by_lineage(
            app,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
        )? {
            Some(existing) => existing,
            None => {
                ensure_no_conflicting_active_lineage(
                    app,
                    &req.account_id,
                    &req.device_id,
                    &req.offline_public_key,
                )?;
                new_local_lineage(
                    issuer,
                    &req.account_id,
                    &req.device_id,
                    &req.offline_public_key,
                    &req.asset_definition_id,
                    &req.app_attest_key_id,
                    None,
                )?
            }
        }
    };
    validate_lineage_request(
        &lineage,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        Some(&req.asset_definition_id),
        Some(&req.app_attest_key_id),
    )?;
    let expected_server_revision = lineage.server_revision;
    let expected_state_hash = lineage.server_state_hash.clone();
    validate_lineage_attestation(
        app,
        &req.account_id,
        &lineage.lineage_id,
        "load",
        canonical_json_bytes(&LineageLoadAttestationPayload {
            lineage_id: &lineage.lineage_id,
            amount: &amount_string,
        })?,
        &req.app_attest_key_id,
        &req.attestation,
        &mut lineage.counter_book,
        lineage.apple_app_attest_binding.as_ref(),
    )?;
    lineage.balance = lineage
        .balance
        .clone()
        .checked_add(amount.clone())
        .ok_or_else(|| conversion_error("offline cash balance overflow".to_owned()))?;
    lineage.locked_balance = parse_numeric(&minimum_required_locked_balance(
        &canonical_amount_string(&lineage.balance),
        Some(&lineage.authorization),
        now_ms(),
    )?)?;
    lineage.server_revision = committed_server_revision(expected_server_revision);
    lineage.server_state_hash = lineage_anchor_hash(
        &lineage.lineage_id,
        &lineage.account_id,
        &lineage.device_id,
        &lineage.offline_public_key,
        &lineage.asset_definition_id,
        &canonical_amount_string(&lineage.balance),
        &canonical_amount_string(&lineage.locked_balance),
        lineage.server_revision,
        lineage.pending_local_revision,
        &lineage.authorization.authorization_id,
    )?;
    let post_state_hash = lineage.server_state_hash.clone();
    let mut envelope = envelope_from_record(issuer, &lineage)?;
    let completed_at_ms = now_ms();
    let tx = submit_signed_instructions(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        vec![
            InstructionBox::from(LoadOfflineEscrowBalance {
                asset: controller_asset_id(&req.account_id, &lineage.asset_definition_id)?,
                amount,
            }),
            InstructionBox::from(CommitOfflineLineageOperation {
                expected_server_revision,
                expected_state_hash: expected_state_hash.clone(),
                lineage: shared_record_from_local(issuer, &lineage)?,
                result: SharedOfflineLineageOperationResult {
                    operation_key: operation_key.clone(),
                    kind: "load".to_owned(),
                    request_hash_hex: request_hash_hex.clone(),
                    lineage_id: lineage.lineage_id.clone(),
                    envelope: shared_envelope_from_local(&envelope)?,
                    completed_at_ms,
                },
            }),
        ],
        "/v1/offline/cash/load",
    )
    .await?;
    envelope.settlement = Some(settlement_from_tx(
        "load",
        &req.operation_id,
        &lineage,
        &amount_string,
        &expected_state_hash,
        &post_state_hash,
        &tx,
    )?);
    finalize_operation_result_settlement(
        app,
        issuer,
        &lineage,
        operation_key,
        "load",
        request_hash_hex,
        completed_at_ms,
        &envelope,
        "/v1/offline/cash/load",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn refresh_lineage(
    app: &AppState,
    req: OfflineLineageRefreshRequest,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_offline_recursive_stark_ready()?;
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;
    let issuer = issuer(app)?;
    let operation_key = operation_key("refresh", &req.operation_id);
    let request_hash_hex = refresh_request_hash_hex(&req)?;
    if let Some(existing) = load_operation_result(app, &operation_key) {
        if existing.request_hash_hex != request_hash_hex {
            return Err(conversion_error(
                "offline cash operation_id is already bound to a different request".to_owned(),
            ));
        }
        if existing.envelope.settlement.is_none() {
            return Err(conversion_error(
                "offline cash operation is finalizing its settlement proof; retry".to_owned(),
            ));
        }
        return local_envelope_from_shared(&existing.envelope);
    }

    let mut lineage = load_shared_lineage(app, &req.lineage_id)?
        .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?;
    validate_lineage_request(
        &lineage,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        None,
        Some(&req.app_attest_key_id),
    )?;
    let expected_server_revision = lineage.server_revision;
    let expected_state_hash = lineage.server_state_hash.clone();
    validate_lineage_attestation(
        app,
        &req.account_id,
        &lineage.lineage_id,
        "refresh",
        canonical_json_bytes(&LineageRefreshAttestationPayload {
            lineage_id: &lineage.lineage_id,
        })?,
        &req.app_attest_key_id,
        &req.attestation,
        &mut lineage.counter_book,
        lineage.apple_app_attest_binding.as_ref(),
    )?;
    lineage.authorization = signed_authorization(
        issuer,
        AuthorizationDraft {
            lineage_id: lineage.lineage_id.clone(),
            account_id: lineage.account_id.clone(),
            device_id: lineage.device_id.clone(),
            offline_public_key: lineage.offline_public_key.clone(),
            verdict_id: lineage.authorization.verdict_id.clone(),
            device_binding: None,
            app_attest_key_id: lineage.app_attest_key_id.clone(),
            issued_at_ms: now_ms(),
        },
    )?;
    lineage.locked_balance = parse_numeric(&minimum_required_locked_balance(
        &canonical_amount_string(&lineage.balance),
        Some(&lineage.authorization),
        now_ms(),
    )?)?;
    lineage.server_revision = committed_server_revision(expected_server_revision);
    lineage.server_state_hash = lineage_anchor_hash(
        &lineage.lineage_id,
        &lineage.account_id,
        &lineage.device_id,
        &lineage.offline_public_key,
        &lineage.asset_definition_id,
        &canonical_amount_string(&lineage.balance),
        &canonical_amount_string(&lineage.locked_balance),
        lineage.server_revision,
        lineage.pending_local_revision,
        &lineage.authorization.authorization_id,
    )?;
    let envelope = envelope_from_record(issuer, &lineage)?;
    submit_signed_instruction(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(CommitOfflineLineageOperation {
            expected_server_revision,
            expected_state_hash: expected_state_hash.clone(),
            lineage: shared_record_from_local(issuer, &lineage)?,
            result: SharedOfflineLineageOperationResult {
                operation_key: operation_key.clone(),
                kind: "refresh".to_owned(),
                request_hash_hex,
                lineage_id: lineage.lineage_id.clone(),
                envelope: shared_envelope_from_local(&envelope)?,
                completed_at_ms: now_ms(),
            },
        }),
        "/v1/offline/cash/refresh",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn sync_lineage(
    app: &AppState,
    req: OfflineLineageSyncRequest,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    for receipt in &req.receipts {
        ensure_canonical_transfer_receipt_identifiers(receipt, "receipts")?;
    }
    let issuer = issuer(app)?;
    let operation_key = operation_key("sync", &req.operation_id);
    let request_hash_hex = sync_request_hash_hex(&req)?;
    if let Some(existing) = load_operation_result(app, &operation_key) {
        if existing.request_hash_hex != request_hash_hex {
            return Err(conversion_error(
                "offline cash operation_id is already bound to a different request".to_owned(),
            ));
        }
        if existing.envelope.settlement.is_none() {
            return Err(conversion_error(
                "offline cash operation is finalizing its settlement proof; retry".to_owned(),
            ));
        }
        return local_envelope_from_shared(&existing.envelope);
    }

    let mut lineage = load_shared_lineage(app, &req.lineage_id)?
        .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?;
    validate_lineage_request(
        &lineage,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        None,
        None,
    )?;
    let expected_server_revision = lineage.server_revision;
    let expected_state_hash = lineage.server_state_hash.clone();
    let prior_server_revision = lineage.server_revision;
    let prior_pending_local_revision = lineage.pending_local_revision;
    apply_receipts(app, issuer, &mut lineage, &req.receipts)?;
    if lineage.server_revision == prior_server_revision
        && lineage.pending_local_revision == prior_pending_local_revision
    {
        return envelope_from_record(issuer, &lineage);
    }
    let envelope = envelope_from_record(issuer, &lineage)?;
    submit_signed_instruction(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(CommitOfflineLineageOperation {
            expected_server_revision,
            expected_state_hash: expected_state_hash.clone(),
            lineage: shared_record_from_local(issuer, &lineage)?,
            result: SharedOfflineLineageOperationResult {
                operation_key: operation_key.clone(),
                kind: "sync".to_owned(),
                request_hash_hex,
                lineage_id: lineage.lineage_id.clone(),
                envelope: shared_envelope_from_local(&envelope)?,
                completed_at_ms: now_ms(),
            },
        }),
        "/v1/offline/cash/sync",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn redeem_lineage(
    app: &AppState,
    req: OfflineLineageRedeemRequest,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_offline_recursive_stark_ready()?;
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    for receipt in &req.receipts {
        ensure_canonical_transfer_receipt_identifiers(receipt, "receipts")?;
    }
    let issuer = issuer(app)?;
    let amount = parse_amount(&req.amount)?;
    let amount_string = canonical_amount_string(&amount);
    let operation_key = operation_key("redeem", &req.operation_id);
    let request_hash_hex = redeem_request_hash_hex(&req, &amount)?;
    if let Some(existing) = load_operation_result(app, &operation_key) {
        if existing.request_hash_hex != request_hash_hex {
            return Err(conversion_error(
                "offline cash operation_id is already bound to a different request".to_owned(),
            ));
        }
        return local_envelope_from_shared(&existing.envelope);
    }

    let mut lineage = load_shared_lineage(app, &req.lineage_id)?
        .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?;
    validate_lineage_request(
        &lineage,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        None,
        None,
    )?;
    let expected_server_revision = lineage.server_revision;
    let expected_state_hash = lineage.server_state_hash.clone();
    apply_receipts(app, issuer, &mut lineage, &req.receipts)?;
    let pre_redeem_state_hash = lineage.server_state_hash.clone();
    verify_redeem_request_proof(&req, &lineage, &amount_string, &pre_redeem_state_hash)?;
    lineage.balance = lineage
        .balance
        .clone()
        .checked_sub(amount.clone())
        .ok_or_else(|| {
            conversion_error("insufficient offline cash balance for redeem".to_owned())
        })?;
    lineage.locked_balance = parse_numeric(&minimum_required_locked_balance(
        &canonical_amount_string(&lineage.balance),
        Some(&lineage.authorization),
        now_ms(),
    )?)?;
    lineage.server_revision = committed_server_revision(expected_server_revision);
    lineage.server_state_hash = lineage_anchor_hash(
        &lineage.lineage_id,
        &lineage.account_id,
        &lineage.device_id,
        &lineage.offline_public_key,
        &lineage.asset_definition_id,
        &canonical_amount_string(&lineage.balance),
        &canonical_amount_string(&lineage.locked_balance),
        lineage.server_revision,
        lineage.pending_local_revision,
        &lineage.authorization.authorization_id,
    )?;
    let post_state_hash = lineage.server_state_hash.clone();
    let mut envelope = envelope_from_record(issuer, &lineage)?;
    let completed_at_ms = now_ms();
    let tx = submit_signed_instructions(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        vec![
            InstructionBox::from(RedeemOfflineEscrowBalance {
                asset: controller_asset_id(&req.account_id, &lineage.asset_definition_id)?,
                amount,
            }),
            InstructionBox::from(CommitOfflineLineageOperation {
                expected_server_revision,
                expected_state_hash: expected_state_hash.clone(),
                lineage: shared_record_from_local(issuer, &lineage)?,
                result: SharedOfflineLineageOperationResult {
                    operation_key: operation_key.clone(),
                    kind: "redeem".to_owned(),
                    request_hash_hex: request_hash_hex.clone(),
                    lineage_id: lineage.lineage_id.clone(),
                    envelope: shared_envelope_from_local(&envelope)?,
                    completed_at_ms,
                },
            }),
        ],
        "/v1/offline/cash/redeem",
    )
    .await?;
    envelope.settlement = Some(settlement_from_tx(
        "redeem",
        &req.operation_id,
        &lineage,
        &amount_string,
        &pre_redeem_state_hash,
        &post_state_hash,
        &tx,
    )?);
    finalize_operation_result_settlement(
        app,
        issuer,
        &lineage,
        operation_key,
        "redeem",
        request_hash_hex,
        completed_at_ms,
        &envelope,
        "/v1/offline/cash/redeem",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn setup_cash(
    app: &AppState,
    req: OfflineLineageSetupRequest,
    mode: OfflineCashAttestationMode,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.asset_definition_id, "asset_definition_id")?;
    ensure_canonical_asset_definition_id_literal(&req.asset_definition_id, "asset_definition_id")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;

    let issuer = issuer(app)?;
    if let Some(existing) = load_shared_lineage_by_lineage(
        app,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
    )? {
        validate_lineage_request(
            &existing,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
            Some(&req.asset_definition_id),
            Some(&req.app_attest_key_id),
        )?;
        let mut counter_book = existing.counter_book.clone();
        match &mode {
            OfflineCashAttestationMode::AppleAttest { binding, proof } => {
                let attestation = offline_cash_device_attestation(binding, proof)?;
                let _ = validate_cash_attestation(
                    app,
                    &req.account_id,
                    "setup",
                    "setup",
                    setup_challenge_payload(&req)?,
                    &existing.app_attest_key_id,
                    &attestation,
                    &mut counter_book,
                    existing.apple_app_attest_binding.as_ref(),
                )?;
            }
            OfflineCashAttestationMode::Android { binding, proof } => {
                validate_android_cash_device_binding(
                    app,
                    &req.account_id,
                    binding,
                    &req.device_id,
                    &req.offline_public_key,
                )?;
                validate_android_cash_operation_proof(
                    &req.account_id,
                    "setup",
                    "setup",
                    setup_challenge_payload(&req)?,
                    binding,
                    proof,
                )?;
            }
        }
        return envelope_from_record(issuer, &existing);
    }
    ensure_no_conflicting_active_lineage(
        app,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
    )?;

    let mut record = new_local_lineage(
        issuer,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        &req.asset_definition_id,
        &req.app_attest_key_id,
        Some(match &mode {
            OfflineCashAttestationMode::AppleAttest { binding, .. }
            | OfflineCashAttestationMode::Android { binding, .. } => binding.clone(),
        }),
    )?;
    record.apple_app_attest_binding = match &mode {
        OfflineCashAttestationMode::AppleAttest { binding, proof } => {
            let attestation = offline_cash_device_attestation(binding, proof)?;
            validate_cash_attestation(
                app,
                &req.account_id,
                "setup",
                "setup",
                setup_challenge_payload(&req)?,
                &record.app_attest_key_id,
                &attestation,
                &mut record.counter_book,
                None,
            )?
        }
        OfflineCashAttestationMode::Android { binding, proof } => {
            validate_android_cash_device_binding(
                app,
                &req.account_id,
                binding,
                &req.device_id,
                &req.offline_public_key,
            )?;
            validate_android_cash_operation_proof(
                &req.account_id,
                "setup",
                "setup",
                setup_challenge_payload(&req)?,
                binding,
                proof,
            )?;
            None
        }
    };
    let envelope = envelope_from_record(issuer, &record)?;
    submit_signed_instruction(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(RegisterOfflineLineage {
            lineage: shared_record_from_local(issuer, &record)?,
        }),
        "/v1/offline/cash/setup",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn load_cash(
    app: &AppState,
    req: OfflineLineageLoadRequest,
    mode: OfflineCashAttestationMode,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_offline_recursive_stark_ready()?;
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.asset_definition_id, "asset_definition_id")?;
    ensure_canonical_asset_definition_id_literal(&req.asset_definition_id, "asset_definition_id")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;
    let issuer = issuer(app)?;
    let amount = parse_amount(&req.amount)?;
    let amount_string = canonical_amount_string(&amount);
    let operation_key = operation_key("load", &req.operation_id);
    let request_hash_hex = load_request_hash_hex(&req, &amount)?;
    if let Some(existing) = load_operation_result(app, &operation_key) {
        if existing.request_hash_hex != request_hash_hex {
            return Err(conversion_error(
                "offline cash operation_id is already bound to a different request".to_owned(),
            ));
        }
        return local_envelope_from_shared(&existing.envelope);
    }

    let mut lineage = if let Some(lineage_id) = req.lineage_id.as_ref() {
        load_shared_lineage(app, lineage_id)?
            .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?
    } else {
        match load_shared_lineage_by_lineage(
            app,
            &req.account_id,
            &req.device_id,
            &req.offline_public_key,
        )? {
            Some(existing) => existing,
            None => {
                ensure_no_conflicting_active_lineage(
                    app,
                    &req.account_id,
                    &req.device_id,
                    &req.offline_public_key,
                )?;
                new_local_lineage(
                    issuer,
                    &req.account_id,
                    &req.device_id,
                    &req.offline_public_key,
                    &req.asset_definition_id,
                    &req.app_attest_key_id,
                    Some(match &mode {
                        OfflineCashAttestationMode::AppleAttest { binding, .. }
                        | OfflineCashAttestationMode::Android { binding, .. } => binding.clone(),
                    }),
                )?
            }
        }
    };
    validate_lineage_request(
        &lineage,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        Some(&req.asset_definition_id),
        Some(&req.app_attest_key_id),
    )?;
    let expected_server_revision = lineage.server_revision;
    let expected_state_hash = lineage.server_state_hash.clone();
    let lineage_id = req
        .lineage_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("setup");
    let payload = canonical_json_bytes(&CashLineageAmountPayload {
        lineage_id,
        amount: &amount_string,
    })?;
    match &mode {
        OfflineCashAttestationMode::AppleAttest { binding, proof } => {
            let attestation = offline_cash_device_attestation(binding, proof)?;
            validate_cash_attestation(
                app,
                &req.account_id,
                lineage_id,
                "load",
                payload,
                &req.app_attest_key_id,
                &attestation,
                &mut lineage.counter_book,
                lineage.apple_app_attest_binding.as_ref(),
            )?;
        }
        OfflineCashAttestationMode::Android { binding, proof } => {
            validate_android_cash_device_binding(
                app,
                &req.account_id,
                binding,
                &req.device_id,
                &req.offline_public_key,
            )?;
            validate_android_cash_operation_proof(
                &req.account_id,
                lineage_id,
                "load",
                payload,
                binding,
                proof,
            )?;
        }
    }
    lineage.balance = lineage
        .balance
        .clone()
        .checked_add(amount.clone())
        .ok_or_else(|| conversion_error("offline cash balance overflow".to_owned()))?;
    lineage.locked_balance = parse_numeric(&minimum_required_locked_balance(
        &canonical_amount_string(&lineage.balance),
        Some(&lineage.authorization),
        now_ms(),
    )?)?;
    lineage.server_revision = committed_server_revision(expected_server_revision);
    lineage.server_state_hash = lineage_anchor_hash(
        &lineage.lineage_id,
        &lineage.account_id,
        &lineage.device_id,
        &lineage.offline_public_key,
        &lineage.asset_definition_id,
        &canonical_amount_string(&lineage.balance),
        &canonical_amount_string(&lineage.locked_balance),
        lineage.server_revision,
        lineage.pending_local_revision,
        &lineage.authorization.authorization_id,
    )?;
    let post_state_hash = lineage.server_state_hash.clone();
    let mut envelope = envelope_from_record(issuer, &lineage)?;
    let completed_at_ms = now_ms();
    let tx = submit_signed_instructions(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        vec![
            InstructionBox::from(LoadOfflineEscrowBalance {
                asset: controller_asset_id(&req.account_id, &lineage.asset_definition_id)?,
                amount,
            }),
            InstructionBox::from(CommitOfflineLineageOperation {
                expected_server_revision,
                expected_state_hash: expected_state_hash.clone(),
                lineage: shared_record_from_local(issuer, &lineage)?,
                result: SharedOfflineLineageOperationResult {
                    operation_key: operation_key.clone(),
                    kind: "load".to_owned(),
                    request_hash_hex: request_hash_hex.clone(),
                    lineage_id: lineage.lineage_id.clone(),
                    envelope: shared_envelope_from_local(&envelope)?,
                    completed_at_ms,
                },
            }),
        ],
        "/v1/offline/cash/load",
    )
    .await?;
    envelope.settlement = Some(settlement_from_tx(
        "load",
        &req.operation_id,
        &lineage,
        &amount_string,
        &expected_state_hash,
        &post_state_hash,
        &tx,
    )?);
    finalize_operation_result_settlement(
        app,
        issuer,
        &lineage,
        operation_key,
        "load",
        request_hash_hex,
        completed_at_ms,
        &envelope,
        "/v1/offline/cash/load",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn refresh_cash(
    app: &AppState,
    req: OfflineLineageRefreshRequest,
    mode: OfflineCashAttestationMode,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    ensure_non_empty(&req.app_attest_key_id, "app_attest_key_id")?;
    let issuer = issuer(app)?;
    let operation_key = operation_key("refresh", &req.operation_id);
    let request_hash_hex = refresh_request_hash_hex(&req)?;
    if let Some(existing) = load_operation_result(app, &operation_key) {
        if existing.request_hash_hex != request_hash_hex {
            return Err(conversion_error(
                "offline cash operation_id is already bound to a different request".to_owned(),
            ));
        }
        return local_envelope_from_shared(&existing.envelope);
    }

    let mut lineage = load_shared_lineage(app, &req.lineage_id)?
        .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?;
    validate_lineage_request(
        &lineage,
        &req.account_id,
        &req.device_id,
        &req.offline_public_key,
        None,
        Some(&req.app_attest_key_id),
    )?;
    let expected_server_revision = lineage.server_revision;
    let expected_state_hash = lineage.server_state_hash.clone();
    let payload = canonical_json_bytes(&CashLineagePayload {
        lineage_id: &req.lineage_id,
    })?;
    match &mode {
        OfflineCashAttestationMode::AppleAttest { binding, proof } => {
            let attestation = offline_cash_device_attestation(binding, proof)?;
            validate_cash_attestation(
                app,
                &req.account_id,
                &req.lineage_id,
                "refresh",
                payload,
                &req.app_attest_key_id,
                &attestation,
                &mut lineage.counter_book,
                lineage.apple_app_attest_binding.as_ref(),
            )?;
        }
        OfflineCashAttestationMode::Android { binding, proof } => {
            validate_android_cash_device_binding(
                app,
                &req.account_id,
                binding,
                &req.device_id,
                &req.offline_public_key,
            )?;
            validate_android_cash_operation_proof(
                &req.account_id,
                &req.lineage_id,
                "refresh",
                payload,
                binding,
                proof,
            )?;
        }
    }
    lineage.authorization = signed_authorization(
        issuer,
        AuthorizationDraft {
            lineage_id: lineage.lineage_id.clone(),
            account_id: lineage.account_id.clone(),
            device_id: lineage.device_id.clone(),
            offline_public_key: lineage.offline_public_key.clone(),
            verdict_id: lineage.authorization.verdict_id.clone(),
            device_binding: Some(match &mode {
                OfflineCashAttestationMode::AppleAttest { binding, .. }
                | OfflineCashAttestationMode::Android { binding, .. } => binding.clone(),
            }),
            app_attest_key_id: lineage.app_attest_key_id.clone(),
            issued_at_ms: now_ms(),
        },
    )?;
    lineage.locked_balance = parse_numeric(&minimum_required_locked_balance(
        &canonical_amount_string(&lineage.balance),
        Some(&lineage.authorization),
        now_ms(),
    )?)?;
    lineage.server_revision = committed_server_revision(expected_server_revision);
    lineage.server_state_hash = lineage_anchor_hash(
        &lineage.lineage_id,
        &lineage.account_id,
        &lineage.device_id,
        &lineage.offline_public_key,
        &lineage.asset_definition_id,
        &canonical_amount_string(&lineage.balance),
        &canonical_amount_string(&lineage.locked_balance),
        lineage.server_revision,
        lineage.pending_local_revision,
        &lineage.authorization.authorization_id,
    )?;
    let envelope = envelope_from_record(issuer, &lineage)?;
    submit_signed_instruction(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(CommitOfflineLineageOperation {
            expected_server_revision,
            expected_state_hash: expected_state_hash.clone(),
            lineage: shared_record_from_local(issuer, &lineage)?,
            result: SharedOfflineLineageOperationResult {
                operation_key: operation_key.clone(),
                kind: "refresh".to_owned(),
                request_hash_hex,
                lineage_id: lineage.lineage_id.clone(),
                envelope: shared_envelope_from_local(&envelope)?,
                completed_at_ms: now_ms(),
            },
        }),
        "/v1/offline/cash/refresh",
    )
    .await?;
    Ok(envelope)
}

pub(crate) async fn sync_cash(
    app: &AppState,
    req: OfflineLineageSyncRequest,
    mode: OfflineCashAttestationMode,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    for receipt in &req.receipts {
        ensure_canonical_transfer_receipt_identifiers(receipt, "receipts")?;
    }
    match &mode {
        OfflineCashAttestationMode::AppleAttest { binding, proof } => {
            let mut lineage = load_shared_lineage(app, &req.lineage_id)?
                .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?;
            validate_lineage_request(
                &lineage,
                &req.account_id,
                &req.device_id,
                &req.offline_public_key,
                None,
                Some(&binding.attestation_key_id),
            )?;
            let attestation = offline_cash_device_attestation(binding, proof)?;
            validate_cash_attestation(
                app,
                &req.account_id,
                &req.lineage_id,
                "sync",
                canonical_json_bytes(&CashLineagePayload {
                    lineage_id: &req.lineage_id,
                })?,
                &lineage.app_attest_key_id,
                &attestation,
                &mut lineage.counter_book,
                lineage.apple_app_attest_binding.as_ref(),
            )?;
        }
        OfflineCashAttestationMode::Android { binding, proof } => {
            validate_android_cash_device_binding(
                app,
                &req.account_id,
                binding,
                &req.device_id,
                &req.offline_public_key,
            )?;
            validate_android_cash_operation_proof(
                &req.account_id,
                &req.lineage_id,
                "sync",
                canonical_json_bytes(&CashLineagePayload {
                    lineage_id: &req.lineage_id,
                })?,
                binding,
                proof,
            )?;
        }
    }
    sync_lineage(app, req).await
}

pub(crate) async fn redeem_cash(
    app: &AppState,
    req: OfflineLineageRedeemRequest,
    mode: OfflineCashAttestationMode,
) -> Result<OfflineLineageEnvelope, Error> {
    ensure_non_empty(&req.account_id, "account_id")?;
    ensure_canonical_account_id_literal(&req.account_id, "account_id")?;
    ensure_non_empty(&req.device_id, "device_id")?;
    ensure_non_empty(&req.offline_public_key, "offline_public_key")?;
    for receipt in &req.receipts {
        ensure_canonical_transfer_receipt_identifiers(receipt, "receipts")?;
    }
    match &mode {
        OfflineCashAttestationMode::AppleAttest { binding, proof } => {
            let mut lineage = load_shared_lineage(app, &req.lineage_id)?
                .ok_or_else(|| conversion_error("offline cash lineage not found".to_owned()))?;
            validate_lineage_request(
                &lineage,
                &req.account_id,
                &req.device_id,
                &req.offline_public_key,
                None,
                Some(&binding.attestation_key_id),
            )?;
            let attestation = offline_cash_device_attestation(binding, proof)?;
            validate_cash_attestation(
                app,
                &req.account_id,
                &req.lineage_id,
                "redeem",
                canonical_json_bytes(&CashLineageAmountPayload {
                    lineage_id: &req.lineage_id,
                    amount: &req.amount,
                })?,
                &lineage.app_attest_key_id,
                &attestation,
                &mut lineage.counter_book,
                lineage.apple_app_attest_binding.as_ref(),
            )?;
        }
        OfflineCashAttestationMode::Android { binding, proof } => {
            validate_android_cash_device_binding(
                app,
                &req.account_id,
                binding,
                &req.device_id,
                &req.offline_public_key,
            )?;
            validate_android_cash_operation_proof(
                &req.account_id,
                &req.lineage_id,
                "redeem",
                canonical_json_bytes(&CashLineageAmountPayload {
                    lineage_id: &req.lineage_id,
                    amount: &req.amount,
                })?,
                binding,
                proof,
            )?;
        }
    }
    redeem_lineage(app, req).await
}

fn normalize_offline_policy_snapshot(
    snapshot: OfflinePolicySnapshot,
) -> Result<OfflinePolicySnapshot, Error> {
    let mut blacklisted_account_ids = snapshot
        .blacklisted_account_ids
        .into_iter()
        .map(|value| {
            let authority = AccountId::parse_encoded(value.trim()).map_err(|err| {
                conversion_error(format!("invalid blacklisted_account_id: {err}"))
            })?;
            Ok(authority.into_account_id().to_string())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    blacklisted_account_ids.sort();
    blacklisted_account_ids.dedup();

    let mut asset_send_limits = snapshot
        .asset_send_limits
        .into_iter()
        .map(|item| {
            let asset_definition_id = item
                .asset_definition_id
                .trim()
                .parse::<AssetDefinitionId>()
                .map_err(|err| conversion_error(format!("invalid asset_definition_id: {err}")))?
                .to_string();
            let daily_send_limit = canonical_amount_string(&parse_amount(&item.daily_send_limit)?);
            let monthly_send_limit =
                canonical_amount_string(&parse_amount(&item.monthly_send_limit)?);
            Ok(OfflineAssetSendLimit {
                asset_definition_id,
                daily_send_limit,
                monthly_send_limit,
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    asset_send_limits
        .sort_by(|left, right| left.asset_definition_id.cmp(&right.asset_definition_id));
    asset_send_limits.dedup_by(|left, right| left.asset_definition_id == right.asset_definition_id);

    Ok(OfflinePolicySnapshot {
        blacklisted_account_ids,
        asset_send_limits,
    })
}

pub(crate) fn set_policy_snapshot(
    app: &AppState,
    snapshot: OfflinePolicySnapshot,
) -> Result<OfflinePolicySnapshot, Error> {
    let snapshot = normalize_offline_policy_snapshot(snapshot)?;
    let mut guard = app
        .offline_policy_snapshot
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = snapshot.clone();
    Ok(snapshot)
}

pub(crate) fn policy_snapshot(app: &AppState) -> OfflinePolicySnapshot {
    app.offline_policy_snapshot
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

pub(crate) fn revocation_bundle(app: &AppState) -> Result<OfflineRevocationBundle, Error> {
    let issuer = issuer(app)?;
    let mut verdict_ids = revoked_verdict_ids(app).into_iter().collect::<Vec<_>>();
    verdict_ids.sort();
    verdict_ids.dedup();
    let snapshot = policy_snapshot(app);

    let issued_at_ms = now_ms();
    let expires_at_ms =
        issued_at_ms.saturating_add(issuer.lineage_policy.revocation_ttl.as_millis() as u64);
    let mut bundle = OfflineRevocationBundle {
        issued_at_ms,
        expires_at_ms,
        verdict_ids,
        blacklisted_account_ids: snapshot.blacklisted_account_ids,
        asset_send_limits: snapshot.asset_send_limits,
        issuer_signature_base64: String::new(),
    };
    let signature_payload = canonical_json_bytes(&RevocationBundleUnsignedPayload {
        issued_at_ms: bundle.issued_at_ms,
        expires_at_ms: bundle.expires_at_ms,
        verdict_ids: bundle.verdict_ids.clone(),
        blacklisted_account_ids: bundle.blacklisted_account_ids.clone(),
        asset_send_limits: bundle.asset_send_limits.clone(),
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
    req: OfflineLineageRevocationRequest,
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

fn new_local_lineage(
    issuer: &OfflineIssuerSigner,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: &str,
    app_attest_key_id: &str,
    device_binding: Option<OfflineCashAndroidDeviceBinding>,
) -> Result<StoredLineage, Error> {
    let lineage_id = deterministic_id("lineage", &[account_id, device_id, offline_public_key]);
    let authorization = signed_authorization(
        issuer,
        AuthorizationDraft {
            lineage_id: lineage_id.clone(),
            account_id: account_id.to_owned(),
            device_id: device_id.to_owned(),
            offline_public_key: offline_public_key.to_owned(),
            verdict_id: deterministic_id("verdict", &[account_id, device_id, offline_public_key]),
            device_binding,
            app_attest_key_id: app_attest_key_id.to_owned(),
            issued_at_ms: now_ms(),
        },
    )?;
    Ok(StoredLineage {
        lineage_id: lineage_id.clone(),
        account_id: account_id.to_owned(),
        device_id: device_id.to_owned(),
        offline_public_key: offline_public_key.to_owned(),
        asset_definition_id: asset_definition_id.to_owned(),
        balance: Numeric::zero(),
        locked_balance: Numeric::zero(),
        server_revision: 0,
        server_state_hash: lineage_anchor_hash(
            &lineage_id,
            account_id,
            device_id,
            offline_public_key,
            asset_definition_id,
            "0",
            "0",
            0,
            0,
            &authorization.authorization_id,
        )?,
        pending_local_revision: 0,
        authorization,
        app_attest_key_id: app_attest_key_id.to_owned(),
        counter_book: BTreeMap::new(),
        seen_transfer_ids: BTreeSet::new(),
        seen_sender_states: BTreeSet::new(),
        apple_app_attest_binding: None,
    })
}

fn validate_lineage_request(
    lineage: &StoredLineage,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: Option<&str>,
    app_attest_key_id: Option<&str>,
) -> Result<(), Error> {
    ensure_canonical_account_id_literal(&lineage.account_id, "lineage.account_id")?;
    ensure_canonical_asset_definition_id_literal(
        &lineage.asset_definition_id,
        "lineage.asset_definition_id",
    )?;
    ensure_canonical_account_id_literal(account_id, "account_id")?;
    if lineage.account_id != account_id
        || lineage.device_id != device_id
        || lineage.offline_public_key != offline_public_key
    {
        return Err(conversion_error(
            "lineage_conflict: offline cash lineage does not match the request".to_owned(),
        ));
    }
    if let Some(definition_id) = asset_definition_id {
        ensure_canonical_asset_definition_id_literal(definition_id, "asset_definition_id")?;
        if lineage.asset_definition_id != definition_id {
            return Err(conversion_error(
                "asset_definition_id does not match the offline cash lineage".to_owned(),
            ));
        }
    }
    if let Some(key_id) = app_attest_key_id {
        if lineage.app_attest_key_id != key_id {
            return Err(conversion_error(
                "lineage_conflict: app_attest_key_id does not match the offline cash lineage"
                    .to_owned(),
            ));
        }
    }
    Ok(())
}

fn apply_receipts(
    app: &AppState,
    issuer: &OfflineIssuerSigner,
    lineage: &mut StoredLineage,
    receipts: &[OfflineTransferReceipt],
) -> Result<(), Error> {
    if receipts.is_empty() {
        return Ok(());
    }
    let issuer_public_key = issuer_public_key_base64(issuer);
    let mut current_balance = canonical_amount_string(&lineage.balance);
    let mut current_parked = canonical_amount_string(&lineage.locked_balance);
    let mut current_hash = lineage.server_state_hash.clone();
    let mut current_revision = lineage.pending_local_revision;
    let revoked_verdict_ids = revoked_verdict_ids(app);

    let mut ordered = receipts.to_vec();
    ordered.sort_by_key(|receipt| receipt.local_revision);
    let mut applied_any = false;

    for receipt in ordered {
        if receipt.local_revision <= current_revision {
            continue;
        }
        ensure_canonical_transfer_receipt_identifiers(&receipt, "receipt")?;
        validate_receipt_signature(&receipt)?;
        validate_attestation_hash(&receipt)?;
        validate_counter(&receipt.attestation, &mut lineage.counter_book)?;
        let expected_post_balance = validate_local_continuity(
            &receipt,
            &lineage.lineage_id,
            &lineage.offline_public_key,
            &current_balance,
            &current_parked,
            &current_hash,
            current_revision,
            &issuer_public_key,
            &revoked_verdict_ids,
        )?;
        if !lineage
            .seen_transfer_ids
            .insert(receipt.transfer_id.clone())
        {
            return Err(conversion_error(
                "duplicate transfer_id in offline cash sync".to_owned(),
            ));
        }
        if !lineage.seen_sender_states.insert(sender_state_key(
            &receipt.lineage_id,
            receipt.local_revision,
        )) {
            return Err(conversion_error(
                "duplicate sender state in offline cash sync".to_owned(),
            ));
        }
        current_balance = expected_post_balance;
        current_parked = receipt.post_locked_balance.clone();
        current_hash = receipt.post_state_hash.clone();
        current_revision = receipt.local_revision;
        applied_any = true;
    }

    if applied_any {
        lineage.balance = parse_numeric(&current_balance)?;
        lineage.locked_balance = parse_numeric(&current_parked)?;
        lineage.pending_local_revision = current_revision;
        lineage.server_revision = lineage.server_revision.saturating_add(1);
        lineage.server_state_hash = current_hash;
    }

    Ok(())
}

fn validate_local_continuity(
    receipt: &OfflineTransferReceipt,
    expected_lineage_id: &str,
    expected_offline_public_key: &str,
    current_balance: &str,
    current_parked: &str,
    current_hash: &str,
    current_revision: u64,
    issuer_public_key_base64: &str,
    revoked_verdict_ids: &BTreeSet<String>,
) -> Result<String, Error> {
    if receipt.lineage_id != expected_lineage_id
        || receipt.offline_public_key != expected_offline_public_key
        || receipt.local_revision != current_revision.saturating_add(1)
        || receipt.pre_balance != current_balance
        || receipt.pre_locked_balance != current_parked
        || receipt.pre_state_hash != current_hash
    {
        return Err(conversion_error(
            "offline cash continuity proof is invalid".to_owned(),
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
                &receipt.lineage_id,
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
        &receipt.lineage_id,
        current_hash,
        &receipt.transfer_id,
        &receipt.direction,
        &receipt.counterparty_lineage_id,
        &receipt.amount,
        receipt.local_revision,
        &expected_post_balance,
        &receipt.post_locked_balance,
    )?;
    let expected_cash_post_hash = cash_next_local_state_hash(
        &receipt.lineage_id,
        current_hash,
        &receipt.transfer_id,
        &receipt.direction,
        &receipt.counterparty_lineage_id,
        &receipt.amount,
        receipt.local_revision,
        &expected_post_balance,
        &receipt.post_locked_balance,
    )?;
    if receipt.post_balance != expected_post_balance
        || (receipt.post_state_hash != expected_post_hash
            && receipt.post_state_hash != expected_cash_post_hash)
    {
        return Err(conversion_error(
            "offline cash continuity proof is invalid".to_owned(),
        ));
    }
    Ok(expected_post_balance)
}

fn validate_source_payload(
    raw_payload: &str,
    recipient_lineage_id: &str,
    amount: &str,
    issuer_public_key_base64: &str,
    revoked_verdict_ids: &BTreeSet<String>,
) -> Result<(), Error> {
    let payload = decode_transfer_payload(raw_payload)?;
    ensure_canonical_lineage_state_identifiers(&payload.anchor, "source_payload.anchor")?;
    validate_issuer_signature(
        authorization_unsigned_payload(&payload.anchor.authorization)?,
        &payload.anchor.authorization.issuer_signature_base64,
        issuer_public_key_base64,
    )?;
    validate_issuer_signature(
        lineage_state_unsigned_payload(&payload.anchor)?,
        &payload.anchor.issuer_signature_base64,
        issuer_public_key_base64,
    )?;

    let mut current_balance = payload.anchor.balance.clone();
    let mut current_parked = minimum_required_locked_balance(
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
    for (index, receipt) in ancestry.into_iter().enumerate() {
        ensure_canonical_transfer_receipt_identifiers(
            &receipt,
            &format!("source_payload.ancestry_receipts[{index}]"),
        )?;
        if !seen_sender_states.insert(sender_state_key(
            &receipt.lineage_id,
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
            &payload.anchor.lineage_id,
            &payload.anchor.offline_public_key,
            &current_balance,
            &current_parked,
            &current_hash,
            current_revision,
            issuer_public_key_base64,
            revoked_verdict_ids,
        )?;
        current_parked = receipt.post_locked_balance.clone();
        current_hash = receipt.post_state_hash.clone();
        current_revision = receipt.local_revision;
    }

    if !seen_sender_states.insert(sender_state_key(
        &payload.receipt.lineage_id,
        payload.receipt.local_revision,
    )) {
        return Err(conversion_error(
            "duplicate sender state in outgoing payload".to_owned(),
        ));
    }
    ensure_canonical_transfer_receipt_identifiers(&payload.receipt, "source_payload.receipt")?;
    validate_receipt_signature(&payload.receipt)?;
    validate_attestation_hash(&payload.receipt)?;
    validate_counter(&payload.receipt.attestation, &mut counter_book)?;
    let _ = validate_local_continuity(
        &payload.receipt,
        &payload.anchor.lineage_id,
        &payload.anchor.offline_public_key,
        &current_balance,
        &current_parked,
        &current_hash,
        current_revision,
        issuer_public_key_base64,
        revoked_verdict_ids,
    )?;
    if payload.receipt.direction != "outgoing"
        || payload.receipt.counterparty_lineage_id != recipient_lineage_id
        || canonical_amount_string(&parse_amount(&payload.receipt.amount)?)
            != canonical_amount_string(&parse_amount(amount)?)
    {
        return Err(conversion_error(
            "source payload does not target the expected offline cash lineage".to_owned(),
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
    let lineage_transfer_payload = if receipt.direction == "incoming" {
        canonical_json_bytes(&AttestationReceivePayload {
            lineage_id: &receipt.lineage_id,
            transfer_id: &receipt.transfer_id,
            amount: &receipt.amount,
            sender_lineage_id: &receipt.counterparty_lineage_id,
        })?
    } else {
        canonical_json_bytes(&AttestationSendPayload {
            lineage_id: &receipt.lineage_id,
            transfer_id: &receipt.transfer_id,
            amount: &receipt.amount,
            receiver_lineage_id: &receipt.counterparty_lineage_id,
        })?
    };
    let lineage_expected = sha256_hex(&canonical_json_bytes(&AttestationChallengePayload {
        account_id: &receipt.account_id,
        lineage_id: &receipt.lineage_id,
        operation,
        payload_hash: &sha256_hex(&lineage_transfer_payload),
    })?);

    let cash_transfer_payload = if receipt.direction == "incoming" {
        canonical_json_bytes(&CashAttestationReceivePayload {
            lineage_id: &receipt.lineage_id,
            transfer_id: &receipt.transfer_id,
            amount: &receipt.amount,
            sender_lineage_id: &receipt.counterparty_lineage_id,
        })?
    } else {
        canonical_json_bytes(&CashAttestationSendPayload {
            lineage_id: &receipt.lineage_id,
            transfer_id: &receipt.transfer_id,
            amount: &receipt.amount,
            receiver_lineage_id: &receipt.counterparty_lineage_id,
        })?
    };
    let cash_expected = sha256_hex(&canonical_json_bytes(&CashAttestationChallengePayload {
        account_id: &receipt.account_id,
        lineage_id: &receipt.lineage_id,
        operation,
        payload_hash: &sha256_hex(&cash_transfer_payload),
    })?);

    if receipt.attestation.challenge_hash_hex != lineage_expected
        && receipt.attestation.challenge_hash_hex != cash_expected
    {
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
    let lineage_payload = transfer_receipt_unsigned_payload(receipt)?;
    if validate_signature(
        "offline receipt sender signature",
        &lineage_payload,
        &receipt.sender_signature_base64,
        &receipt.offline_public_key,
    )
    .is_ok()
    {
        return Ok(());
    }
    let cash_payload = cash_transfer_receipt_unsigned_payload(receipt)?;
    match validate_signature(
        "offline receipt sender signature",
        &cash_payload,
        &receipt.sender_signature_base64,
        &receipt.offline_public_key,
    ) {
        Ok(()) => Ok(()),
        Err(err) => Err(conversion_error(format!(
            "invalid offline receipt sender signature: transfer_id={} direction={} local_revision={} lineage_payload_hash={} cash_payload_hash={} reason={err}",
            receipt.transfer_id,
            receipt.direction,
            receipt.local_revision,
            sha256_hex(&lineage_payload),
            sha256_hex(&cash_payload),
        ))),
    }
}

fn validate_signature(
    label: &str,
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
        .map_err(|err| conversion_error(format!("invalid {label}: {err}")))?;
    Ok(())
}

fn validate_issuer_signature(
    payload: Vec<u8>,
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<(), Error> {
    validate_signature(
        "offline issuer signature",
        &payload,
        signature_base64,
        public_key_base64,
    )
}

fn transfer_receipt_unsigned_payload(receipt: &OfflineTransferReceipt) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&TransferReceiptUnsignedPayload {
        version: receipt.version,
        transfer_id: receipt.transfer_id.clone(),
        direction: receipt.direction.clone(),
        lineage_id: receipt.lineage_id.clone(),
        account_id: receipt.account_id.clone(),
        device_id: receipt.device_id.clone(),
        offline_public_key: receipt.offline_public_key.clone(),
        pre_balance: canonical_amount_string(&parse_numeric(&receipt.pre_balance)?),
        post_balance: canonical_amount_string(&parse_numeric(&receipt.post_balance)?),
        pre_locked_balance: canonical_amount_string(&parse_numeric(&receipt.pre_locked_balance)?),
        post_locked_balance: canonical_amount_string(&parse_numeric(&receipt.post_locked_balance)?),
        pre_state_hash: receipt.pre_state_hash.clone(),
        post_state_hash: receipt.post_state_hash.clone(),
        local_revision: receipt.local_revision,
        counterparty_lineage_id: receipt.counterparty_lineage_id.clone(),
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

fn cash_transfer_receipt_unsigned_payload(
    receipt: &OfflineTransferReceipt,
) -> Result<Vec<u8>, Error> {
    if let Some(authorization) = receipt.authorization.as_ref() {
        if let Some(device_binding) = authorization.device_binding.clone() {
            return canonical_json_bytes(&CashTransferReceiptUnsignedPayload {
                version: receipt.version,
                transfer_id: receipt.transfer_id.clone(),
                direction: receipt.direction.clone(),
                lineage_id: receipt.lineage_id.clone(),
                account_id: receipt.account_id.clone(),
                device_id: receipt.device_id.clone(),
                offline_public_key: receipt.offline_public_key.clone(),
                pre_balance: canonical_amount_string(&parse_numeric(&receipt.pre_balance)?),
                post_balance: canonical_amount_string(&parse_numeric(&receipt.post_balance)?),
                pre_locked_balance: canonical_amount_string(&parse_numeric(
                    &receipt.pre_locked_balance,
                )?),
                post_locked_balance: canonical_amount_string(&parse_numeric(
                    &receipt.post_locked_balance,
                )?),
                pre_state_hash: receipt.pre_state_hash.clone(),
                post_state_hash: receipt.post_state_hash.clone(),
                local_revision: receipt.local_revision,
                counterparty_lineage_id: receipt.counterparty_lineage_id.clone(),
                counterparty_account_id: receipt.counterparty_account_id.clone(),
                counterparty_device_id: receipt.counterparty_device_id.clone(),
                counterparty_offline_public_key: receipt.counterparty_offline_public_key.clone(),
                amount: canonical_amount_string(&parse_amount(&receipt.amount)?),
                authorization: Some(CashTransferReceiptAuthorizationPayload {
                    authorization_id: authorization.authorization_id.clone(),
                    lineage_id: authorization.lineage_id.clone(),
                    account_id: authorization.account_id.clone(),
                    verdict_id: authorization.verdict_id.clone(),
                    max_balance: canonical_amount_string(&parse_amount(
                        &authorization.max_balance,
                    )?),
                    max_tx_value: canonical_amount_string(&parse_amount(
                        &authorization.max_tx_value,
                    )?),
                    issued_at_ms: authorization.issued_at_ms,
                    refresh_at_ms: authorization.refresh_at_ms,
                    expires_at_ms: authorization.expires_at_ms,
                    device_binding,
                    issuer_signature_base64: authorization.issuer_signature_base64.clone(),
                }),
                attestation: receipt.attestation.clone(),
                source_payload: receipt.source_payload.clone(),
                created_at_ms: receipt.created_at_ms,
            });
        }
    }
    let authorization = receipt
        .authorization
        .as_ref()
        .map(
            |authorization| -> Result<LegacyCashTransferReceiptAuthorizationPayload, Error> {
                Ok(LegacyCashTransferReceiptAuthorizationPayload {
                    authorization_id: authorization.authorization_id.clone(),
                    lineage_id: authorization.lineage_id.clone(),
                    account_id: authorization.account_id.clone(),
                    device_id: authorization.device_id.clone(),
                    offline_public_key: authorization.offline_public_key.clone(),
                    verdict_id: authorization.verdict_id.clone(),
                    max_balance: canonical_amount_string(&parse_amount(
                        &authorization.max_balance,
                    )?),
                    max_tx_value: canonical_amount_string(&parse_amount(
                        &authorization.max_tx_value,
                    )?),
                    issued_at_ms: authorization.issued_at_ms,
                    refresh_at_ms: authorization.refresh_at_ms,
                    expires_at_ms: authorization.expires_at_ms,
                    app_attest_key_id: authorization.app_attest_key_id.clone(),
                    issuer_signature_base64: authorization.issuer_signature_base64.clone(),
                })
            },
        )
        .transpose()?;
    canonical_json_bytes(&LegacyCashTransferReceiptUnsignedPayload {
        version: receipt.version,
        transfer_id: receipt.transfer_id.clone(),
        direction: receipt.direction.clone(),
        lineage_id: receipt.lineage_id.clone(),
        account_id: receipt.account_id.clone(),
        device_id: receipt.device_id.clone(),
        offline_public_key: receipt.offline_public_key.clone(),
        pre_balance: canonical_amount_string(&parse_numeric(&receipt.pre_balance)?),
        post_balance: canonical_amount_string(&parse_numeric(&receipt.post_balance)?),
        pre_locked_balance: canonical_amount_string(&parse_numeric(&receipt.pre_locked_balance)?),
        post_locked_balance: canonical_amount_string(&parse_numeric(&receipt.post_locked_balance)?),
        pre_state_hash: receipt.pre_state_hash.clone(),
        post_state_hash: receipt.post_state_hash.clone(),
        local_revision: receipt.local_revision,
        counterparty_lineage_id: receipt.counterparty_lineage_id.clone(),
        counterparty_account_id: receipt.counterparty_account_id.clone(),
        counterparty_device_id: receipt.counterparty_device_id.clone(),
        counterparty_offline_public_key: receipt.counterparty_offline_public_key.clone(),
        amount: canonical_amount_string(&parse_amount(&receipt.amount)?),
        authorization,
        attestation: receipt.attestation.clone(),
        source_payload: receipt.source_payload.clone(),
        created_at_ms: receipt.created_at_ms,
    })
}

fn authorization_unsigned_payload(
    authorization: &OfflineSpendAuthorization,
) -> Result<Vec<u8>, Error> {
    if let Some(device_binding) = authorization.device_binding.as_ref() {
        return canonical_json_bytes(&CashAuthorizationUnsignedPayload {
            authorization_id: &authorization.authorization_id,
            lineage_id: &authorization.lineage_id,
            account_id: &authorization.account_id,
            verdict_id: &authorization.verdict_id,
            max_balance: &canonical_amount_string(&parse_amount(&authorization.max_balance)?),
            max_tx_value: &canonical_amount_string(&parse_amount(&authorization.max_tx_value)?),
            issued_at_ms: authorization.issued_at_ms,
            refresh_at_ms: authorization.refresh_at_ms,
            expires_at_ms: authorization.expires_at_ms,
            device_binding,
        });
    }
    canonical_json_bytes(&AuthorizationUnsignedPayload {
        authorization_id: &authorization.authorization_id,
        lineage_id: &authorization.lineage_id,
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

fn lineage_state_unsigned_payload(lineage_state: &OfflineLineageState) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&LineageStateUnsignedPayload {
        lineage_id: &lineage_state.lineage_id,
        account_id: &lineage_state.account_id,
        device_id: &lineage_state.device_id,
        offline_public_key: &lineage_state.offline_public_key,
        asset_definition_id: &lineage_state.asset_definition_id,
        balance: &canonical_amount_string(&parse_numeric(&lineage_state.balance)?),
        locked_balance: &canonical_amount_string(&parse_numeric(&lineage_state.locked_balance)?),
        server_revision: lineage_state.server_revision,
        server_state_hash: &lineage_state.server_state_hash,
        pending_local_revision: lineage_state.pending_local_revision,
        authorization_id: &lineage_state.authorization.authorization_id,
    })
}

fn next_local_state_hash(
    lineage_id: &str,
    previous_state_hash: &str,
    transfer_id: &str,
    direction: &str,
    counterparty_lineage_id: &str,
    amount: &str,
    local_revision: u64,
    post_balance: &str,
    post_locked_balance: &str,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(&LocalStateHashPayload {
        lineage_id,
        previous_state_hash,
        transfer_id,
        direction,
        counterparty_lineage_id,
        amount: &canonical_amount_string(&parse_amount(amount)?),
        local_revision,
        post_balance: &canonical_amount_string(&parse_numeric(post_balance)?),
        post_locked_balance: &canonical_amount_string(&parse_numeric(post_locked_balance)?),
    })?))
}

fn cash_next_local_state_hash(
    lineage_id: &str,
    previous_state_hash: &str,
    transfer_id: &str,
    direction: &str,
    counterparty_lineage_id: &str,
    amount: &str,
    local_revision: u64,
    post_balance: &str,
    post_locked_balance: &str,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &CashLocalStateHashPayload {
            lineage_id,
            previous_state_hash,
            transfer_id,
            direction,
            counterparty_lineage_id,
            amount: &canonical_amount_string(&parse_amount(amount)?),
            local_revision,
            post_balance: &canonical_amount_string(&parse_numeric(post_balance)?),
            post_locked_balance: &canonical_amount_string(&parse_numeric(post_locked_balance)?),
        },
    )?))
}

fn envelope_from_record(
    issuer: &OfflineIssuerSigner,
    record: &StoredLineage,
) -> Result<OfflineLineageEnvelope, Error> {
    let mut lineage_state = OfflineLineageState {
        lineage_id: record.lineage_id.clone(),
        account_id: record.account_id.clone(),
        device_id: record.device_id.clone(),
        offline_public_key: record.offline_public_key.clone(),
        asset_definition_id: record.asset_definition_id.clone(),
        balance: canonical_amount_string(&record.balance),
        locked_balance: canonical_amount_string(&record.locked_balance),
        server_revision: record.server_revision,
        server_state_hash: record.server_state_hash.clone(),
        pending_local_revision: record.pending_local_revision,
        authorization: record.authorization.clone(),
        issuer_signature_base64: String::new(),
    };
    lineage_state.issuer_signature_base64 =
        sign_base64(issuer, &lineage_state_unsigned_payload(&lineage_state)?);
    Ok(OfflineLineageEnvelope {
        lineage_state,
        settlement: None,
    })
}

struct AuthorizationDraft {
    lineage_id: String,
    account_id: String,
    device_id: String,
    offline_public_key: String,
    verdict_id: String,
    device_binding: Option<OfflineCashAndroidDeviceBinding>,
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
            draft.lineage_id.as_str(),
            draft.account_id.as_str(),
            draft.device_id.as_str(),
            draft.offline_public_key.as_str(),
            draft.verdict_id.as_str(),
            &draft.issued_at_ms.to_string(),
        ],
    );
    let mut authorization = OfflineSpendAuthorization {
        authorization_id,
        lineage_id: draft.lineage_id,
        account_id: draft.account_id,
        device_id: draft.device_id,
        offline_public_key: draft.offline_public_key,
        verdict_id: draft.verdict_id,
        max_balance: canonical_amount_string(&parse_amount(&issuer.lineage_policy.max_balance)?),
        max_tx_value: canonical_amount_string(&parse_amount(&issuer.lineage_policy.max_tx_value)?),
        issued_at_ms: draft.issued_at_ms,
        refresh_at_ms: draft
            .issued_at_ms
            .saturating_add(issuer.lineage_policy.authorization_refresh.as_millis() as u64),
        expires_at_ms: draft
            .issued_at_ms
            .saturating_add(issuer.lineage_policy.authorization_ttl.as_millis() as u64),
        device_binding: draft.device_binding,
        app_attest_key_id: draft.app_attest_key_id,
        issuer_signature_base64: String::new(),
    };
    authorization.issuer_signature_base64 =
        sign_base64(issuer, &authorization_unsigned_payload(&authorization)?);
    Ok(authorization)
}

fn lineage_anchor_hash(
    lineage_id: &str,
    account_id: &str,
    device_id: &str,
    offline_public_key: &str,
    asset_definition_id: &str,
    balance: &str,
    locked_balance: &str,
    server_revision: u64,
    pending_local_revision: u64,
    authorization_id: &str,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &LineageAnchorHashPayload {
            lineage_id,
            account_id,
            device_id,
            offline_public_key,
            asset_definition_id,
            balance,
            locked_balance,
            server_revision,
            pending_local_revision,
            authorization_id,
        },
    )?))
}

fn issuer(app: &AppState) -> Result<&OfflineIssuerSigner, Error> {
    app.offline_issuer.as_ref().ok_or_else(|| {
        conversion_error(
            "torii.offline_issuer must be configured for offline cash routes".to_owned(),
        )
    })
}

fn operator_authority(issuer: &OfflineIssuerSigner) -> Result<AccountId, Error> {
    issuer.operator_authority.clone().ok_or_else(|| {
        conversion_error(
            "torii.offline_issuer.operator_authority must be configured for offline cash routes"
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

#[derive(Debug, Clone)]
struct SubmittedTransactionReceipt {
    chain_tx_hash: String,
    entry_hash: String,
    block_height: u64,
}

pub(crate) fn offline_recursive_stark_ready() -> bool {
    cfg!(feature = "zk-stark")
}

fn ensure_offline_recursive_stark_ready() -> Result<(), Error> {
    if offline_recursive_stark_ready() {
        Ok(())
    } else {
        Err(conversion_error(
            "offline recursive stark proofs are unavailable".to_owned(),
        ))
    }
}

fn offline_stark_params(domain_tag: String) -> StarkFriParamsV1 {
    StarkFriParamsV1 {
        version: 1,
        n_log2: OFFLINE_STARK_DOMAIN_LOG2,
        blowup_log2: OFFLINE_STARK_BLOWUP_LOG2,
        fold_arity: 2,
        queries: OFFLINE_STARK_QUERY_COUNT,
        merkle_arity: 2,
        hash_fn: STARK_HASH_SHA256_V1,
        domain_tag,
    }
}

fn synthesize_stark_envelope(
    domain_tag: String,
    transcript_label: &str,
) -> Result<StarkVerifyEnvelopeV1, Error> {
    let bytes = synthesize_stark_fri_envelope_bytes(
        offline_stark_params(domain_tag),
        transcript_label.to_owned(),
    )
    .map_err(|err| conversion_error(format!("failed to synthesize stark envelope: {err}")))?;
    norito::decode_from_bytes::<StarkVerifyEnvelopeV1>(&bytes)
        .map_err(|err| conversion_error(format!("failed to decode stark envelope: {err}")))
}

fn encode_stark_envelope_bytes(envelope: &StarkVerifyEnvelopeV1) -> Result<Vec<u8>, Error> {
    norito::to_bytes(envelope)
        .map_err(|err| conversion_error(format!("failed to encode stark envelope: {err}")))
}

fn receipt_keys(receipts: &[OfflineTransferReceipt]) -> Vec<String> {
    let mut keys = receipts
        .iter()
        .map(|receipt| format!("{}:{}", receipt.transfer_id, receipt.local_revision))
        .collect::<Vec<_>>();
    keys.sort();
    keys
}

fn settlement_commitment_hex(
    operation_id: &str,
    kind: &str,
    lineage: &StoredLineage,
    amount: &str,
    pre_state_hash: &str,
    post_state_hash: &str,
    tx: &SubmittedTransactionReceipt,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &SettlementCommitmentPayload {
            operation_id,
            kind,
            account_id: &lineage.account_id,
            lineage_id: &lineage.lineage_id,
            asset_definition_id: &lineage.asset_definition_id,
            amount,
            offline_public_key: &lineage.offline_public_key,
            authorization_id: &lineage.authorization.authorization_id,
            pre_state_hash,
            post_state_hash,
            chain_tx_hash: &tx.chain_tx_hash,
            entry_hash: &tx.entry_hash,
            block_height: tx.block_height,
        },
    )?))
}

fn redeem_request_commitment_hex(
    req: &OfflineLineageRedeemRequest,
    lineage: &StoredLineage,
    amount: &str,
    pre_state_hash: &str,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &RedeemRequestCommitmentPayload {
            operation_id: &req.operation_id,
            kind: "redeem_request",
            account_id: &req.account_id,
            lineage_id: &req.lineage_id,
            asset_definition_id: &lineage.asset_definition_id,
            amount,
            offline_public_key: &req.offline_public_key,
            authorization_id: &lineage.authorization.authorization_id,
            pre_state_hash,
            receipt_keys: receipt_keys(&req.receipts),
        },
    )?))
}

fn verify_redeem_request_proof(
    req: &OfflineLineageRedeemRequest,
    lineage: &StoredLineage,
    amount: &str,
    pre_state_hash: &str,
) -> Result<(), Error> {
    ensure_offline_recursive_stark_ready()?;
    if req.redeem_proof.backend != OFFLINE_SETTLEMENT_PROOF_BACKEND {
        return Err(conversion_error(
            "redeem proof backend is not supported".to_owned(),
        ));
    }
    if req.redeem_proof.circuit_id != OFFLINE_REDEEM_REQUEST_CIRCUIT_ID {
        return Err(conversion_error(
            "redeem proof circuit_id is not supported".to_owned(),
        ));
    }
    if req.redeem_proof.recursion_depth != 1 {
        return Err(conversion_error(
            "redeem proof recursion_depth is invalid".to_owned(),
        ));
    }
    let expected_commitment = redeem_request_commitment_hex(req, lineage, amount, pre_state_hash)?;
    if req.redeem_proof.public_inputs_hex != expected_commitment {
        return Err(conversion_error(
            "redeem proof public inputs do not match the request".to_owned(),
        ));
    }
    if req.redeem_proof.envelope.params.domain_tag != expected_commitment {
        return Err(conversion_error(
            "redeem proof domain tag does not match the request".to_owned(),
        ));
    }
    let expected =
        synthesize_stark_envelope(expected_commitment, OFFLINE_REDEEM_REQUEST_CIRCUIT_ID)?;
    if encode_stark_envelope_bytes(&req.redeem_proof.envelope)?
        != encode_stark_envelope_bytes(&expected)?
    {
        return Err(conversion_error(
            "redeem proof envelope is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn settlement_from_tx(
    kind: &str,
    operation_id: &str,
    lineage: &StoredLineage,
    amount: &str,
    pre_state_hash: &str,
    post_state_hash: &str,
    tx: &SubmittedTransactionReceipt,
) -> Result<OfflineMutationSettlement, Error> {
    ensure_offline_recursive_stark_ready()?;
    let settlement_commitment_hex = settlement_commitment_hex(
        operation_id,
        kind,
        lineage,
        amount,
        pre_state_hash,
        post_state_hash,
        tx,
    )?;
    let envelope = synthesize_stark_envelope(
        settlement_commitment_hex.clone(),
        OFFLINE_SETTLEMENT_CIRCUIT_ID,
    )?;
    Ok(OfflineMutationSettlement {
        kind: kind.to_owned(),
        operation_id: operation_id.to_owned(),
        chain_tx_hash: tx.chain_tx_hash.clone(),
        entry_hash: tx.entry_hash.clone(),
        block_height: tx.block_height,
        pre_state_hash: pre_state_hash.to_owned(),
        post_state_hash: post_state_hash.to_owned(),
        settlement_commitment_hex: settlement_commitment_hex.clone(),
        proof: OfflineTransparentZkProof {
            backend: OFFLINE_SETTLEMENT_PROOF_BACKEND.to_owned(),
            circuit_id: OFFLINE_SETTLEMENT_CIRCUIT_ID.to_owned(),
            recursion_depth: 1,
            public_inputs_hex: settlement_commitment_hex,
            envelope,
        },
    })
}

async fn finalize_operation_result_settlement(
    app: &AppState,
    issuer: &OfflineIssuerSigner,
    lineage: &StoredLineage,
    operation_key: String,
    kind: &str,
    request_hash_hex: String,
    completed_at_ms: u64,
    envelope: &OfflineLineageEnvelope,
    endpoint: &'static str,
) -> Result<(), Error> {
    submit_signed_instruction(
        app,
        operator_authority(issuer)?,
        issuer.operator_keypair.private_key().clone(),
        InstructionBox::from(CommitOfflineLineageOperation {
            expected_server_revision: lineage.server_revision,
            expected_state_hash: lineage.server_state_hash.clone(),
            lineage: shared_record_from_local(issuer, lineage)?,
            result: SharedOfflineLineageOperationResult {
                operation_key,
                kind: kind.to_owned(),
                request_hash_hex,
                lineage_id: lineage.lineage_id.clone(),
                envelope: shared_envelope_from_local(envelope)?,
                completed_at_ms,
            },
        }),
        endpoint,
    )
    .await?;
    Ok(())
}

async fn submit_signed_instruction(
    app: &AppState,
    authority: AccountId,
    private_key: PrivateKey,
    instruction: InstructionBox,
    endpoint: &'static str,
) -> Result<SubmittedTransactionReceipt, Error> {
    let tx = build_signed_instructions(authority, private_key, [instruction], app);
    submit_prebuilt_transaction(app, tx, endpoint).await
}

async fn submit_signed_instructions(
    app: &AppState,
    authority: AccountId,
    private_key: PrivateKey,
    instructions: Vec<InstructionBox>,
    endpoint: &'static str,
) -> Result<SubmittedTransactionReceipt, Error> {
    let tx = build_signed_instructions(authority, private_key, instructions, app);
    submit_prebuilt_transaction(app, tx, endpoint).await
}

fn build_signed_instructions<I>(
    authority: AccountId,
    private_key: PrivateKey,
    instructions: I,
    app: &AppState,
) -> SignedTransaction
where
    I: IntoIterator<Item = InstructionBox>,
{
    TransactionBuilder::new((*app.chain_id).clone(), authority)
        .with_instructions(instructions)
        .sign(&private_key)
}

async fn submit_prebuilt_transaction(
    app: &AppState,
    tx: SignedTransaction,
    endpoint: &'static str,
) -> Result<SubmittedTransactionReceipt, Error> {
    let tx_hash = tx.hash();
    let entry_hash = tx.hash_as_entrypoint();
    let mut events_rx = app.events.subscribe();
    let duplicate_submission = match routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        tx,
        app.telemetry_handle(),
        endpoint,
    )
    .await
    {
        Ok(_) => false,
        Err(Error::PushIntoQueue { source, .. })
            if matches!(
                *source,
                queue::Error::InBlockchain | queue::Error::IsInQueue
            ) =>
        {
            // Treat duplicate submissions as success, but still wait until the
            // transaction emits an authoritative approved/rejected pipeline status
            // before returning an offline-cash envelope to the client.
            true
        }
        Err(err) => return Err(err),
    };
    wait_for_transaction_approval(app, &mut events_rx, tx_hash, endpoint, duplicate_submission)
        .await?;
    let Some(height) = app.state.committed_transaction_height(&tx_hash) else {
        return Err(conversion_error(format!(
            "offline cash transaction committed without an indexed height for {endpoint}: {tx_hash}"
        )));
    };
    Ok(SubmittedTransactionReceipt {
        chain_tx_hash: tx_hash.to_string(),
        entry_hash: entry_hash.to_string(),
        block_height: u64::try_from(height.get()).unwrap_or(u64::MAX),
    })
}

fn matching_transaction_status<'a>(
    event_box: &'a EventBox,
    tx_hash: &iroha_crypto::HashOf<SignedTransaction>,
) -> Option<&'a TransactionStatus> {
    match event_box {
        EventBox::Pipeline(PipelineEventBox::Transaction(event)) if event.hash() == tx_hash => {
            Some(event.status())
        }
        EventBox::PipelineBatch(events) => events.iter().find_map(|event| match event {
            PipelineEventBox::Transaction(event) if event.hash() == tx_hash => Some(event.status()),
            _ => None,
        }),
        _ => None,
    }
}

async fn wait_for_transaction_approval(
    app: &AppState,
    events_rx: &mut tokio::sync::broadcast::Receiver<EventBox>,
    tx_hash: iroha_crypto::HashOf<SignedTransaction>,
    endpoint: &'static str,
    duplicate_submission: bool,
) -> Result<(), Error> {
    let start = tokio::time::Instant::now();
    let mut saw_committed = false;
    loop {
        if !saw_committed && app.state.has_committed_transaction(tx_hash.clone()) {
            saw_committed = true;
        }
        let remaining = OFFLINE_CASH_TX_COMMIT_TIMEOUT.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            let timeout_context = if saw_committed || duplicate_submission {
                "committed without an approved pipeline event"
            } else {
                "did not reach an approved pipeline event"
            };
            return Err(conversion_error(format!(
                "offline cash transaction {timeout_context} within {}ms for {endpoint}: {tx_hash}",
                OFFLINE_CASH_TX_COMMIT_TIMEOUT.as_millis(),
            )));
        }
        match tokio::time::timeout(remaining, events_rx.recv()).await {
            Ok(Ok(event_box)) => {
                let Some(status) = matching_transaction_status(&event_box, &tx_hash) else {
                    continue;
                };
                match status {
                    TransactionStatus::Approved => return Ok(()),
                    TransactionStatus::Rejected(reason) => {
                        return Err(conversion_error(format!(
                            "offline cash transaction rejected for {endpoint}: hash={tx_hash} display={reason} debug={reason:?}"
                        )));
                    }
                    TransactionStatus::Expired => {
                        return Err(conversion_error(format!(
                            "offline cash transaction expired for {endpoint}: {tx_hash}"
                        )));
                    }
                    TransactionStatus::Queued => {}
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                continue;
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                return Err(conversion_error(format!(
                    "offline cash transaction event stream closed while waiting for {endpoint}: {tx_hash}"
                )));
            }
            Err(_) => continue,
        }
    }
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
            "offline cash amount must be greater than zero".to_owned(),
        ));
    }
    Ok(amount)
}

fn parse_numeric(raw: &str) -> Result<Numeric, Error> {
    let normalized = raw.trim().replace(',', "");
    if normalized.is_empty() {
        return Err(conversion_error(
            "offline cash amount is required".to_owned(),
        ));
    }
    normalized
        .parse::<Numeric>()
        .map_err(|err| conversion_error(format!("invalid offline cash amount: {err}")))
}

fn add_amounts(lhs: &str, rhs: &str) -> Result<String, Error> {
    let left = parse_numeric(lhs)?;
    let right = parse_numeric(rhs)?;
    Ok(canonical_amount_string(
        &left
            .checked_add(right)
            .ok_or_else(|| conversion_error("offline cash amount overflow".to_owned()))?,
    ))
}

fn subtract_amounts(lhs: &str, rhs: &str) -> Result<String, Error> {
    let left = parse_numeric(lhs)?;
    let right = parse_numeric(rhs)?;
    Ok(canonical_amount_string(
        &left
            .checked_sub(right)
            .ok_or_else(|| conversion_error("insufficient offline cash balance".to_owned()))?,
    ))
}

fn compare_amounts(lhs: &str, rhs: &str) -> Result<std::cmp::Ordering, Error> {
    let left = parse_numeric(lhs)?;
    let right = parse_numeric(rhs)?;
    Ok(left.cmp(&right))
}

fn minimum_required_locked_balance(
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
    let minimum_pre_parked = minimum_required_locked_balance(
        &receipt.pre_balance,
        Some(authorization),
        receipt.created_at_ms,
    )?;
    let minimum_post_parked = minimum_required_locked_balance(
        expected_post_balance,
        Some(authorization),
        receipt.created_at_ms,
    )?;
    match receipt.direction.as_str() {
        "outgoing" => {
            if receipt.pre_locked_balance != minimum_pre_parked
                || receipt.post_locked_balance != minimum_post_parked
            {
                return Err(conversion_error(
                    "offline cash locked-balance continuity is invalid".to_owned(),
                ));
            }
        }
        "incoming" => {
            if compare_amounts(&receipt.pre_locked_balance, &minimum_pre_parked)?.is_lt()
                || compare_amounts(&receipt.post_locked_balance, &minimum_post_parked)?.is_lt()
                || compare_amounts(&receipt.pre_locked_balance, &receipt.pre_balance)?.is_gt()
                || compare_amounts(&receipt.post_locked_balance, expected_post_balance)?.is_gt()
            {
                return Err(conversion_error(
                    "offline cash locked-balance continuity is invalid".to_owned(),
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
    if authorization.lineage_id != receipt.lineage_id
        || authorization.account_id != receipt.account_id
        || authorization.device_id != receipt.device_id
        || authorization.offline_public_key != receipt.offline_public_key
        || authorization.app_attest_key_id != receipt.attestation.key_id
    {
        return Err(conversion_error(
            "offline transfer authorization does not match the sender offline cash lineage"
                .to_owned(),
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

fn setup_challenge_payload(req: &OfflineLineageSetupRequest) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&LineageSetupAttestationPayload {
        account_id: &req.account_id,
        device_id: &req.device_id,
        offline_public_key: &req.offline_public_key,
    })
}

fn latest_block_timestamp_ms(app: &AppState) -> u64 {
    app.state
        .latest_block_header_fast()
        .map(|header| header.creation_time_ms)
        .unwrap_or_else(now_ms)
}

fn metadata_insert_string(metadata: &mut Metadata, key: &str, value: &str) -> Result<(), Error> {
    let name = Name::from_str(key).map_err(|err| {
        conversion_error(format!(
            "invalid offline cash attestation metadata key `{key}`: {err}"
        ))
    })?;
    metadata.insert(
        name,
        iroha_primitives::json::Json::from(json::Value::String(value.to_owned())),
    );
    Ok(())
}

fn metadata_from_apple_binding(binding: &StoredAppleAppAttestBinding) -> Result<Metadata, Error> {
    let mut metadata = Metadata::default();
    metadata_insert_string(
        &mut metadata,
        "ios.app_attest.team_id",
        &binding.ios_team_id,
    )?;
    metadata_insert_string(
        &mut metadata,
        "ios.app_attest.bundle_id",
        &binding.ios_bundle_id,
    )?;
    metadata_insert_string(
        &mut metadata,
        "ios.app_attest.environment",
        &binding.ios_environment,
    )?;
    Ok(metadata)
}

fn apple_app_attest_binding_from_request(
    attestation: &OfflineDeviceAttestation,
) -> Result<Option<StoredAppleAppAttestBinding>, Error> {
    let report = attestation
        .attestation_report_base64
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let team_id = attestation
        .ios_team_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let bundle_id = attestation
        .ios_bundle_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let environment = attestation
        .ios_environment
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if report.is_none() && team_id.is_none() && bundle_id.is_none() && environment.is_none() {
        return Ok(None);
    }
    let report = report.ok_or_else(|| {
        conversion_error(
            "ios attestation report is required when offline cash attestation metadata is provided"
                .to_owned(),
        )
    })?;
    let team_id = team_id.ok_or_else(|| {
        conversion_error(
            "ios_team_id is required when attestation_report_base64 is provided".to_owned(),
        )
    })?;
    let bundle_id = bundle_id.ok_or_else(|| {
        conversion_error(
            "ios_bundle_id is required when attestation_report_base64 is provided".to_owned(),
        )
    })?;
    let environment = environment.ok_or_else(|| {
        conversion_error(
            "ios_environment is required when attestation_report_base64 is provided".to_owned(),
        )
    })?;
    Ok(Some(StoredAppleAppAttestBinding {
        attestation_report_base64: report.to_owned(),
        ios_team_id: team_id.to_owned(),
        ios_bundle_id: bundle_id.to_owned(),
        ios_environment: environment.to_owned(),
    }))
}

fn decode_challenge_hash_hex(challenge_hash_hex: &str) -> Result<[u8; 32], Error> {
    let normalized = challenge_hash_hex.trim().trim_start_matches("0x");
    let bytes = hex::decode(normalized).map_err(|err| {
        conversion_error(format!(
            "invalid offline cash attestation challenge hash: {err}"
        ))
    })?;
    if bytes.len() != Hash::LENGTH {
        return Err(conversion_error(
            "offline cash attestation challenge hash must be 32 bytes".to_owned(),
        ));
    }
    let mut hash = [0u8; Hash::LENGTH];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}

fn validate_setup_attestation(
    app: &AppState,
    req: &OfflineLineageSetupRequest,
    counter_book: &mut BTreeMap<String, u64>,
    expected_app_attest_key_id: &str,
) -> Result<Option<StoredAppleAppAttestBinding>, Error> {
    validate_lineage_attestation(
        app,
        &req.account_id,
        "setup",
        "setup",
        setup_challenge_payload(req)?,
        expected_app_attest_key_id,
        &req.attestation,
        counter_book,
        None,
    )
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashLineageAmountPayload<'a> {
    lineage_id: &'a str,
    amount: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct CashLineagePayload<'a> {
    lineage_id: &'a str,
}

#[derive(crate::json_macros::JsonSerialize)]
struct AndroidDeviceBindingChallengePayload<'a> {
    account_id: &'a str,
    device_id: &'a str,
    offline_public_key: &'a str,
    operation: &'a str,
}

fn cash_attestation_challenge_seed(
    account_id: &str,
    lineage_id: &str,
    operation: &str,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    canonical_json_bytes(&CashAttestationChallengePayload {
        account_id,
        lineage_id,
        operation,
        payload_hash: &sha256_hex(payload),
    })
}

fn validate_cash_attestation(
    app: &AppState,
    account_id: &str,
    lineage_id: &str,
    operation: &str,
    payload: Vec<u8>,
    expected_app_attest_key_id: &str,
    attestation: &OfflineDeviceAttestation,
    counter_book: &mut BTreeMap<String, u64>,
    stored_apple_app_attest_binding: Option<&StoredAppleAppAttestBinding>,
) -> Result<Option<StoredAppleAppAttestBinding>, Error> {
    if attestation.key_id != expected_app_attest_key_id {
        return Err(conversion_error(
            "app_attest_key_id does not match the attestation proof".to_owned(),
        ));
    }
    let challenge_seed =
        cash_attestation_challenge_seed(account_id, lineage_id, operation, &payload)?;
    if attestation.challenge_hash_hex != sha256_hex(&challenge_seed) {
        return Err(conversion_error(
            "offline cash attestation challenge hash is invalid".to_owned(),
        ));
    }
    let request_binding = apple_app_attest_binding_from_request(attestation)?;
    if let Some(binding) = request_binding.as_ref().or(stored_apple_app_attest_binding) {
        let metadata = metadata_from_apple_binding(binding)?;
        let attestation_report = BASE64_STANDARD
            .decode(binding.attestation_report_base64.as_bytes())
            .map_err(|err| {
                conversion_error(format!(
                    "invalid base64 offline cash attestation report: {err}"
                ))
            })?;
        let assertion = BASE64_STANDARD
            .decode(attestation.assertion_base64.as_bytes())
            .map_err(|err| {
                conversion_error(format!(
                    "invalid base64 offline cash attestation assertion: {err}"
                ))
            })?;
        let settlement_cfg = &app.state.settlement().offline;
        verify_lineage_apple_app_attest(
            &LineageAppleAppAttestVerification {
                metadata,
                attestation_report,
                key_id: attestation.key_id.clone(),
                assertion,
                counter: attestation.counter,
                challenge_hash: decode_challenge_hash_hex(&attestation.challenge_hash_hex)?,
            },
            latest_block_timestamp_ms(app),
            settlement_cfg,
        )
        .map_err(|err| {
            conversion_error(format!(
                "offline cash app attest verification failed: {err}"
            ))
        })?;
    } else if extract_assertion_counter(&attestation.assertion_base64)? != attestation.counter {
        return Err(conversion_error(
            "offline cash attestation counter does not match assertion data".to_owned(),
        ));
    }
    validate_counter(attestation, counter_book)?;
    Ok(request_binding)
}

fn validate_android_cash_device_binding(
    app: &AppState,
    account_id: &str,
    binding: &OfflineCashAndroidDeviceBinding,
    expected_device_id: &str,
    expected_offline_public_key: &str,
) -> Result<(), Error> {
    if !binding.platform.eq_ignore_ascii_case("android") {
        return Err(conversion_error(
            "offline cash device binding platform must be android".to_owned(),
        ));
    }
    ensure_non_empty(
        &binding.attestation_key_id,
        "device_binding.attestation_key_id",
    )?;
    ensure_non_empty(&binding.device_id, "device_binding.device_id")?;
    ensure_non_empty(
        &binding.offline_public_key,
        "device_binding.offline_public_key",
    )?;
    ensure_non_empty(
        &binding.attestation_report_base64,
        "device_binding.attestation_report_base64",
    )?;
    if binding.device_id != expected_device_id {
        return Err(conversion_error(
            "offline cash device binding does not match the request device".to_owned(),
        ));
    }
    if binding.offline_public_key != expected_offline_public_key {
        return Err(conversion_error(
            "offline cash device binding does not match the request signer".to_owned(),
        ));
    }
    let public_key_bytes = BASE64_STANDARD
        .decode(binding.offline_public_key.as_bytes())
        .map_err(|err| conversion_error(format!("invalid base64 public key: {err}")))?;
    let expected_key_id = sha256_hex(&public_key_bytes);
    if binding.attestation_key_id != expected_key_id {
        return Err(conversion_error(
            "offline cash device binding attestation key id is invalid".to_owned(),
        ));
    }

    let challenge_hash_hex = sha256_hex(&canonical_json_bytes(
        &AndroidDeviceBindingChallengePayload {
            account_id,
            device_id: expected_device_id,
            offline_public_key: expected_offline_public_key,
            operation: "device_binding",
        },
    )?);
    let expected_challenge = decode_challenge_hash_hex(&challenge_hash_hex)?;
    let chain = decode_android_attestation_chain(&binding.attestation_report_base64)?;
    let leaf = verify_android_chain(
        &chain,
        latest_block_timestamp_ms(app),
        &app.state.settlement().offline,
    )?;
    let key_description = parse_android_key_description(&leaf)?;
    if key_description.attestation_challenge.as_slice() != expected_challenge.as_slice() {
        return Err(conversion_error(
            "offline cash android attestation challenge does not match the device binding"
                .to_owned(),
        ));
    }
    if key_description.attestation_security_level == AndroidSecurityLevel::Software
        || key_description.keymaster_security_level == AndroidSecurityLevel::Software
    {
        return Err(conversion_error(
            "offline cash android attestation must be hardware-backed".to_owned(),
        ));
    }
    if leaf.public_key().subject_public_key.data.as_ref() != public_key_bytes.as_slice() {
        return Err(conversion_error(
            "offline cash android attestation key does not match offline_public_key".to_owned(),
        ));
    }
    Ok(())
}

fn validate_android_cash_operation_proof(
    account_id: &str,
    lineage_id: &str,
    operation: &str,
    payload: Vec<u8>,
    binding: &OfflineCashAndroidDeviceBinding,
    proof: &OfflineCashAndroidDeviceProof,
) -> Result<(), Error> {
    if !proof.platform.eq_ignore_ascii_case("android") {
        return Err(conversion_error(
            "offline cash device proof platform must be android".to_owned(),
        ));
    }
    if proof.counter.unwrap_or(0) != 0 {
        return Err(conversion_error(
            "android offline cash proofs must not include a counter".to_owned(),
        ));
    }
    if proof.attestation_key_id != binding.attestation_key_id {
        return Err(conversion_error(
            "offline cash device proof does not match the device binding".to_owned(),
        ));
    }
    let challenge_seed =
        cash_attestation_challenge_seed(account_id, lineage_id, operation, &payload)?;
    let expected_hash_hex = sha256_hex(&challenge_seed);
    if !proof
        .challenge_hash_hex
        .eq_ignore_ascii_case(&expected_hash_hex)
    {
        return Err(conversion_error(
            "offline cash device proof challenge hash is invalid".to_owned(),
        ));
    }
    validate_signature(
        "offline cash device proof assertion",
        &decode_challenge_hash_hex(&proof.challenge_hash_hex)?,
        &proof.assertion_base64,
        &binding.offline_public_key,
    )
    .map_err(|_| conversion_error("offline cash device proof assertion is invalid".to_owned()))
}

fn validate_lineage_attestation(
    app: &AppState,
    account_id: &str,
    lineage_id: &str,
    operation: &str,
    payload: Vec<u8>,
    expected_app_attest_key_id: &str,
    attestation: &OfflineDeviceAttestation,
    counter_book: &mut BTreeMap<String, u64>,
    stored_apple_app_attest_binding: Option<&StoredAppleAppAttestBinding>,
) -> Result<Option<StoredAppleAppAttestBinding>, Error> {
    if attestation.key_id != expected_app_attest_key_id {
        return Err(conversion_error(
            "app_attest_key_id does not match the attestation proof".to_owned(),
        ));
    }
    let challenge_seed = canonical_json_bytes(&AttestationChallengePayload {
        account_id,
        lineage_id,
        operation,
        payload_hash: &sha256_hex(&payload),
    })?;
    if attestation.challenge_hash_hex != sha256_hex(&challenge_seed) {
        return Err(conversion_error(
            "offline cash attestation challenge hash is invalid".to_owned(),
        ));
    }
    let request_binding = apple_app_attest_binding_from_request(attestation)?;
    if let Some(binding) = request_binding.as_ref().or(stored_apple_app_attest_binding) {
        let metadata = metadata_from_apple_binding(binding)?;
        let attestation_report = BASE64_STANDARD
            .decode(binding.attestation_report_base64.as_bytes())
            .map_err(|err| {
                conversion_error(format!(
                    "invalid base64 offline cash attestation report: {err}"
                ))
            })?;
        let assertion = BASE64_STANDARD
            .decode(attestation.assertion_base64.as_bytes())
            .map_err(|err| {
                conversion_error(format!(
                    "invalid base64 offline cash attestation assertion: {err}"
                ))
            })?;
        let settlement_cfg = &app.state.settlement().offline;
        verify_lineage_apple_app_attest(
            &LineageAppleAppAttestVerification {
                metadata,
                attestation_report,
                key_id: attestation.key_id.clone(),
                assertion,
                counter: attestation.counter,
                challenge_hash: decode_challenge_hash_hex(&attestation.challenge_hash_hex)?,
            },
            latest_block_timestamp_ms(app),
            settlement_cfg,
        )
        .map_err(|err| {
            conversion_error(format!(
                "offline cash app attest verification failed: {err}"
            ))
        })?;
    } else if extract_assertion_counter(&attestation.assertion_base64)? != attestation.counter {
        return Err(conversion_error(
            "offline cash attestation counter does not match assertion data".to_owned(),
        ));
    }
    validate_counter(attestation, counter_book)?;
    Ok(request_binding)
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum AndroidSecurityLevel {
    Software,
    TrustedEnvironment,
    StrongBox,
}

struct AndroidKeyDescription {
    attestation_security_level: AndroidSecurityLevel,
    keymaster_security_level: AndroidSecurityLevel,
    attestation_challenge: Vec<u8>,
}

struct AndroidDerReader<'a> {
    data: &'a [u8],
    offset: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AndroidTagClass {
    Universal,
    Application,
    ContextSpecific,
    Private,
}

struct AndroidTlv<'a> {
    class: AndroidTagClass,
    constructed: bool,
    tag: u32,
    value: &'a [u8],
}

impl TryFrom<u64> for AndroidSecurityLevel {
    type Error = Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Software),
            1 => Ok(Self::TrustedEnvironment),
            2 => Ok(Self::StrongBox),
            other => Err(conversion_error(format!(
                "unknown android security level `{other}`"
            ))),
        }
    }
}

impl<'a> AndroidDerReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn has_remaining(&self) -> bool {
        self.offset < self.data.len()
    }

    fn read_integer(&mut self, label: &str) -> Result<u64, Error> {
        let value = self.expect_universal(AndroidTagClass::Universal, false, 2, label)?;
        parse_android_unsigned_integer_bytes(value, label)
    }

    fn read_enumerated(&mut self, label: &str) -> Result<u64, Error> {
        let value = self.expect_universal(AndroidTagClass::Universal, false, 10, label)?;
        parse_android_unsigned_integer_bytes(value, label)
    }

    fn read_octet_string(&mut self, label: &str) -> Result<&'a [u8], Error> {
        self.expect_universal(AndroidTagClass::Universal, false, 4, label)
    }

    fn read_sequence_bytes(&mut self, label: &str) -> Result<&'a [u8], Error> {
        self.expect_universal(AndroidTagClass::Universal, true, 16, label)
    }

    fn expect_universal(
        &mut self,
        class: AndroidTagClass,
        constructed: bool,
        tag: u32,
        label: &str,
    ) -> Result<&'a [u8], Error> {
        let tlv = self.read_tlv()?;
        if tlv.class != class || tlv.constructed != constructed || tlv.tag != tag {
            return Err(conversion_error(format!(
                "unexpected DER tag while parsing `{label}`"
            )));
        }
        Ok(tlv.value)
    }

    fn read_tlv(&mut self) -> Result<AndroidTlv<'a>, Error> {
        if self.offset >= self.data.len() {
            return Err(conversion_error("unexpected end of DER input".to_owned()));
        }
        let tag_byte = self.data[self.offset];
        self.offset += 1;
        let class = match tag_byte >> 6 {
            0 => AndroidTagClass::Universal,
            1 => AndroidTagClass::Application,
            2 => AndroidTagClass::ContextSpecific,
            _ => AndroidTagClass::Private,
        };
        let constructed = (tag_byte & 0x20) != 0;
        let mut tag_number = u32::from(tag_byte & 0x1F);
        if tag_number == 0x1F {
            tag_number = 0;
            loop {
                if self.offset >= self.data.len() {
                    return Err(conversion_error("invalid DER tag encoding".to_owned()));
                }
                let byte = self.data[self.offset];
                self.offset += 1;
                tag_number = (tag_number << 7) | u32::from(byte & 0x7F);
                if byte & 0x80 == 0 {
                    break;
                }
            }
        }
        let length = self.read_length()?;
        if self.offset + length > self.data.len() {
            return Err(conversion_error(
                "DER value exceeds available input".to_owned(),
            ));
        }
        let value = &self.data[self.offset..self.offset + length];
        self.offset += length;
        Ok(AndroidTlv {
            class,
            constructed,
            tag: tag_number,
            value,
        })
    }

    fn read_length(&mut self) -> Result<usize, Error> {
        if self.offset >= self.data.len() {
            return Err(conversion_error("invalid DER length encoding".to_owned()));
        }
        let first = self.data[self.offset];
        self.offset += 1;
        if first & 0x80 == 0 {
            return Ok(first as usize);
        }
        let octets = (first & 0x7F) as usize;
        if octets == 0 || octets > 4 {
            return Err(conversion_error(
                "unsupported DER length encoding".to_owned(),
            ));
        }
        if self.offset + octets > self.data.len() {
            return Err(conversion_error("invalid DER length encoding".to_owned()));
        }
        let mut length = 0usize;
        for _ in 0..octets {
            length = (length << 8) | self.data[self.offset] as usize;
            self.offset += 1;
        }
        Ok(length)
    }
}

fn parse_android_unsigned_integer_bytes(bytes: &[u8], label: &str) -> Result<u64, Error> {
    if bytes.is_empty() || bytes.len() > 8 {
        return Err(conversion_error(format!(
            "invalid DER integer while parsing `{label}`"
        )));
    }
    let mut value = 0u64;
    for byte in bytes {
        value = (value << 8) | u64::from(*byte);
    }
    Ok(value)
}

fn decode_android_attestation_chain(report_base64: &str) -> Result<Vec<Vec<u8>>, Error> {
    let bytes = BASE64_STANDARD
        .decode(report_base64.as_bytes())
        .map_err(|err| conversion_error(format!("invalid base64 android attestation: {err}")))?;
    let value: CborValue = from_reader(bytes.as_slice())
        .map_err(|err| conversion_error(format!("invalid android attestation CBOR: {err}")))?;
    match value {
        CborValue::Array(entries) if !entries.is_empty() => entries
            .into_iter()
            .map(|entry| match entry {
                CborValue::Bytes(bytes) => Ok(bytes),
                _ => Err(conversion_error(
                    "android attestation must be a CBOR array of certificates".to_owned(),
                )),
            })
            .collect(),
        _ => Err(conversion_error(
            "android attestation must be a CBOR array of certificates".to_owned(),
        )),
    }
}

fn verify_android_chain<'a>(
    certificates: &'a [Vec<u8>],
    block_timestamp_ms: u64,
    settlement_cfg: &iroha_config::parameters::actual::Offline,
) -> Result<X509Certificate<'a>, Error> {
    if certificates.is_empty() {
        return Err(conversion_error("attestation chain is empty".to_owned()));
    }
    let block_time = asn1_time_from_unix_ms(block_timestamp_ms)?;
    let (_, leaf_cert) = X509Certificate::from_der(&certificates[0])
        .map_err(|err| conversion_error(format!("failed to parse attestation leaf cert: {err}")))?;
    check_certificate_validity(&leaf_cert, block_time)?;

    for window in certificates.windows(2) {
        let (_, child) = X509Certificate::from_der(&window[0])
            .map_err(|err| conversion_error(format!("failed to parse attestation cert: {err}")))?;
        let (_, parent) = X509Certificate::from_der(&window[1])
            .map_err(|err| conversion_error(format!("failed to parse attestation cert: {err}")))?;
        check_certificate_validity(&child, block_time)?;
        check_certificate_validity(&parent, block_time)?;
        child
            .verify_signature(Some(parent.public_key()))
            .map_err(|_| {
                conversion_error("attestation chain is not internally signed".to_owned())
            })?;
    }

    let last_bytes = certificates
        .last()
        .expect("attestation chain cannot be empty");
    let (_, last_cert) = X509Certificate::from_der(last_bytes)
        .map_err(|err| conversion_error(format!("failed to parse attestation cert: {err}")))?;
    let mut anchored = false;
    for anchor in &settlement_cfg.android_trust_anchors {
        if android_anchor_matches(anchor, last_bytes, &last_cert) {
            anchored = true;
            break;
        }
    }
    if !anchored {
        for anchor in ANDROID_ROOT_ANCHORS.iter() {
            if android_anchor_matches(anchor, last_bytes, &last_cert) {
                anchored = true;
                break;
            }
        }
    }
    if !anchored {
        return Err(conversion_error(
            "attestation chain does not terminate at a trusted root".to_owned(),
        ));
    }
    Ok(leaf_cert)
}

fn android_anchor_matches(
    anchor_bytes: &[u8],
    last_bytes: &[u8],
    last_cert: &X509Certificate<'_>,
) -> bool {
    if last_bytes == anchor_bytes {
        return true;
    }
    if let Ok((_, root)) = X509Certificate::from_der(anchor_bytes) {
        return last_cert.verify_signature(Some(root.public_key())).is_ok();
    }
    false
}

fn check_certificate_validity(
    cert: &X509Certificate<'_>,
    block_time: ASN1Time,
) -> Result<(), Error> {
    if block_time < cert.validity().not_before || block_time > cert.validity().not_after {
        return Err(conversion_error(
            "attestation certificate is not valid for current block time".to_owned(),
        ));
    }
    Ok(())
}

fn asn1_time_from_unix_ms(block_timestamp_ms: u64) -> Result<ASN1Time, Error> {
    let seconds = i64::try_from(block_timestamp_ms / 1000)
        .map_err(|_| conversion_error("block timestamp is out of range".to_owned()))?;
    ASN1Time::from_timestamp(seconds)
        .map_err(|err| conversion_error(format!("failed to convert block timestamp: {err}")))
}

fn parse_android_key_description(
    cert: &X509Certificate<'_>,
) -> Result<AndroidKeyDescription, Error> {
    let key_desc_oid = x509_parser::oid_registry::Oid::from(&[1, 3, 6, 1, 4, 1, 11129, 2, 1, 17])
        .expect("android attestation OID must be valid");
    let ext = cert
        .extensions()
        .iter()
        .find(|ext| ext.oid == key_desc_oid)
        .ok_or_else(|| {
            conversion_error(
                "android attestation certificate does not contain keyDescription extension"
                    .to_owned(),
            )
        })?;
    let mut reader = AndroidDerReader::new(ext.value);
    let octet = reader.read_octet_string("attestationExtension")?;
    if reader.has_remaining() {
        return Err(conversion_error(
            "android attestation extension contained trailing data".to_owned(),
        ));
    }
    let mut seq = AndroidDerReader::new(octet);
    let attestation_version = seq.read_integer("attestationVersion")?;
    if attestation_version == 0 {
        return Err(conversion_error(
            "android attestationVersion must be positive".to_owned(),
        ));
    }
    let attestation_security_level =
        AndroidSecurityLevel::try_from(seq.read_enumerated("attestationSecurityLevel")?)?;
    let keymaster_version = seq.read_integer("keymasterVersion")?;
    if keymaster_version == 0 {
        return Err(conversion_error(
            "android keymasterVersion must be positive".to_owned(),
        ));
    }
    let keymaster_security_level =
        AndroidSecurityLevel::try_from(seq.read_enumerated("keymasterSecurityLevel")?)?;
    let attestation_challenge = seq.read_octet_string("attestationChallenge")?.to_vec();
    let _unique_id = seq.read_octet_string("uniqueId")?;
    let _software = seq.read_sequence_bytes("softwareEnforced")?;
    let _tee = seq.read_sequence_bytes("teeEnforced")?;
    if seq.has_remaining() {
        let _strongbox = seq.read_sequence_bytes("strongBoxEnforced")?;
    }
    if seq.has_remaining() {
        return Err(conversion_error(
            "android keyDescription contained trailing data".to_owned(),
        ));
    }
    Ok(AndroidKeyDescription {
        attestation_security_level,
        keymaster_security_level,
        attestation_challenge,
    })
}

fn sender_state_key(lineage_id: &str, local_revision: u64) -> String {
    format!("{lineage_id}:{local_revision}")
}

fn load_request_hash_hex(
    req: &OfflineLineageLoadRequest,
    amount: &Numeric,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &LineageLoadRequestHashPayload {
            lineage_id: req.lineage_id.as_deref(),
            account_id: &req.account_id,
            device_id: &req.device_id,
            offline_public_key: &req.offline_public_key,
            asset_definition_id: &req.asset_definition_id,
            app_attest_key_id: &req.app_attest_key_id,
            amount: &canonical_amount_string(amount),
        },
    )?))
}

fn refresh_request_hash_hex(req: &OfflineLineageRefreshRequest) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &LineageRefreshAttestationPayload {
            lineage_id: &req.lineage_id,
        },
    )?))
}

fn sync_request_hash_hex(req: &OfflineLineageSyncRequest) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &LineageRedeemRequestHashPayload {
            lineage_id: &req.lineage_id,
            account_id: &req.account_id,
            device_id: &req.device_id,
            offline_public_key: &req.offline_public_key,
            amount: "",
            receipt_keys: receipt_keys(&req.receipts),
            redeem_public_inputs_hex: "",
        },
    )?))
}

fn redeem_request_hash_hex(
    req: &OfflineLineageRedeemRequest,
    amount: &Numeric,
) -> Result<String, Error> {
    Ok(sha256_hex(&canonical_json_bytes(
        &LineageRedeemRequestHashPayload {
            lineage_id: &req.lineage_id,
            account_id: &req.account_id,
            device_id: &req.device_id,
            offline_public_key: &req.offline_public_key,
            amount: &canonical_amount_string(amount),
            receipt_keys: receipt_keys(&req.receipts),
            redeem_public_inputs_hex: &req.redeem_proof.public_inputs_hex,
        },
    )?))
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ACCOUNT_I105: &str =
        "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
    const TEST_COUNTERPARTY_ACCOUNT_I105: &str =
        "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D";

    #[test]
    fn sync_request_hash_is_stable_for_reordered_receipts() {
        let make_receipt = |transfer_id: &str, local_revision: u64| OfflineTransferReceipt {
            version: 1,
            transfer_id: transfer_id.to_owned(),
            direction: "incoming".to_owned(),
            lineage_id: "lineage".to_owned(),
            account_id: TEST_ACCOUNT_I105.to_owned(),
            device_id: "device-1".to_owned(),
            offline_public_key: BASE64_STANDARD.encode([7u8; 32]),
            pre_balance: "0".to_owned(),
            post_balance: "1".to_owned(),
            pre_locked_balance: "0".to_owned(),
            post_locked_balance: "0".to_owned(),
            pre_state_hash: "pre".to_owned(),
            post_state_hash: "post".to_owned(),
            local_revision,
            counterparty_lineage_id: "peer".to_owned(),
            counterparty_account_id: TEST_COUNTERPARTY_ACCOUNT_I105.to_owned(),
            counterparty_device_id: "device-2".to_owned(),
            counterparty_offline_public_key: BASE64_STANDARD.encode([8u8; 32]),
            amount: "1".to_owned(),
            authorization: None,
            attestation: OfflineDeviceAttestation {
                key_id: "key".to_owned(),
                counter: local_revision,
                assertion_base64: BASE64_STANDARD.encode(b"assertion"),
                challenge_hash_hex: "abc".to_owned(),
                attestation_report_base64: None,
                ios_team_id: None,
                ios_bundle_id: None,
                ios_environment: None,
            },
            source_payload: None,
            sender_signature_base64: BASE64_STANDARD.encode([9u8; 64]),
            created_at_ms: 1,
        };

        let lhs = OfflineLineageSyncRequest {
            operation_id: "sync-1".to_owned(),
            lineage_id: "lineage".to_owned(),
            account_id: TEST_ACCOUNT_I105.to_owned(),
            device_id: "device-1".to_owned(),
            offline_public_key: BASE64_STANDARD.encode([7u8; 32]),
            receipts: vec![make_receipt("b", 2), make_receipt("a", 1)],
        };
        let rhs = OfflineLineageSyncRequest {
            receipts: lhs.receipts.iter().rev().cloned().collect(),
            ..lhs.clone()
        };

        let left = sync_request_hash_hex(&lhs).expect("left hash");
        let right = sync_request_hash_hex(&rhs).expect("right hash");
        assert_eq!(left, right);
    }

    #[test]
    fn cash_receipt_payload_omits_empty_attestation_option_fields() {
        let receipt = OfflineTransferReceipt {
            version: 1,
            transfer_id: "transfer-1".to_owned(),
            direction: "outgoing".to_owned(),
            lineage_id: "lineage-1".to_owned(),
            account_id: TEST_ACCOUNT_I105.to_owned(),
            device_id: "device-1".to_owned(),
            offline_public_key: BASE64_STANDARD.encode([7u8; 32]),
            pre_balance: "25".to_owned(),
            post_balance: "15".to_owned(),
            pre_locked_balance: "0".to_owned(),
            post_locked_balance: "0".to_owned(),
            pre_state_hash: "pre".to_owned(),
            post_state_hash: "post".to_owned(),
            local_revision: 1,
            counterparty_lineage_id: "lineage-2".to_owned(),
            counterparty_account_id: TEST_COUNTERPARTY_ACCOUNT_I105.to_owned(),
            counterparty_device_id: "device-2".to_owned(),
            counterparty_offline_public_key: BASE64_STANDARD.encode([8u8; 32]),
            amount: "10".to_owned(),
            authorization: None,
            attestation: OfflineDeviceAttestation {
                key_id: "key-1".to_owned(),
                counter: 3,
                assertion_base64: BASE64_STANDARD.encode(b"assertion"),
                challenge_hash_hex: "challenge".to_owned(),
                attestation_report_base64: None,
                ios_team_id: None,
                ios_bundle_id: None,
                ios_environment: None,
            },
            source_payload: None,
            sender_signature_base64: BASE64_STANDARD.encode([9u8; 64]),
            created_at_ms: 1,
        };

        let payload = cash_transfer_receipt_unsigned_payload(&receipt).expect("cash payload");
        let value = json::from_slice::<json::Value>(&payload).expect("json value");
        let attestation = value
            .as_object()
            .and_then(|object| object.get("attestation"))
            .and_then(json::Value::as_object)
            .expect("attestation object");

        assert_eq!(attestation.len(), 4);
        assert!(attestation.get("key_id").is_some());
        assert!(attestation.get("counter").is_some());
        assert!(attestation.get("assertion_base64").is_some());
        assert!(attestation.get("challenge_hash_hex").is_some());
        assert!(attestation.get("attestation_report_base64").is_none());
        assert!(attestation.get("ios_team_id").is_none());
        assert!(attestation.get("ios_bundle_id").is_none());
        assert!(attestation.get("ios_environment").is_none());
    }

    #[test]
    fn cash_local_state_hash_uses_lineage_shape() {
        let hash = cash_next_local_state_hash(
            "lineage-1",
            "previous-hash",
            "transfer-1",
            "outgoing",
            "lineage-2",
            "10",
            1,
            "15",
            "0",
        )
        .expect("cash state hash");

        let expected = sha256_hex(
            &canonical_json_bytes(&norito::json!({
                "amount": "10",
                "counterparty_lineage_id": "lineage-2",
                "direction": "outgoing",
                "lineage_id": "lineage-1",
                "local_revision": 1,
                "post_balance": "15",
                "post_locked_balance": "0",
                "previous_state_hash": "previous-hash",
                "transfer_id": "transfer-1",
            }))
            .expect("expected json"),
        );

        assert_eq!(hash, expected);
    }

    #[test]
    fn redeem_with_receipts_still_commits_single_server_revision_advance() {
        let expected_server_revision = 5u64;
        let server_revision_after_receipts = 6u64;

        let committed_revision = committed_server_revision(expected_server_revision);

        assert_eq!(
            committed_revision,
            expected_server_revision.saturating_add(1)
        );
        assert_eq!(committed_revision, server_revision_after_receipts);
    }

    #[test]
    fn canonical_account_helper_rejects_alias_literals() {
        let err = ensure_canonical_account_id_literal("alice@wallets", "account_id")
            .expect_err("aliases must be rejected");
        assert!(err.to_string().contains("canonical I105 account id"));
    }

    #[test]
    fn canonical_asset_definition_helper_rejects_alias_literals() {
        let err =
            ensure_canonical_asset_definition_id_literal("usd#wallets", "asset_definition_id")
                .expect_err("aliases must be rejected");
        assert!(
            err.to_string()
                .contains("canonical Base58 asset definition id")
        );
    }
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
        if let Ok(mut value) = json::from_slice::<json::Value>(&decoded) {
            normalize_cash_outgoing_payload_value(&mut value)?;
            return json::from_value::<OfflineOutgoingTransferPayload>(value).map_err(|err| {
                conversion_error(format!(
                    "invalid offline cash transfer payload after translation: {err}"
                ))
            });
        }
    }
    if let Ok(payload) = json::from_str::<OfflineOutgoingTransferPayload>(encoded) {
        return Ok(payload);
    }
    let mut value = json::from_str::<json::Value>(encoded)
        .map_err(|err| conversion_error(format!("invalid offline transfer payload: {err}")))?;
    normalize_cash_outgoing_payload_value(&mut value)?;
    json::from_value::<OfflineOutgoingTransferPayload>(value).map_err(|err| {
        conversion_error(format!(
            "invalid offline cash transfer payload after translation: {err}"
        ))
    })
}

fn committed_server_revision(expected_server_revision: u64) -> u64 {
    expected_server_revision.saturating_add(1)
}

fn normalize_cash_authorization_value(value: &mut json::Value) -> Result<(), Error> {
    let Some(map) = value.as_object_mut() else {
        return Err(conversion_error(
            "offline cash authorization payload must be an object".to_owned(),
        ));
    };
    let _ = map;
    Ok(())
}

fn normalize_cash_state_value(value: &mut json::Value) -> Result<(), Error> {
    let Some(map) = value.as_object_mut() else {
        return Err(conversion_error(
            "offline cash lineage anchor must be an object".to_owned(),
        ));
    };
    if let Some(authorization) = map.get_mut("authorization") {
        normalize_cash_authorization_value(authorization)?;
    }
    Ok(())
}

fn normalize_cash_receipt_value(value: &mut json::Value) -> Result<(), Error> {
    let Some(map) = value.as_object_mut() else {
        return Err(conversion_error(
            "offline cash receipt payload must be an object".to_owned(),
        ));
    };
    if let Some(authorization) = map.get_mut("authorization") {
        normalize_cash_authorization_value(authorization)?;
    }
    Ok(())
}

fn normalize_cash_outgoing_payload_value(value: &mut json::Value) -> Result<(), Error> {
    let Some(map) = value.as_object_mut() else {
        return Err(conversion_error(
            "offline transfer payload must be an object".to_owned(),
        ));
    };
    if let Some(anchor) = map.get_mut("anchor") {
        normalize_cash_state_value(anchor)?;
    }
    if let Some(json::Value::Array(items)) = map.get_mut("ancestry_receipts") {
        for item in items {
            normalize_cash_receipt_value(item)?;
        }
    }
    if let Some(receipt) = map.get_mut("receipt") {
        normalize_cash_receipt_value(receipt)?;
    }
    Ok(())
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

fn ensure_canonical_account_id_literal(value: &str, field_name: &str) -> Result<(), Error> {
    let trimmed = value.trim();
    AccountId::parse_encoded(trimmed).map_err(|err| {
        conversion_error(format!(
            "{field_name} must be a canonical I105 account id: {err}"
        ))
    })?;
    Ok(())
}

fn ensure_canonical_asset_definition_id_literal(
    value: &str,
    field_name: &str,
) -> Result<(), Error> {
    let trimmed = value.trim();
    AssetDefinitionId::parse_address_literal(trimmed).map_err(|err| {
        conversion_error(format!(
            "{field_name} must be a canonical Base58 asset definition id: {err}"
        ))
    })?;
    Ok(())
}

fn ensure_canonical_authorization_identifiers(
    authorization: &OfflineSpendAuthorization,
    field_name: &str,
) -> Result<(), Error> {
    ensure_canonical_account_id_literal(
        &authorization.account_id,
        &format!("{field_name}.account_id"),
    )?;
    Ok(())
}

fn ensure_canonical_lineage_state_identifiers(
    state: &OfflineLineageState,
    field_name: &str,
) -> Result<(), Error> {
    ensure_canonical_account_id_literal(&state.account_id, &format!("{field_name}.account_id"))?;
    ensure_canonical_asset_definition_id_literal(
        &state.asset_definition_id,
        &format!("{field_name}.asset_definition_id"),
    )?;
    ensure_canonical_authorization_identifiers(
        &state.authorization,
        &format!("{field_name}.authorization"),
    )?;
    Ok(())
}

fn ensure_canonical_transfer_receipt_identifiers(
    receipt: &OfflineTransferReceipt,
    field_name: &str,
) -> Result<(), Error> {
    ensure_canonical_account_id_literal(&receipt.account_id, &format!("{field_name}.account_id"))?;
    ensure_canonical_account_id_literal(
        &receipt.counterparty_account_id,
        &format!("{field_name}.counterparty_account_id"),
    )?;
    if let Some(authorization) = receipt.authorization.as_ref() {
        ensure_canonical_authorization_identifiers(
            authorization,
            &format!("{field_name}.authorization"),
        )?;
    }
    Ok(())
}

fn conversion_error(message: String) -> Error {
    routing::conversion_error(message)
}
