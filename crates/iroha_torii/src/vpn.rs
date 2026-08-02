use std::{
    collections::HashSet,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use iroha_config::client_api::ConfigGetDTO;
use iroha_core::{
    kiso::KisoHandle,
    state::{VPN_SETTLED_RECEIPT_HISTORY_LIMIT as MAX_RECEIPTS_PER_ACCOUNT, WorldReadOnly},
};
use iroha_crypto::{
    Algorithm, Hash, HashOf, PublicKey,
    soranet::{
        certificate::{RelayCertificateBundleV2, select_vpn_endpoint},
        directory::GuardDirectorySnapshotV2,
    },
};
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    asset::AssetDefinitionId,
    block::SignedBlock,
    domain::DomainId,
    isi::{InstructionBox, OpenVpnLeaseEscrow, SettleVpnLease},
    name::Name,
    query::error::QueryExecutionFail,
    soranet::vpn::{
        VPN_DEFAULT_TUNNEL_MTU_BYTES, VpnExitClassV1, VpnHelperTicketV1, VpnLeaseRecordV1,
        VpnLeaseStatusV1, VpnQuotePolicyV1, VpnSessionReceiptV1, VpnTariffV1, VpnUsageVoucherV1,
        derive_vpn_session_address_plan_v1,
    },
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_primitives::numeric::{Numeric, Quantity, RoundingMode};
use mv::storage::StorageReadOnly;
use sha2::{Digest as _, Sha256};

use crate::{Error, SharedAppState};

const SUPPORTED_EXIT_CLASSES: [&str; 3] = ["standard", "low-latency", "high-security"];
const DEFAULT_TUNNEL_ADDRESSES: [&str; 2] = ["10.208.0.2/32", "fd53:7261:6574::2/128"];
const MAX_SESSION_ADDRESS_ALLOCATION_ATTEMPTS: u32 = 4_096;

/// Immutable VPN relay trust derived from an authenticated guard-directory snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnRelayTrust {
    /// Exact relay Ed25519 identity advertised by the signed certificate.
    pub relay_id: [u8; 32],
    /// Exact signed QUIC endpoint selected for VPN traffic.
    pub relay_endpoint: String,
    /// Exact TLS DNS name selected for the endpoint.
    pub tls_server_name: String,
    /// Exact leaf SPKI SHA-256 pin selected for the endpoint.
    pub relay_tls_spki_sha256: [u8; 32],
    /// Descriptor commitment authenticated by the relay certificate.
    pub descriptor_commit: [u8; 32],
    /// SHA-256 digest of the canonical relay certificate bundle.
    pub relay_certificate_sha256: [u8; 32],
    /// Externally provisioned digest authenticating the exact directory snapshot.
    pub directory_snapshot_digest: [u8; 32],
    /// Exclusive upper bound on authenticated relay trust, in Unix milliseconds.
    pub valid_until_ms: u64,
}

impl VpnRelayTrust {
    /// Authenticate a directory snapshot and select one exact VPN relay endpoint.
    ///
    /// # Errors
    /// Returns an error if snapshot authentication or freshness fails, the relay
    /// is absent, or its certificate does not authorize a VPN endpoint.
    pub fn from_guard_directory_at(
        snapshot_bytes: &[u8],
        expected_snapshot_digest: [u8; 32],
        relay_id: [u8; 32],
        at_unix: i64,
    ) -> Result<Self, String> {
        let snapshot = GuardDirectorySnapshotV2::authenticate_bytes_at(
            snapshot_bytes,
            expected_snapshot_digest,
            at_unix,
        )
        .map_err(|error| format!("VPN guard directory authentication failed: {error}"))?;
        let bundle = snapshot
            .relays
            .iter()
            .map(|entry| RelayCertificateBundleV2::from_cbor(&entry.certificate))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("VPN relay certificate decode failed: {error}"))?
            .into_iter()
            .find(|bundle| bundle.certificate.relay_id == relay_id)
            .ok_or_else(|| {
                format!(
                    "VPN relay {} is absent from the authenticated guard directory",
                    hex::encode(relay_id)
                )
            })?;
        if !bundle.certificate.roles.exit {
            return Err("VPN relay certificate does not authorize the exit role".to_owned());
        }
        let endpoint = select_vpn_endpoint(&bundle.certificate.endpoints)
            .map_err(|error| format!("VPN endpoint selection failed: {error}"))?;
        let canonical_bundle = bundle
            .try_to_cbor()
            .map_err(|error| format!("VPN relay certificate encode failed: {error}"))?;
        let valid_until_unix = snapshot
            .valid_until_unix
            .min(bundle.certificate.valid_until);
        let valid_until_ms = u64::try_from(valid_until_unix)
            .ok()
            .and_then(|seconds| seconds.checked_mul(1_000))
            .ok_or_else(|| "VPN relay trust validity exceeds Unix millisecond range".to_owned())?;
        Ok(Self {
            relay_id,
            relay_endpoint: endpoint.quic_multiaddr.clone(),
            tls_server_name: endpoint.tls_server_name.clone(),
            relay_tls_spki_sha256: endpoint.tls_spki_sha256,
            descriptor_commit: bundle.certificate.descriptor_commit,
            relay_certificate_sha256: Sha256::digest(canonical_bundle).into(),
            directory_snapshot_digest: expected_snapshot_digest,
            valid_until_ms,
        })
    }
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct VpnProfileResponseDto {
    pub available: bool,
    pub relay_endpoint: String,
    pub supported_exit_classes: Vec<String>,
    pub default_exit_class: String,
    pub lease_secs: u64,
    pub dns_push_interval_secs: u64,
    pub meter_family: String,
    #[norito(default)]
    pub route_pushes: Vec<String>,
    #[norito(default)]
    pub excluded_routes: Vec<String>,
    #[norito(default)]
    pub dns_servers: Vec<String>,
    #[norito(default)]
    pub tunnel_addresses: Vec<String>,
    pub mtu_bytes: u64,
    pub display_billing_label: String,
    pub fee_asset_id: String,
    pub escrow_account_id: String,
    pub operator_account_id: String,
    pub lease_fee: Quantity,
    pub settlement_grace_secs: u64,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_id_hex: String,
    pub descriptor_commit_hex: String,
    pub tls_server_name: String,
    pub relay_tls_spki_sha256_hex: String,
    pub relay_certificate_sha256_hex: String,
    pub directory_snapshot_digest_hex: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct VpnQuoteCreateRequestDto {
    #[norito(default)]
    pub exit_class: String,
    #[norito(default)]
    pub metering_public_key_hex: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct VpnQuoteResponseDto {
    pub quote_id: String,
    pub lease_id_hex: String,
    pub session_id_hex: String,
    pub payment_reference: String,
    pub account_id: String,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub lease_secs: u64,
    pub quote_expires_at_ms: u64,
    pub fee_asset_id: String,
    pub escrow_account_id: String,
    pub operator_account_id: String,
    pub lease_fee: Quantity,
    pub route_pushes: Vec<String>,
    pub excluded_routes: Vec<String>,
    pub dns_servers: Vec<String>,
    pub tunnel_addresses: Vec<String>,
    pub mtu_bytes: u64,
    pub meter_family: String,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_id_hex: String,
    pub descriptor_commit_hex: String,
    pub tls_server_name: String,
    pub relay_tls_spki_sha256_hex: String,
    pub relay_certificate_sha256_hex: String,
    pub directory_snapshot_digest_hex: String,
    pub metering_public_key_hex: String,
    #[norito(default)]
    pub open_lease_instruction: Option<VpnTxInstructionDto>,
    #[norito(default)]
    pub tx_instructions: Vec<VpnTxInstructionDto>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct VpnSessionCreateRequestDto {
    #[norito(default)]
    pub exit_class: String,
    #[norito(default)]
    pub quote_id: String,
    #[norito(default)]
    pub payment_tx_hash: String,
    #[norito(default)]
    pub metering_public_key_hex: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct VpnSessionResponseDto {
    pub session_id: String,
    pub account_id: String,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub lease_secs: u64,
    pub expires_at_ms: u64,
    pub connected_at_ms: u64,
    pub meter_family: String,
    pub quote_id: String,
    pub payment_reference: String,
    pub payment_tx_hash: String,
    pub fee_asset_id: String,
    pub escrow_account_id: String,
    pub operator_account_id: String,
    pub lease_fee: Quantity,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_id_hex: String,
    pub descriptor_commit_hex: String,
    pub tls_server_name: String,
    pub relay_tls_spki_sha256_hex: String,
    pub relay_certificate_sha256_hex: String,
    pub directory_snapshot_digest_hex: String,
    #[norito(default)]
    pub route_pushes: Vec<String>,
    #[norito(default)]
    pub excluded_routes: Vec<String>,
    #[norito(default)]
    pub dns_servers: Vec<String>,
    #[norito(default)]
    pub tunnel_addresses: Vec<String>,
    pub mtu_bytes: u64,
    pub helper_ticket_hex: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub status: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct VpnReceiptResponseDto {
    pub session_id: String,
    pub account_id: String,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub meter_family: String,
    pub connected_at_ms: u64,
    pub disconnected_at_ms: u64,
    pub duration_ms: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub status: String,
    pub receipt_source: String,
    pub quote_id: String,
    pub payment_tx_hash: String,
    pub fee_asset_id: String,
    pub escrow_account_id: String,
    pub operator_account_id: String,
    pub lease_fee: Quantity,
    pub earned_fee: Quantity,
    pub refunded_fee: Quantity,
    #[norito(default)]
    pub lease_id_hex: String,
    #[norito(default)]
    pub settle_lease_instruction: Option<VpnTxInstructionDto>,
    #[norito(default)]
    pub tx_instructions: Vec<VpnTxInstructionDto>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
#[norito(deny_unknown_fields)]
pub struct VpnReceiptSubmitRequestDto {
    pub relay_receipt_hex: String,
    pub client_voucher_hex: String,
    #[norito(default)]
    pub lease_id_hex: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct VpnTxInstructionDto {
    pub wire_id: String,
    pub payload_hex: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    PartialEq,
    Eq,
)]
pub struct VpnReceiptListResponseDto {
    #[norito(default)]
    pub items: Vec<VpnReceiptResponseDto>,
    pub total: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct VpnSessionRecord {
    pub session_id: String,
    pub account_id: AccountId,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub lease_secs: u64,
    pub expires_at_ms: u64,
    pub connected_at_ms: u64,
    pub meter_family: String,
    pub quote_id: String,
    pub payment_reference: String,
    pub payment_tx_hash: String,
    pub fee_asset_id: String,
    pub escrow_account_id: AccountId,
    pub operator_account_id: AccountId,
    pub lease_fee: Quantity,
    pub tariff: VpnTariffV1,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_id: [u8; 32],
    pub descriptor_commit: [u8; 32],
    pub tls_server_name: String,
    pub relay_tls_spki_sha256: [u8; 32],
    pub relay_certificate_sha256: [u8; 32],
    pub directory_snapshot_digest: [u8; 32],
    pub relay_trust_valid_until_ms: u64,
    pub metering_public_key: PublicKey,
    pub route_pushes: Vec<String>,
    pub excluded_routes: Vec<String>,
    pub dns_servers: Vec<String>,
    pub tunnel_addresses: Vec<String>,
    pub mtu_bytes: u64,
    pub helper_ticket_hex: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct VpnReceiptRecord {
    pub session_id: String,
    pub account_id: AccountId,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub meter_family: String,
    pub connected_at_ms: u64,
    pub disconnected_at_ms: u64,
    pub duration_ms: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub status: String,
    pub receipt_source: String,
    pub quote_id: String,
    pub payment_tx_hash: String,
    pub fee_asset_id: String,
    pub escrow_account_id: AccountId,
    pub operator_account_id: AccountId,
    pub lease_fee: Quantity,
    pub earned_fee: Quantity,
    pub refunded_fee: Quantity,
    pub lease_id_hex: String,
    pub settle_lease_instruction: Option<VpnTxInstructionDto>,
}

#[derive(Debug, Clone)]
pub(crate) struct VpnQuoteRecord {
    pub quote_id: String,
    pub account_id: AccountId,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub lease_secs: u64,
    pub quote_expires_at_ms: u64,
    pub payment_reference: String,
    pub fee_asset_id: String,
    pub escrow_account_id: AccountId,
    pub operator_account_id: AccountId,
    pub lease_fee: Quantity,
    pub tariff: VpnTariffV1,
    pub settlement_grace_ms: u64,
    pub metering_public_key: PublicKey,
    pub route_pushes: Vec<String>,
    pub excluded_routes: Vec<String>,
    pub dns_servers: Vec<String>,
    pub tunnel_addresses: Vec<String>,
    pub mtu_bytes: u64,
    pub meter_family: String,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_id: [u8; 32],
    pub descriptor_commit: [u8; 32],
    pub tls_server_name: String,
    pub relay_tls_spki_sha256: [u8; 32],
    pub relay_certificate_sha256: [u8; 32],
    pub directory_snapshot_digest: [u8; 32],
    pub relay_trust_valid_until_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn conversion_error(message: impl Into<String>) -> Error {
    Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
        message.into(),
    )))
}

fn not_permitted_error(message: impl Into<String>) -> Error {
    Error::Query(ValidationFail::NotPermitted(message.into()))
}

fn inconsistent_vpn_state(message: impl Into<String>) -> Error {
    Error::AppServiceUnavailable {
        code: "vpn_state_inconsistent",
        message: message.into(),
    }
}

fn normalize_exit_class(value: &str, default_value: &str) -> Result<String, Error> {
    let candidate = if value.trim().is_empty() {
        default_value.trim()
    } else {
        value.trim()
    };
    let parsed = VpnExitClassV1::try_from_label(candidate).map_err(|err| {
        conversion_error(format!(
            "exit_class must be one of {} ({err})",
            SUPPORTED_EXIT_CLASSES.join(", ")
        ))
    })?;
    Ok(parsed.as_label().to_owned())
}

fn default_tunnel_addresses() -> Vec<String> {
    DEFAULT_TUNNEL_ADDRESSES
        .iter()
        .map(|item| (*item).to_owned())
        .collect()
}

fn build_profile_at(
    dto: &ConfigGetDTO,
    trust: Option<&VpnRelayTrust>,
    current_ms: u64,
) -> VpnProfileResponseDto {
    let vpn = &dto.network.soranet_vpn;
    let trust = trust.filter(|trust| {
        vpn.enabled
            && vpn
                .lease_secs
                .checked_mul(1_000)
                .and_then(|duration_ms| current_ms.checked_add(duration_ms))
                .is_some_and(|lease_end_ms| lease_end_ms <= trust.valid_until_ms)
    });
    let relay_endpoint = trust
        .map(|trust| trust.relay_endpoint.clone())
        .unwrap_or_default();
    let default_exit_class = vpn.exit_class.trim().to_owned();
    let supported_exit_classes = SUPPORTED_EXIT_CLASSES
        .iter()
        .map(|item| (*item).to_owned())
        .collect::<Vec<_>>();
    VpnProfileResponseDto {
        available: trust.is_some(),
        relay_endpoint,
        supported_exit_classes,
        default_exit_class: default_exit_class.clone(),
        lease_secs: vpn.lease_secs,
        dns_push_interval_secs: vpn.dns_push_interval_secs,
        meter_family: vpn.meter_family.clone(),
        route_pushes: vpn.route_pushes.clone(),
        excluded_routes: vpn.excluded_routes.clone(),
        dns_servers: vpn.dns_servers.clone(),
        tunnel_addresses: default_tunnel_addresses(),
        mtu_bytes: u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES),
        display_billing_label: format!(
            "{default_exit_class} · {} · {} XOR",
            vpn.meter_family, vpn.lease_fee
        ),
        fee_asset_id: vpn.fee_asset_id.clone(),
        escrow_account_id: vpn.escrow_account_id.clone(),
        operator_account_id: vpn.operator_account_id.clone(),
        lease_fee: vpn.lease_fee.clone(),
        settlement_grace_secs: vpn.settlement_grace_secs,
        flow_label_bits: vpn.flow_label_bits,
        padding_budget_ms: vpn.padding_budget_ms,
        relay_id_hex: trust
            .map(|trust| hex::encode(trust.relay_id))
            .unwrap_or_default(),
        descriptor_commit_hex: trust
            .map(|trust| hex::encode(trust.descriptor_commit))
            .unwrap_or_default(),
        tls_server_name: trust
            .map(|trust| trust.tls_server_name.clone())
            .unwrap_or_default(),
        relay_tls_spki_sha256_hex: trust
            .map(|trust| hex::encode(trust.relay_tls_spki_sha256))
            .unwrap_or_default(),
        relay_certificate_sha256_hex: trust
            .map(|trust| hex::encode(trust.relay_certificate_sha256))
            .unwrap_or_default(),
        directory_snapshot_digest_hex: trust
            .map(|trust| hex::encode(trust.directory_snapshot_digest))
            .unwrap_or_default(),
    }
}

fn require_signed_request(
    app: &SharedAppState,
    headers: &HeaderMap,
    method: &Method,
    uri: &Uri,
    body: &[u8],
) -> Result<AccountId, Error> {
    match crate::app_auth::verify_canonical_request(&app.state, headers, method, uri, body, None)? {
        Some(verified) => Ok(verified.account),
        None => Err(not_permitted_error("signed account headers are required")),
    }
}

fn session_id_seed(account_id: &AccountId, exit_class: &str, nonce: &str, now_ms: u64) -> String {
    format!("{account_id}:{exit_class}:{nonce}:{now_ms}")
}

fn build_session_id(account_id: &AccountId, exit_class: &str, nonce: &str, now_ms: u64) -> String {
    hex::encode(
        blake3::hash(session_id_seed(account_id, exit_class, nonce, now_ms).as_bytes()).as_bytes(),
    )
}

fn normalize_allocation_nonce(base_nonce: &str, attempt: u32) -> String {
    if attempt == 0 {
        return base_nonce.to_owned();
    }
    format!("{base_nonce}:vpn-attempt-{attempt}")
}

fn relay_session_id_from_session_id(session_id: &str) -> [u8; 16] {
    let digest = blake3::hash(session_id.as_bytes());
    let mut session_key = [0u8; 16];
    session_key.copy_from_slice(&digest.as_bytes()[..16]);
    session_key
}

fn fixed_hash_hex(input: &str) -> String {
    hex::encode(blake3::hash(input.as_bytes()).as_bytes())
}

fn fixed_hash_bytes(input: &str) -> [u8; 32] {
    *blake3::hash(input.as_bytes()).as_bytes()
}

fn account_hash(account_id: &AccountId) -> [u8; 32] {
    fixed_hash_bytes(&account_id.to_string())
}

fn decode_hex_32(raw: &str, field: &str) -> Result<[u8; 32], Error> {
    let normalized = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    let decoded =
        hex::decode(normalized).map_err(|err| conversion_error(format!("{field}: {err}")))?;
    decoded
        .try_into()
        .map_err(|_| conversion_error(format!("{field} must decode to 32 bytes")))
}

fn parse_metering_public_key(raw: &str) -> Result<PublicKey, Error> {
    let normalized = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    if normalized.is_empty() {
        return Err(conversion_error(
            "metering_public_key_hex must not be empty",
        ));
    }
    PublicKey::from_hex(Algorithm::Ed25519, normalized).map_err(|err| {
        conversion_error(format!(
            "metering_public_key_hex must be an Ed25519 public key hex payload: {err}"
        ))
    })
}

fn public_key_payload_hex(public_key: &PublicKey) -> Result<String, Error> {
    let (_, payload) = public_key
        .try_to_bytes()
        .map_err(|err| conversion_error(format!("metering public key is malformed: {err}")))?;
    Ok(hex::encode(payload))
}

fn xor_asset_definition_id() -> AssetDefinitionId {
    let domain =
        DomainId::parse_fully_qualified("universal.universal").expect("static XOR domain id");
    let name = Name::from_str("xor").expect("static XOR asset name");
    AssetDefinitionId::derive_from_components(domain, name)
}

fn parse_profile_account_id(raw: &str, field: &str) -> Result<AccountId, Error> {
    AccountId::parse_encoded(raw.trim())
        .map(|parsed| parsed.into_account_id())
        .map_err(|err| conversion_error(format!("{field} must be a canonical account id: {err}")))
}

fn parse_fee_asset_definition(raw: &str) -> Result<AssetDefinitionId, Error> {
    let normalized = raw.trim();
    if normalized.eq_ignore_ascii_case("xor#universal")
        || normalized.eq_ignore_ascii_case("xor#universal.universal")
        || normalized == iroha_config::parameters::defaults::soranet::vpn::fee_asset_id()
        || normalized == iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
    {
        return Ok(xor_asset_definition_id());
    }
    normalized.parse::<AssetDefinitionId>().map_err(|err| {
        conversion_error(format!(
            "fee_asset_id must be an asset definition id: {err}"
        ))
    })
}

fn active_fee_per_minute(lease_fee: &Quantity, lease_secs: u64) -> Result<Quantity, Error> {
    if lease_secs == 0 {
        return Err(conversion_error("vpn lease_secs must be greater than zero"));
    }
    lease_fee
        .try_mul_div_decimal_round(
            &Numeric::from(60_u64),
            &Numeric::from(lease_secs),
            9,
            RoundingMode::Ceil,
        )
        .map_err(|err| conversion_error(format!("vpn tariff arithmetic failed: {err}")))
}

fn vpn_tariff_for_lease(lease_fee: &Quantity, lease_secs: u64) -> Result<VpnTariffV1, Error> {
    Ok(VpnTariffV1 {
        lease_fee: lease_fee.clone(),
        active_fee_per_minute: active_fee_per_minute(lease_fee, lease_secs)?,
        ingress_fee_per_mib: Quantity::zero(),
        egress_fee_per_mib: Quantity::zero(),
    })
}

fn build_helper_ticket_hex(
    record: &VpnSessionRecord,
    expires_at_ms: u64,
    secret: Option<&[u8; 32]>,
) -> Result<String, Error> {
    let secret = secret.ok_or_else(|| {
        not_permitted_error("vpn helper ticket secret is not configured on this Torii node")
    })?;
    VpnHelperTicketV1 {
        session_id: relay_session_id_from_session_id(&record.session_id),
        quote_id: decode_hex_32(&record.quote_id, "quote_id")?,
        account_hash: account_hash(&record.account_id),
        relay_id: record.relay_id,
        payment_tx_hash: decode_hex_32(&record.payment_tx_hash, "payment_tx_hash")?,
        metering_public_key: record.metering_public_key.clone(),
        tariff: record.tariff.clone(),
        expires_at_ms,
    }
    .try_to_hex(secret)
    .map_err(|err| conversion_error(format!("invalid vpn helper ticket: {err}")))
}

fn build_quote_id(
    account_id: &AccountId,
    exit_class: &str,
    nonce: &str,
    current_ms: u64,
) -> String {
    fixed_hash_hex(&format!(
        "soranet-vpn-quote-v1:{account_id}:{exit_class}:{nonce}:{current_ms}"
    ))
}

fn build_session_id_from_quote(quote: &VpnQuoteRecord, payment_tx_hash: &str) -> String {
    let _ = payment_tx_hash;
    quote.quote_id.clone()
}

fn default_lease_id_hex(record: &VpnSessionRecord) -> String {
    record.quote_id.clone()
}

fn tx_instr_from_box(boxed: InstructionBox) -> VpnTxInstructionDto {
    use iroha_data_model::isi::Instruction;

    let type_name = Instruction::id(&*boxed);
    let wire_id = type_name.to_string();
    let payload = Instruction::dyn_encode(&*boxed);
    let framed = iroha_data_model::isi::frame_instruction_payload(type_name, &payload)
        .expect("instruction payload must use canonical Norito framing");
    VpnTxInstructionDto {
        wire_id,
        payload_hex: hex::encode(framed),
    }
}

fn settle_lease_instruction(
    lease_id: [u8; 32],
    relay_receipt: VpnSessionReceiptV1,
    voucher: VpnUsageVoucherV1,
) -> VpnTxInstructionDto {
    let instruction: InstructionBox = SettleVpnLease::new(lease_id, relay_receipt, voucher).into();
    tx_instr_from_box(instruction)
}

fn quote_policy_from_record(record: &VpnQuoteRecord) -> VpnQuotePolicyV1 {
    VpnQuotePolicyV1 {
        exit_class: VpnExitClassV1::try_from_label(&record.exit_class)
            .expect("quote exit class is normalized"),
        relay_endpoint: record.relay_endpoint.clone(),
        relay_id: record.relay_id,
        descriptor_commit: record.descriptor_commit,
        tls_server_name: record.tls_server_name.clone(),
        relay_tls_spki_sha256: record.relay_tls_spki_sha256,
        relay_certificate_sha256: record.relay_certificate_sha256,
        directory_snapshot_digest: record.directory_snapshot_digest,
        relay_trust_valid_until_ms: record.relay_trust_valid_until_ms,
        lease_secs: record.lease_secs,
        meter_family: record.meter_family.clone(),
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.clone(),
        route_pushes: record.route_pushes.clone(),
        excluded_routes: record.excluded_routes.clone(),
        dns_servers: record.dns_servers.clone(),
        tunnel_addresses: record.tunnel_addresses.clone(),
        mtu_bytes: record.mtu_bytes,
        flow_label_bits: record.flow_label_bits,
        padding_budget_ms: record.padding_budget_ms,
    }
}

fn open_lease_instruction(record: &VpnQuoteRecord) -> Result<VpnTxInstructionDto, Error> {
    let lease_id = decode_hex_32(&record.quote_id, "quote_id")?;
    let asset_definition = parse_fee_asset_definition(&record.fee_asset_id)?;
    let instruction: InstructionBox = OpenVpnLeaseEscrow::new(
        lease_id,
        relay_session_id_from_session_id(&record.quote_id),
        lease_id,
        record.relay_id,
        record.operator_account_id.clone(),
        record.metering_public_key.clone(),
        asset_definition,
        record.lease_fee.clone(),
        record.tariff.clone(),
        quote_policy_from_record(record),
        record.quote_expires_at_ms,
        record.settlement_grace_ms,
    )
    .into();
    Ok(tx_instr_from_box(instruction))
}

fn settlement_lease_id_from_request_or_receipt(
    request: &VpnReceiptSubmitRequestDto,
    relay_receipt: &VpnSessionReceiptV1,
) -> Result<([u8; 32], String), Error> {
    let lease_id_hex = request.lease_id_hex.trim();
    if lease_id_hex.is_empty() {
        let fallback = hex::encode(relay_receipt.quote_id);
        return Ok((relay_receipt.quote_id, fallback));
    }
    let lease_id = decode_hex_32(lease_id_hex, "lease_id_hex")?;
    Ok((lease_id, hex::encode(lease_id)))
}

fn address_plan_fingerprint(client_tunnel_addresses: &[String]) -> String {
    client_tunnel_addresses.join("|")
}

fn quote_response_from_record(record: &VpnQuoteRecord) -> Result<VpnQuoteResponseDto, Error> {
    let open_lease_instruction = open_lease_instruction(record)?;
    let tx_instructions = vec![open_lease_instruction.clone()];
    Ok(VpnQuoteResponseDto {
        quote_id: record.quote_id.clone(),
        lease_id_hex: record.quote_id.clone(),
        session_id_hex: hex::encode(relay_session_id_from_session_id(&record.quote_id)),
        payment_reference: record.payment_reference.clone(),
        account_id: record.account_id.to_string(),
        exit_class: record.exit_class.clone(),
        relay_endpoint: record.relay_endpoint.clone(),
        lease_secs: record.lease_secs,
        quote_expires_at_ms: record.quote_expires_at_ms,
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.to_string(),
        operator_account_id: record.operator_account_id.to_string(),
        lease_fee: record.lease_fee.clone(),
        route_pushes: record.route_pushes.clone(),
        excluded_routes: record.excluded_routes.clone(),
        dns_servers: record.dns_servers.clone(),
        tunnel_addresses: record.tunnel_addresses.clone(),
        mtu_bytes: record.mtu_bytes,
        meter_family: record.meter_family.clone(),
        flow_label_bits: record.flow_label_bits,
        padding_budget_ms: record.padding_budget_ms,
        relay_id_hex: hex::encode(record.relay_id),
        descriptor_commit_hex: hex::encode(record.descriptor_commit),
        tls_server_name: record.tls_server_name.clone(),
        relay_tls_spki_sha256_hex: hex::encode(record.relay_tls_spki_sha256),
        relay_certificate_sha256_hex: hex::encode(record.relay_certificate_sha256),
        directory_snapshot_digest_hex: hex::encode(record.directory_snapshot_digest),
        metering_public_key_hex: public_key_payload_hex(&record.metering_public_key)?,
        open_lease_instruction: Some(open_lease_instruction),
        tx_instructions,
    })
}

fn allocate_session_id_and_address_plan(
    app: &SharedAppState,
    account_id: &AccountId,
    exit_class: &str,
    base_nonce: &str,
    current_ms: u64,
) -> Result<
    (
        String,
        iroha_data_model::soranet::vpn::VpnSessionAddressPlanV1,
    ),
    Error,
> {
    let used_fingerprints = app
        .vpn_sessions
        .iter()
        .map(|entry| {
            let record = entry.value();
            address_plan_fingerprint(&record.tunnel_addresses)
        })
        .collect::<HashSet<_>>();

    for attempt in 0..MAX_SESSION_ADDRESS_ALLOCATION_ATTEMPTS {
        let allocation_nonce = normalize_allocation_nonce(base_nonce, attempt);
        let session_id = build_session_id(account_id, exit_class, &allocation_nonce, current_ms);
        let address_plan =
            derive_vpn_session_address_plan_v1(relay_session_id_from_session_id(&session_id));
        let fingerprint = address_plan_fingerprint(&address_plan.client_tunnel_addresses);
        if !used_fingerprints.contains(&fingerprint) {
            return Ok((session_id, address_plan));
        }
    }

    Err(not_permitted_error(
        "vpn address allocation exhausted for the current active session set",
    ))
}

fn response_from_record(record: &VpnSessionRecord) -> VpnSessionResponseDto {
    VpnSessionResponseDto {
        session_id: record.session_id.clone(),
        account_id: record.account_id.to_string(),
        exit_class: record.exit_class.clone(),
        relay_endpoint: record.relay_endpoint.clone(),
        lease_secs: record.lease_secs,
        expires_at_ms: record.expires_at_ms,
        connected_at_ms: record.connected_at_ms,
        meter_family: record.meter_family.clone(),
        quote_id: record.quote_id.clone(),
        payment_reference: record.payment_reference.clone(),
        payment_tx_hash: record.payment_tx_hash.clone(),
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.to_string(),
        operator_account_id: record.operator_account_id.to_string(),
        lease_fee: record.lease_fee.clone(),
        flow_label_bits: record.flow_label_bits,
        padding_budget_ms: record.padding_budget_ms,
        relay_id_hex: hex::encode(record.relay_id),
        descriptor_commit_hex: hex::encode(record.descriptor_commit),
        tls_server_name: record.tls_server_name.clone(),
        relay_tls_spki_sha256_hex: hex::encode(record.relay_tls_spki_sha256),
        relay_certificate_sha256_hex: hex::encode(record.relay_certificate_sha256),
        directory_snapshot_digest_hex: hex::encode(record.directory_snapshot_digest),
        route_pushes: record.route_pushes.clone(),
        excluded_routes: record.excluded_routes.clone(),
        dns_servers: record.dns_servers.clone(),
        tunnel_addresses: record.tunnel_addresses.clone(),
        mtu_bytes: record.mtu_bytes,
        helper_ticket_hex: record.helper_ticket_hex.clone(),
        bytes_in: record.bytes_in,
        bytes_out: record.bytes_out,
        status: "active".to_owned(),
    }
}

fn receipt_response_from_record(record: &VpnReceiptRecord) -> VpnReceiptResponseDto {
    let tx_instructions = record
        .settle_lease_instruction
        .clone()
        .into_iter()
        .collect();
    VpnReceiptResponseDto {
        session_id: record.session_id.clone(),
        account_id: record.account_id.to_string(),
        exit_class: record.exit_class.clone(),
        relay_endpoint: record.relay_endpoint.clone(),
        meter_family: record.meter_family.clone(),
        connected_at_ms: record.connected_at_ms,
        disconnected_at_ms: record.disconnected_at_ms,
        duration_ms: record.duration_ms,
        bytes_in: record.bytes_in,
        bytes_out: record.bytes_out,
        status: record.status.clone(),
        receipt_source: record.receipt_source.clone(),
        quote_id: record.quote_id.clone(),
        payment_tx_hash: record.payment_tx_hash.clone(),
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.to_string(),
        operator_account_id: record.operator_account_id.to_string(),
        lease_fee: record.lease_fee.clone(),
        earned_fee: record.earned_fee.clone(),
        refunded_fee: record.refunded_fee.clone(),
        lease_id_hex: record.lease_id_hex.clone(),
        settle_lease_instruction: record.settle_lease_instruction.clone(),
        tx_instructions,
    }
}

fn build_receipt_record(
    record: &VpnSessionRecord,
    disconnected_at_ms: u64,
    status: &str,
) -> VpnReceiptRecord {
    let duration_ms = disconnected_at_ms.saturating_sub(record.connected_at_ms);
    VpnReceiptRecord {
        session_id: record.session_id.clone(),
        account_id: record.account_id.clone(),
        exit_class: record.exit_class.clone(),
        relay_endpoint: record.relay_endpoint.clone(),
        meter_family: record.meter_family.clone(),
        connected_at_ms: record.connected_at_ms,
        disconnected_at_ms,
        duration_ms,
        bytes_in: record.bytes_in,
        bytes_out: record.bytes_out,
        status: status.to_owned(),
        receipt_source: "torii".to_owned(),
        quote_id: record.quote_id.clone(),
        payment_tx_hash: record.payment_tx_hash.clone(),
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.clone(),
        operator_account_id: record.operator_account_id.clone(),
        lease_fee: record.lease_fee.clone(),
        earned_fee: Quantity::zero(),
        refunded_fee: record.lease_fee.clone(),
        lease_id_hex: default_lease_id_hex(record),
        settle_lease_instruction: None,
    }
}

fn build_settled_receipt_record(
    record: &VpnSessionRecord,
    relay_receipt: &VpnSessionReceiptV1,
    voucher: &VpnUsageVoucherV1,
    lease_id: [u8; 32],
    lease_id_hex: String,
    disconnected_at_ms: u64,
) -> Result<VpnReceiptRecord, Error> {
    let duration_ms = disconnected_at_ms.saturating_sub(record.connected_at_ms);
    let earned_fee = if relay_receipt.earned_fee > record.lease_fee {
        record.lease_fee.clone()
    } else {
        relay_receipt.earned_fee.clone()
    };
    let refunded_fee = record
        .lease_fee
        .checked_sub(&earned_fee)
        .map_err(|err| conversion_error(format!("vpn refund arithmetic failed: {err}")))?;
    Ok(VpnReceiptRecord {
        session_id: record.session_id.clone(),
        account_id: record.account_id.clone(),
        exit_class: record.exit_class.clone(),
        relay_endpoint: record.relay_endpoint.clone(),
        meter_family: record.meter_family.clone(),
        connected_at_ms: record.connected_at_ms,
        disconnected_at_ms,
        duration_ms,
        bytes_in: relay_receipt.ingress_bytes,
        bytes_out: relay_receipt.egress_bytes,
        status: "settled".to_owned(),
        receipt_source: "relay".to_owned(),
        quote_id: record.quote_id.clone(),
        payment_tx_hash: record.payment_tx_hash.clone(),
        fee_asset_id: record.fee_asset_id.clone(),
        escrow_account_id: record.escrow_account_id.clone(),
        operator_account_id: record.operator_account_id.clone(),
        lease_fee: record.lease_fee.clone(),
        earned_fee,
        refunded_fee,
        lease_id_hex,
        settle_lease_instruction: Some(settle_lease_instruction(
            lease_id,
            relay_receipt.clone(),
            voucher.clone(),
        )),
    })
}

fn store_receipt(app: &SharedAppState, receipt: VpnReceiptRecord) {
    let key = receipt.account_id.to_string();
    let mut entry = app.vpn_receipts.entry(key).or_default();
    entry.insert(0, receipt);
    if entry.len() > MAX_RECEIPTS_PER_ACCOUNT {
        entry.truncate(MAX_RECEIPTS_PER_ACCOUNT);
    }
}

fn prune_expired_quotes(app: &SharedAppState, current_ms: u64) {
    let stale_ids = app
        .vpn_quotes
        .iter()
        .filter_map(|entry| {
            if entry.value().quote_expires_at_ms <= current_ms {
                Some(entry.key().clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for quote_id in stale_ids {
        app.vpn_quotes.remove(&quote_id);
    }
}

fn prune_expired_sessions(app: &SharedAppState, current_ms: u64) {
    let stale_ids = app
        .vpn_sessions
        .iter()
        .filter_map(|entry| {
            if entry.value().expires_at_ms <= current_ms {
                Some(entry.key().clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for session_id in stale_ids {
        if let Some((_, record)) = app.vpn_sessions.remove(&session_id) {
            store_receipt(app, build_receipt_record(&record, current_ms, "expired"));
        }
    }
}

fn remove_existing_sessions_for_account(
    app: &SharedAppState,
    account_id: &AccountId,
    current_ms: u64,
) {
    let matching_ids = app
        .vpn_sessions
        .iter()
        .filter_map(|entry| {
            if &entry.value().account_id == account_id {
                Some(entry.key().clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for session_id in matching_ids {
        if let Some((_, record)) = app.vpn_sessions.remove(&session_id) {
            store_receipt(app, build_receipt_record(&record, current_ms, "replaced"));
        }
    }
}

fn list_receipts_for_account(
    app: &SharedAppState,
    account_id: &AccountId,
) -> Result<Vec<VpnReceiptResponseDto>, Error> {
    let mut records = app
        .vpn_receipts
        .get(&account_id.to_string())
        .map(|entry| entry.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    let cached_lease_ids = records
        .iter()
        .map(|record| record.lease_id_hex.clone())
        .collect::<HashSet<_>>();
    let world = app.state.world_view();
    let indexed_leases = world.vpn_settled_leases_by_account().get(account_id);
    for (settled_at_ms, lease_id) in indexed_leases
        .into_iter()
        .flat_map(|leases| leases.iter().rev())
    {
        let lease = world.vpn_leases().get(lease_id).ok_or_else(|| {
            inconsistent_vpn_state(format!(
                "settled VPN receipt index references missing lease {}",
                hex::encode(lease_id)
            ))
        })?;
        if &lease.client_account_id != account_id
            || lease.status != VpnLeaseStatusV1::Settled
            || lease.settled_at_ms != Some(*settled_at_ms)
            || lease.lease_id != *lease_id
        {
            return Err(inconsistent_vpn_state(format!(
                "settled VPN receipt index entry does not match lease {}",
                hex::encode(lease_id)
            )));
        }
        let lease_id_hex = hex::encode(lease.lease_id);
        if cached_lease_ids.contains(&lease_id_hex) {
            continue;
        }
        let record = receipt_record_from_settled_lease(lease)?.ok_or_else(|| {
            inconsistent_vpn_state(format!(
                "settled VPN receipt index references incomplete lease {lease_id_hex}"
            ))
        })?;
        records.push(record);
    }
    records.sort_by(|left, right| right.disconnected_at_ms.cmp(&left.disconnected_at_ms));
    records.truncate(MAX_RECEIPTS_PER_ACCOUNT);
    Ok(records.iter().map(receipt_response_from_record).collect())
}

fn external_signed_transaction_results(
    block: &SignedBlock,
) -> impl Iterator<
    Item = (
        HashOf<TransactionEntrypoint>,
        SignedTransaction,
        &iroha_data_model::transaction::TransactionResult,
    ),
> + '_ {
    let external_total = block.external_entrypoint_count();
    block
        .external_entrypoints_cloned()
        .take(external_total)
        .zip(block.results().take(external_total))
        .filter_map(|(entrypoint, result)| {
            let entrypoint_hash = entrypoint.hash();
            let signed = match entrypoint {
                TransactionEntrypoint::External(signed) => signed,
                TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction().clone(),
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => return None,
            };
            Some((entrypoint_hash, signed, result))
        })
}

fn committed_transaction_by_hash(
    app: &SharedAppState,
    payment_tx_hash: &str,
) -> Result<(SignedTransaction, u64), Error> {
    let target: HashOf<TransactionEntrypoint> = payment_tx_hash
        .trim()
        .parse()
        .map_err(|_| conversion_error("payment_tx_hash must be a transaction hash"))?;
    let target_as_signed = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::from(target));
    let Some(height) = app.state.committed_transaction_height(&target_as_signed) else {
        #[cfg(test)]
        if app.state.committed_height() == 0 {
            return Err(not_permitted_error(
                "test VPN payment bypass is disabled without a quote-bound hash",
            ));
        }
        return Err(not_permitted_error(
            "vpn payment transaction is not committed",
        ));
    };
    let height_u64 = u64::try_from(height.get())
        .map_err(|_| conversion_error("payment transaction height exceeds u64"))?;
    let Some(block) = app.state.block_by_height(height) else {
        return Err(not_permitted_error(
            "vpn payment transaction block is not available",
        ));
    };
    for (entrypoint_hash, tx, result) in external_signed_transaction_results(block.as_ref()) {
        if entrypoint_hash != target {
            continue;
        }
        if result.as_ref().is_err() {
            return Err(not_permitted_error(
                "vpn payment transaction did not commit successfully",
            ));
        }
        return Ok((tx, height_u64));
    }
    Err(not_permitted_error(
        "vpn payment transaction was indexed but not found in its block",
    ))
}

fn open_lease_matches_quote(
    open: &OpenVpnLeaseEscrow,
    quote: &VpnQuoteRecord,
) -> Result<bool, Error> {
    let quote_id = decode_hex_32(&quote.quote_id, "quote_id")?;
    let asset_definition = parse_fee_asset_definition(&quote.fee_asset_id)?;
    Ok(open.lease_id == quote_id
        && open.session_id == relay_session_id_from_session_id(&quote.quote_id)
        && open.quote_id == quote_id
        && open.relay_id == quote.relay_id
        && open.operator_account_id == quote.operator_account_id
        && open.metering_public_key == quote.metering_public_key
        && open.asset_definition == asset_definition
        && open.lease_fee == quote.lease_fee
        && open.tariff == quote.tariff
        && open.quote_policy == quote_policy_from_record(quote)
        && open.expires_at_ms == quote.quote_expires_at_ms
        && open.settlement_grace_ms == quote.settlement_grace_ms)
}

fn verify_vpn_payment(
    app: &SharedAppState,
    quote: &VpnQuoteRecord,
    payment_tx_hash: &str,
) -> Result<(), Error> {
    let payment_hash = payment_tx_hash.trim();
    if payment_hash.is_empty() {
        return Err(conversion_error("payment_tx_hash must not be empty"));
    }
    let _ = decode_hex_32(payment_hash, "payment_tx_hash")?;
    if app.vpn_used_payments.contains_key(payment_hash) {
        return Err(not_permitted_error(
            "vpn payment transaction was already used for a session",
        ));
    }

    #[cfg(test)]
    if app.state.committed_height() == 0 && payment_hash == quote.quote_id {
        return Ok(());
    }

    let (tx, _) = committed_transaction_by_hash(app, payment_hash)?;
    if tx.authority() != &quote.account_id {
        return Err(not_permitted_error(
            "vpn payment transaction authority does not match signed account",
        ));
    }
    let mut matched = false;
    for instruction in tx.instructions().explicit_instructions() {
        let Some(open) = instruction.as_any().downcast_ref::<OpenVpnLeaseEscrow>() else {
            continue;
        };
        if open_lease_matches_quote(open, quote)? {
            matched = true;
            break;
        }
    }
    if !matched {
        return Err(not_permitted_error(
            "vpn payment transaction must explicitly open the quoted native XOR VPN lease escrow",
        ));
    }
    Ok(())
}

fn decode_norito_hex<T: norito::codec::Decode>(raw: &str, field: &str) -> Result<T, Error> {
    let normalized = raw.trim().trim_start_matches("0x").trim_start_matches("0X");
    let bytes =
        hex::decode(normalized).map_err(|err| conversion_error(format!("{field}: {err}")))?;
    let mut cursor = bytes.as_slice();
    let decoded = norito::codec::Decode::decode(&mut cursor)
        .map_err(|err| conversion_error(format!("{field} is not valid Norito: {err}")))?;
    if !cursor.is_empty() {
        return Err(conversion_error(format!(
            "{field} has trailing bytes after Norito payload"
        )));
    }
    Ok(decoded)
}

fn lease_record_by_id(app: &SharedAppState, lease_id: &[u8; 32]) -> Option<VpnLeaseRecordV1> {
    let world = app.state.world_view();
    world.vpn_leases().get(lease_id).cloned()
}

fn lease_id_from_session_lookup(session_id: &str) -> Option<[u8; 32]> {
    let normalized = session_id
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if normalized.len() != 64 {
        return None;
    }
    let decoded = hex::decode(normalized).ok()?;
    decoded.try_into().ok()
}

fn session_id_hex_from_lease(record: &VpnLeaseRecordV1) -> String {
    hex::encode(record.quote_id)
}

fn ensure_lease_matches_authenticated_trust(
    record: &VpnLeaseRecordV1,
    trust: &VpnRelayTrust,
) -> Result<(), Error> {
    let policy = &record.quote_policy;
    if record.relay_id != trust.relay_id
        || policy.relay_id != trust.relay_id
        || policy.relay_endpoint != trust.relay_endpoint
        || policy.descriptor_commit != trust.descriptor_commit
        || policy.tls_server_name != trust.tls_server_name
        || policy.relay_tls_spki_sha256 != trust.relay_tls_spki_sha256
        || policy.relay_certificate_sha256 != trust.relay_certificate_sha256
        || policy.directory_snapshot_digest != trust.directory_snapshot_digest
        || policy.relay_trust_valid_until_ms != trust.valid_until_ms
        || record.expires_at_ms > trust.valid_until_ms
    {
        return Err(not_permitted_error(
            "persisted VPN lease does not match the authenticated relay trust",
        ));
    }
    Ok(())
}

fn session_record_from_lease(record: &VpnLeaseRecordV1) -> Result<VpnSessionRecord, Error> {
    let policy = &record.quote_policy;
    if record.relay_id != policy.relay_id {
        return Err(not_permitted_error(
            "vpn lease relay id does not match its authenticated quote policy",
        ));
    }
    if record.expires_at_ms > policy.relay_trust_valid_until_ms {
        return Err(not_permitted_error(
            "vpn relay trust does not cover the complete persisted lease",
        ));
    }
    Ok(VpnSessionRecord {
        session_id: session_id_hex_from_lease(record),
        account_id: record.client_account_id.clone(),
        exit_class: policy.exit_class.as_label().to_owned(),
        relay_endpoint: policy.relay_endpoint.clone(),
        lease_secs: policy.lease_secs,
        expires_at_ms: record.expires_at_ms,
        connected_at_ms: record.opened_at_ms,
        meter_family: policy.meter_family.clone(),
        quote_id: hex::encode(record.quote_id),
        payment_reference: hex::encode(record.quote_id),
        payment_tx_hash: hex::encode(record.open_tx_hash),
        fee_asset_id: policy.fee_asset_id.clone(),
        escrow_account_id: policy.escrow_account_id.clone(),
        operator_account_id: record.operator_account_id.clone(),
        lease_fee: record.lease_fee.clone(),
        tariff: record.tariff.clone(),
        flow_label_bits: policy.flow_label_bits,
        padding_budget_ms: policy.padding_budget_ms,
        relay_id: policy.relay_id,
        descriptor_commit: policy.descriptor_commit,
        tls_server_name: policy.tls_server_name.clone(),
        relay_tls_spki_sha256: policy.relay_tls_spki_sha256,
        relay_certificate_sha256: policy.relay_certificate_sha256,
        directory_snapshot_digest: policy.directory_snapshot_digest,
        relay_trust_valid_until_ms: policy.relay_trust_valid_until_ms,
        metering_public_key: record.metering_public_key.clone(),
        route_pushes: policy.route_pushes.clone(),
        excluded_routes: policy.excluded_routes.clone(),
        dns_servers: policy.dns_servers.clone(),
        tunnel_addresses: policy.tunnel_addresses.clone(),
        mtu_bytes: policy.mtu_bytes,
        helper_ticket_hex: String::new(),
        bytes_in: 0,
        bytes_out: 0,
    })
}

fn active_session_record_from_wsv(
    app: &SharedAppState,
    session_id: &str,
    current_ms: u64,
) -> Result<Option<VpnSessionRecord>, Error> {
    let Some(lease_id) = lease_id_from_session_lookup(session_id) else {
        return Ok(None);
    };
    let Some(lease_record) = lease_record_by_id(app, &lease_id) else {
        return Ok(None);
    };
    if lease_record.status != VpnLeaseStatusV1::Active || lease_record.expires_at_ms <= current_ms {
        return Ok(None);
    }
    let trust = app.vpn_relay_trust.as_deref().ok_or_else(|| {
        not_permitted_error("vpn relay trust is not configured on this Torii node")
    })?;
    ensure_lease_matches_authenticated_trust(&lease_record, trust)?;
    let mut record = session_record_from_lease(&lease_record)?;
    record.helper_ticket_hex = build_helper_ticket_hex(
        &record,
        record.expires_at_ms,
        app.vpn_helper_ticket_secret.as_ref(),
    )?;
    Ok(Some(record))
}

fn receipt_record_from_settled_lease(
    record: &VpnLeaseRecordV1,
) -> Result<Option<VpnReceiptRecord>, Error> {
    if record.status != VpnLeaseStatusV1::Settled {
        return Ok(None);
    }
    let Some(relay_receipt) = record.settled_relay_receipt.as_ref() else {
        return Ok(None);
    };
    let session = session_record_from_lease(record)?;
    let connected_at_ms = relay_receipt.started_at_ms.max(record.opened_at_ms);
    let disconnected_at_ms = record.settled_at_ms.unwrap_or(relay_receipt.ended_at_ms);
    let duration_ms = relay_receipt
        .ended_at_ms
        .saturating_sub(relay_receipt.started_at_ms);
    Ok(Some(VpnReceiptRecord {
        session_id: session.session_id,
        account_id: session.account_id,
        exit_class: session.exit_class,
        relay_endpoint: session.relay_endpoint,
        meter_family: session.meter_family,
        connected_at_ms,
        disconnected_at_ms,
        duration_ms,
        bytes_in: relay_receipt.ingress_bytes,
        bytes_out: relay_receipt.egress_bytes,
        status: "settled".to_owned(),
        receipt_source: "wsv".to_owned(),
        quote_id: session.quote_id,
        payment_tx_hash: session.payment_tx_hash,
        fee_asset_id: session.fee_asset_id,
        escrow_account_id: session.escrow_account_id,
        operator_account_id: session.operator_account_id,
        lease_fee: record.lease_fee.clone(),
        earned_fee: record.earned_fee.clone(),
        refunded_fee: record.refunded_fee.clone(),
        lease_id_hex: hex::encode(record.lease_id),
        settle_lease_instruction: None,
    }))
}

fn verify_relay_receipt_for_session(
    record: &VpnSessionRecord,
    relay_receipt: &VpnSessionReceiptV1,
    voucher: &VpnUsageVoucherV1,
) -> Result<(), Error> {
    let expected_session_id = relay_session_id_from_session_id(&record.session_id);
    if relay_receipt.session_id != expected_session_id
        || voucher.body.session_id != expected_session_id
    {
        return Err(not_permitted_error(
            "vpn receipt session id does not match the active session",
        ));
    }
    let expected_quote_id = decode_hex_32(&record.quote_id, "quote_id")?;
    if relay_receipt.quote_id != expected_quote_id || voucher.body.quote_id != expected_quote_id {
        return Err(not_permitted_error(
            "vpn receipt quote id does not match the active session",
        ));
    }
    let expected_payment_hash = decode_hex_32(&record.payment_tx_hash, "payment_tx_hash")?;
    if relay_receipt.payment_tx_hash != expected_payment_hash {
        return Err(not_permitted_error(
            "vpn receipt payment hash does not match the active session",
        ));
    }
    let expected_account_hash = account_hash(&record.account_id);
    if relay_receipt.account_hash != expected_account_hash {
        return Err(not_permitted_error(
            "vpn receipt account hash does not match the active session",
        ));
    }
    let expected_relay_id = record.relay_id;
    if relay_receipt.relay_id != expected_relay_id || voucher.body.relay_id != expected_relay_id {
        return Err(not_permitted_error(
            "vpn receipt relay id does not match the configured relay",
        ));
    }
    if relay_receipt.client_voucher_hash != voucher.hash() {
        return Err(not_permitted_error(
            "vpn receipt does not commit to the submitted client voucher",
        ));
    }
    if relay_receipt.highest_voucher_sequence != voucher.body.sequence {
        return Err(not_permitted_error(
            "vpn receipt voucher sequence does not match the submitted voucher",
        ));
    }
    if voucher.client_public_key != record.metering_public_key {
        return Err(not_permitted_error(
            "vpn usage voucher public key does not match the session",
        ));
    }
    voucher
        .verify()
        .map_err(|err| not_permitted_error(format!("vpn usage voucher signature failed: {err}")))?;
    if relay_receipt.ingress_bytes != voucher.body.ingress_bytes
        || relay_receipt.egress_bytes != voucher.body.egress_bytes
    {
        return Err(not_permitted_error(
            "vpn receipt byte counters do not match the submitted voucher",
        ));
    }
    if u64::from(relay_receipt.uptime_secs).saturating_mul(1_000) < voucher.body.active_ms {
        return Err(not_permitted_error(
            "vpn receipt uptime is below the submitted voucher active time",
        ));
    }
    if relay_receipt.ended_at_ms < relay_receipt.started_at_ms {
        return Err(not_permitted_error(
            "vpn receipt end timestamp precedes start timestamp",
        ));
    }

    let expected_earned_fee = session_earned_fee(record, voucher)?;
    if relay_receipt.earned_fee != expected_earned_fee {
        return Err(not_permitted_error(
            "vpn receipt earned fee does not match the session tariff",
        ));
    }
    Ok(())
}

fn session_earned_fee(
    record: &VpnSessionRecord,
    voucher: &VpnUsageVoucherV1,
) -> Result<Quantity, Error> {
    record
        .tariff
        .earned_fee(&voucher.body)
        .map_err(|err| conversion_error(format!("vpn tariff arithmetic failed: {err}")))
}

pub(crate) async fn handle_get_vpn_profile(app: SharedAppState) -> Result<Response, Error> {
    let dto = app.kiso.get_dto().await?;
    Ok(crate::utils::JsonBody(build_profile_at(
        &dto,
        app.vpn_relay_trust.as_deref(),
        now_ms(),
    ))
    .into_response())
}

pub(crate) async fn handle_create_vpn_quote(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response, Error> {
    let account_id = require_signed_request(&app, headers, method, uri, body)?;
    let request: VpnQuoteCreateRequestDto = norito::json::from_slice(body)
        .map_err(|err| conversion_error(format!("invalid vpn quote create payload: {err}")))?;
    let dto = app.kiso.get_dto().await?;
    let current_ms = now_ms();
    let profile = build_profile_at(&dto, app.vpn_relay_trust.as_deref(), current_ms);
    if !profile.available {
        if dto.network.soranet_vpn.enabled && app.vpn_relay_trust.is_some() {
            return Err(not_permitted_error(
                "authenticated relay trust does not cover the complete VPN lease",
            ));
        }
        return Err(not_permitted_error("vpn is disabled on this Torii node"));
    }
    if app.vpn_helper_ticket_secret.is_none() {
        return Err(not_permitted_error(
            "vpn helper ticket secret is not configured on this Torii node",
        ));
    }
    let trust = app.vpn_relay_trust.as_deref().ok_or_else(|| {
        not_permitted_error("vpn relay trust is not configured on this Torii node")
    })?;
    let exit_class = normalize_exit_class(&request.exit_class, &profile.default_exit_class)?;
    let escrow_account_id =
        parse_profile_account_id(&profile.escrow_account_id, "escrow_account_id")?;
    let operator_account_id =
        parse_profile_account_id(&profile.operator_account_id, "operator_account_id")?;
    if escrow_account_id == operator_account_id {
        return Err(not_permitted_error(
            "vpn escrow account must be different from the relay operator account",
        ));
    }
    let metering_public_key = parse_metering_public_key(&request.metering_public_key_hex)?;

    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_quotes(&app, current_ms);
    prune_expired_sessions(&app, current_ms);

    let nonce = headers
        .get(crate::HEADER_NONCE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("vpn-quote");
    let quote_id = build_quote_id(&account_id, &exit_class, nonce, current_ms);
    let address_plan =
        derive_vpn_session_address_plan_v1(relay_session_id_from_session_id(&quote_id));
    let lease_duration_ms = profile
        .lease_secs
        .checked_mul(1_000)
        .ok_or_else(|| conversion_error("vpn lease duration overflows milliseconds"))?;
    let quote_expires_at_ms = current_ms
        .checked_add(lease_duration_ms)
        .ok_or_else(|| conversion_error("vpn lease expiry exceeds Unix millisecond range"))?;
    if quote_expires_at_ms > trust.valid_until_ms {
        return Err(not_permitted_error(
            "authenticated relay trust does not cover the complete VPN lease",
        ));
    }
    let tariff = vpn_tariff_for_lease(&profile.lease_fee, profile.lease_secs)?;
    let record = VpnQuoteRecord {
        quote_id: quote_id.clone(),
        account_id,
        exit_class,
        relay_endpoint: profile.relay_endpoint,
        lease_secs: profile.lease_secs,
        quote_expires_at_ms,
        payment_reference: quote_id.clone(),
        fee_asset_id: profile.fee_asset_id,
        escrow_account_id,
        operator_account_id,
        lease_fee: profile.lease_fee,
        tariff,
        settlement_grace_ms: profile.settlement_grace_secs.saturating_mul(1_000),
        metering_public_key,
        route_pushes: profile.route_pushes,
        excluded_routes: profile.excluded_routes,
        dns_servers: profile.dns_servers,
        tunnel_addresses: address_plan.client_tunnel_addresses,
        mtu_bytes: profile.mtu_bytes,
        meter_family: profile.meter_family,
        flow_label_bits: profile.flow_label_bits,
        padding_budget_ms: profile.padding_budget_ms,
        relay_id: trust.relay_id,
        descriptor_commit: trust.descriptor_commit,
        tls_server_name: trust.tls_server_name.clone(),
        relay_tls_spki_sha256: trust.relay_tls_spki_sha256,
        relay_certificate_sha256: trust.relay_certificate_sha256,
        directory_snapshot_digest: trust.directory_snapshot_digest,
        relay_trust_valid_until_ms: trust.valid_until_ms,
    };
    let response = quote_response_from_record(&record)?;
    app.vpn_quotes.insert(quote_id, record);
    Ok((StatusCode::CREATED, crate::utils::JsonBody(response)).into_response())
}

pub(crate) async fn handle_create_vpn_session(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response, Error> {
    let account_id = require_signed_request(&app, headers, method, uri, body)?;
    let request: VpnSessionCreateRequestDto = norito::json::from_slice(body)
        .map_err(|err| conversion_error(format!("invalid vpn session create payload: {err}")))?;
    let dto = app.kiso.get_dto().await?;
    let vpn = &dto.network.soranet_vpn;
    if !vpn.enabled {
        return Err(not_permitted_error("vpn is disabled on this Torii node"));
    }
    let current_ms = now_ms();
    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_quotes(&app, current_ms);
    prune_expired_sessions(&app, current_ms);

    let quote_id = request.quote_id.trim();
    if quote_id.is_empty() {
        return Err(conversion_error("quote_id must not be empty"));
    }
    let Some((_, quote)) = app.vpn_quotes.remove(quote_id) else {
        return Err(not_permitted_error(
            "vpn quote is missing, expired, or already consumed",
        ));
    };
    if quote.quote_expires_at_ms <= current_ms {
        return Err(not_permitted_error("vpn quote has expired"));
    }
    if quote.account_id != account_id {
        return Err(not_permitted_error(
            "vpn quote belongs to a different account",
        ));
    }
    let exit_class = normalize_exit_class(&request.exit_class, &vpn.exit_class)?;
    if quote.exit_class != exit_class {
        return Err(not_permitted_error(
            "vpn quote exit class does not match session request",
        ));
    }
    let metering_public_key = parse_metering_public_key(&request.metering_public_key_hex)?;
    if metering_public_key != quote.metering_public_key {
        return Err(not_permitted_error(
            "vpn session metering key does not match the quoted native lease",
        ));
    }
    if request.payment_tx_hash.trim().is_empty() {
        return Err(conversion_error("payment_tx_hash must not be empty"));
    }
    let payment_tx_hash = hex::encode(decode_hex_32(&request.payment_tx_hash, "payment_tx_hash")?);
    verify_vpn_payment(&app, &quote, &payment_tx_hash)?;

    remove_existing_sessions_for_account(&app, &account_id, current_ms);

    let session_id = build_session_id_from_quote(&quote, &payment_tx_hash);
    let expires_at_ms = quote.quote_expires_at_ms;
    let mut record = VpnSessionRecord {
        session_id: session_id.clone(),
        account_id: quote.account_id,
        exit_class: quote.exit_class,
        relay_endpoint: quote.relay_endpoint,
        lease_secs: quote.lease_secs,
        expires_at_ms,
        connected_at_ms: current_ms,
        meter_family: quote.meter_family,
        quote_id: quote.quote_id,
        payment_reference: quote.payment_reference,
        payment_tx_hash: payment_tx_hash.clone(),
        fee_asset_id: quote.fee_asset_id,
        escrow_account_id: quote.escrow_account_id,
        operator_account_id: quote.operator_account_id,
        lease_fee: quote.lease_fee,
        tariff: quote.tariff,
        flow_label_bits: quote.flow_label_bits,
        padding_budget_ms: quote.padding_budget_ms,
        relay_id: quote.relay_id,
        descriptor_commit: quote.descriptor_commit,
        tls_server_name: quote.tls_server_name,
        relay_tls_spki_sha256: quote.relay_tls_spki_sha256,
        relay_certificate_sha256: quote.relay_certificate_sha256,
        directory_snapshot_digest: quote.directory_snapshot_digest,
        relay_trust_valid_until_ms: quote.relay_trust_valid_until_ms,
        metering_public_key,
        route_pushes: quote.route_pushes,
        excluded_routes: quote.excluded_routes,
        dns_servers: quote.dns_servers,
        tunnel_addresses: quote.tunnel_addresses,
        mtu_bytes: quote.mtu_bytes,
        helper_ticket_hex: String::new(),
        bytes_in: 0,
        bytes_out: 0,
    };
    record.helper_ticket_hex = build_helper_ticket_hex(
        &record,
        expires_at_ms,
        app.vpn_helper_ticket_secret.as_ref(),
    )?;
    let response = response_from_record(&record);
    app.vpn_used_payments.insert(payment_tx_hash, ());
    app.vpn_sessions.insert(session_id, record);
    Ok((StatusCode::CREATED, crate::utils::JsonBody(response)).into_response())
}

pub(crate) async fn handle_get_vpn_session(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    session_id: &str,
) -> Result<Response, Error> {
    let account_id = require_signed_request(&app, headers, method, uri, &[])?;
    let current_ms = now_ms();
    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_sessions(&app, current_ms);
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Err(conversion_error("session_id must not be empty"));
    }
    let record = if let Some(record) = app.vpn_sessions.get(normalized_session_id) {
        record.value().clone()
    } else if let Some(record) =
        active_session_record_from_wsv(&app, normalized_session_id, current_ms)?
    {
        record
    } else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    if record.account_id != account_id {
        return Err(not_permitted_error(
            "vpn session belongs to a different account",
        ));
    }
    Ok(crate::utils::JsonBody(response_from_record(&record)).into_response())
}

pub(crate) async fn handle_delete_vpn_session(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    session_id: &str,
) -> Result<Response, Error> {
    let account_id = require_signed_request(&app, headers, method, uri, &[])?;
    let current_ms = now_ms();
    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_sessions(&app, current_ms);
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Err(conversion_error("session_id must not be empty"));
    }
    let Some(record) = app.vpn_sessions.get(normalized_session_id) else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    if record.account_id != account_id {
        return Err(not_permitted_error(
            "vpn session belongs to a different account",
        ));
    }
    drop(record);
    let (_, removed) = app
        .vpn_sessions
        .remove(normalized_session_id)
        .ok_or_else(|| conversion_error("vpn session disappeared during delete"))?;
    let receipt = build_receipt_record(&removed, current_ms, "disconnected");
    store_receipt(&app, receipt.clone());
    Ok((
        StatusCode::OK,
        crate::utils::JsonBody(receipt_response_from_record(&receipt)),
    )
        .into_response())
}

pub(crate) async fn handle_list_vpn_receipts(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Result<Response, Error> {
    let account_id = require_signed_request(&app, headers, method, uri, &[])?;
    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_sessions(&app, now_ms());
    let items = list_receipts_for_account(&app, &account_id)?;
    let total = u64::try_from(items.len()).unwrap_or(u64::MAX);
    Ok(crate::utils::JsonBody(VpnReceiptListResponseDto { items, total }).into_response())
}

pub(crate) async fn handle_submit_vpn_receipt(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response, Error> {
    let signed_account = require_signed_request(&app, headers, method, uri, body)?;
    let request: VpnReceiptSubmitRequestDto = norito::json::from_slice(body)
        .map_err(|err| conversion_error(format!("invalid vpn receipt payload: {err}")))?;
    let relay_receipt: VpnSessionReceiptV1 =
        decode_norito_hex(&request.relay_receipt_hex, "relay_receipt_hex")?;
    let voucher: VpnUsageVoucherV1 =
        decode_norito_hex(&request.client_voucher_hex, "client_voucher_hex")?;

    let current_ms = now_ms();
    let _vpn_guard = app.vpn_state_lock.lock().await;
    let (lease_id, lease_id_hex) =
        settlement_lease_id_from_request_or_receipt(&request, &relay_receipt)?;
    let lease_record = lease_record_by_id(&app, &lease_id).ok_or_else(|| {
        not_permitted_error("vpn receipt does not match an on-chain native VPN lease")
    })?;
    if lease_record.status != VpnLeaseStatusV1::Active {
        return Err(not_permitted_error("vpn lease is not active"));
    }
    if current_ms > lease_record.refund_available_at_ms() {
        return Err(not_permitted_error(
            "vpn lease settlement grace window expired",
        ));
    }
    let record = session_record_from_lease(&lease_record)?;
    if signed_account != record.operator_account_id {
        return Err(not_permitted_error(
            "vpn receipt submission must be signed by the configured operator account",
        ));
    }
    verify_relay_receipt_for_session(&record, &relay_receipt, &voucher)?;
    app.vpn_sessions.remove(&record.session_id);
    let receipt = build_settled_receipt_record(
        &record,
        &relay_receipt,
        &voucher,
        lease_id,
        lease_id_hex,
        current_ms,
    )?;
    store_receipt(&app, receipt.clone());
    Ok((
        StatusCode::CREATED,
        crate::utils::JsonBody(receipt_response_from_record(&receipt)),
    )
        .into_response())
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use std::{collections::BTreeSet, sync::Arc};

    use axum::{body::to_bytes, response::IntoResponse};
    use iroha_core::state::World;
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        Registrable,
        account::{Account, AccountId},
        domain::{Domain, DomainId},
        soranet::vpn::VpnUsageVoucherBodyV1,
    };
    use norito::codec::Encode;

    use super::*;
    use crate::tests_runtime_handlers::{
        app_auth_test_guard, mk_app_state_for_tests_with_world, signed_app_headers,
        world_with_account,
    };

    fn account_id_for(key_pair: &KeyPair) -> AccountId {
        AccountId::new(key_pair.public_key().clone())
    }

    fn checked_vpn_ed25519_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("test VPN fixture key derivation should succeed")
    }

    fn checked_vpn_account(seed: u8) -> AccountId {
        account_id_for(&checked_vpn_ed25519_keypair(seed))
    }

    fn test_vpn_relay_trust() -> VpnRelayTrust {
        let relay_keypair = checked_vpn_ed25519_keypair(0x55);
        let (_, relay_id) = relay_keypair
            .public_key()
            .try_to_bytes()
            .expect("test relay identity");
        VpnRelayTrust {
            relay_id: relay_id.try_into().expect("32-byte Ed25519 identity"),
            relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
            tls_server_name: "relay.example".to_owned(),
            relay_tls_spki_sha256: [0xAB; 32],
            descriptor_commit: [0xCD; 32],
            relay_certificate_sha256: [0xEF; 32],
            directory_snapshot_digest: [0x42; 32],
            valid_until_ms: u64::MAX,
        }
    }

    #[test]
    fn vpn_relay_trust_rejects_unauthenticated_snapshot_bytes() {
        let error = VpnRelayTrust::from_guard_directory_at(
            b"attacker-controlled directory",
            [0xAA; 32],
            test_vpn_relay_trust().relay_id,
            1,
        )
        .expect_err("directory bytes without the provisioned digest must fail");
        assert!(
            error.contains("snapshot digest mismatch"),
            "unexpected trust error: {error}"
        );
    }

    #[test]
    fn checked_vpn_ed25519_keypair_uses_fallible_seed_derivation() {
        assert_eq!(
            checked_vpn_ed25519_keypair(0x50).algorithm(),
            Algorithm::Ed25519
        );
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
        assert_eq!(
            checked_vpn_account(0x51),
            account_id_for(&checked_vpn_ed25519_keypair(0x51))
        );
    }

    #[test]
    fn active_fee_bounds_only_the_final_minute_ratio() {
        let maximum: Quantity = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
            .parse()
            .expect("signed 512-bit maximum quantity");
        assert_eq!(
            active_fee_per_minute(&maximum, 60)
                .expect("equal minute numerator and lease divisor cancel"),
            maximum
        );
    }

    fn world_with_accounts(accounts: &[AccountId]) -> World {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(
            accounts
                .first()
                .expect("at least one account is required for test world"),
        );
        let accounts = accounts
            .iter()
            .cloned()
            .map(|account_id| Account::new(account_id.clone()).build(&account_id))
            .collect::<Vec<_>>();
        World::with([domain], accounts, [])
    }

    fn vpn_enabled_app_with_operator(
        world: World,
        operator_account_id: &AccountId,
    ) -> SharedAppState {
        let app = mk_app_state_for_tests_with_world(world);
        let mut cfg = crate::test_utils::mk_minimal_root_cfg();
        cfg.network.soranet_vpn.enabled = true;
        cfg.network.soranet_vpn.operator_account_id = operator_account_id.clone();

        let mut inner = match Arc::try_unwrap(app) {
            Ok(inner) => inner,
            Err(_) => panic!("test app should be uniquely owned before VPN reconfiguration"),
        };
        inner.kiso = KisoHandle::mock(&cfg);
        inner.vpn_helper_ticket_secret = Some([0x5A; 32]);
        inner.vpn_relay_trust = Some(Arc::new(test_vpn_relay_trust()));
        Arc::new(inner)
    }

    fn metering_public_key_hex(key_pair: &KeyPair) -> String {
        let (_, payload) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("test metering key is valid");
        hex::encode(payload)
    }

    #[test]
    fn public_key_payload_hex_matches_checked_payload() {
        let key_pair = checked_vpn_ed25519_keypair(0x52);
        let (_, payload) = key_pair
            .public_key()
            .try_to_bytes()
            .expect("test key is valid");

        let encoded = public_key_payload_hex(key_pair.public_key()).expect("payload hex");

        assert_eq!(encoded, hex::encode(payload));
    }

    #[test]
    fn parse_metering_public_key_rejects_inert_or_malformed_ed25519_material() {
        const SMALL_ORDER_POINT: [u8; 32] = [
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0,
        ];
        const NONCANONICAL_IDENTITY: [u8; 32] = [
            0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0x7f,
        ];

        for (label, public_key_bytes) in [
            ("all-zero", [0_u8; 32]),
            ("small-order", SMALL_ORDER_POINT),
            ("noncanonical", NONCANONICAL_IDENTITY),
        ] {
            let error = parse_metering_public_key(&hex::encode(public_key_bytes))
                .expect_err("malformed metering key material must fail closed");

            assert!(
                format!("{error:?}").contains("metering_public_key_hex"),
                "{label} public key rejection should name the field: {error:?}"
            );
        }
    }

    async fn create_quote_for_account(
        app: SharedAppState,
        account: &AccountId,
        key_pair: &KeyPair,
        exit_class: &str,
    ) -> (VpnQuoteResponseDto, KeyPair) {
        let metering_keys = checked_vpn_ed25519_keypair(0x53);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: exit_class.to_owned(),
            metering_public_key_hex: metering_public_key_hex(&metering_keys),
        })
        .expect("quote body");
        let headers = signed_app_headers(account, key_pair, &method, &uri, body.as_ref());
        let response = handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect("quote")
            .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let quote = read_json(response).await;
        (quote, metering_keys)
    }

    async fn create_session_for_quote(
        app: SharedAppState,
        account: &AccountId,
        key_pair: &KeyPair,
        quote: &VpnQuoteResponseDto,
        metering_keys: &KeyPair,
    ) -> VpnSessionResponseDto {
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: quote.exit_class.clone(),
            quote_id: quote.quote_id.clone(),
            payment_tx_hash: quote.quote_id.clone(),
            metering_public_key_hex: metering_public_key_hex(&metering_keys),
        })
        .expect("session body");
        let headers = signed_app_headers(account, key_pair, &method, &uri, body.as_ref());
        let response = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect("session")
            .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        read_json(response).await
    }

    fn sample_session_record(account_id: &AccountId) -> VpnSessionRecord {
        let metering_keys = checked_vpn_ed25519_keypair(0x54);
        let lease_fee = Quantity::from(1_000_000_u64);
        VpnSessionRecord {
            session_id: "session-live".to_owned(),
            account_id: account_id.clone(),
            exit_class: "standard".to_owned(),
            relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
            lease_secs: 600,
            expires_at_ms: 60_000,
            connected_at_ms: 1_000,
            meter_family: "soranet.vpn.standard".to_owned(),
            quote_id: "11".repeat(32),
            payment_reference: "11".repeat(32),
            payment_tx_hash: "22".repeat(32),
            fee_asset_id: iroha_config::parameters::defaults::soranet::vpn::fee_asset_id(),
            escrow_account_id: account_id.clone(),
            operator_account_id: account_id.clone(),
            lease_fee: lease_fee.clone(),
            tariff: vpn_tariff_for_lease(&lease_fee, 600).expect("valid fixture tariff"),
            flow_label_bits: 24,
            padding_budget_ms: 15,
            relay_id: test_vpn_relay_trust().relay_id,
            descriptor_commit: [0xCD; 32],
            tls_server_name: "relay.example".to_owned(),
            relay_tls_spki_sha256: [0xAB; 32],
            relay_certificate_sha256: [0xEF; 32],
            directory_snapshot_digest: [0x42; 32],
            relay_trust_valid_until_ms: u64::MAX,
            metering_public_key: metering_keys.public_key().clone(),
            route_pushes: vec!["0.0.0.0/0".to_owned()],
            excluded_routes: Vec::new(),
            dns_servers: vec!["1.1.1.1".to_owned()],
            tunnel_addresses: default_tunnel_addresses(),
            mtu_bytes: u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES),
            helper_ticket_hex: String::new(),
            bytes_in: 0,
            bytes_out: 0,
        }
    }

    fn lease_record_from_session_record(
        record: &VpnSessionRecord,
        status: VpnLeaseStatusV1,
        relay_receipt: Option<VpnSessionReceiptV1>,
    ) -> VpnLeaseRecordV1 {
        let lease_id = decode_hex_32(&record.quote_id, "quote").expect("quote id");
        let relay_receipt_hash = relay_receipt.as_ref().map(VpnSessionReceiptV1::hash);
        let client_voucher_hash = relay_receipt
            .as_ref()
            .map(|receipt| receipt.client_voucher_hash);
        let earned_fee = relay_receipt
            .as_ref()
            .map(|receipt| receipt.earned_fee.clone())
            .unwrap_or_default();
        let refunded_fee = record
            .lease_fee
            .checked_sub(&earned_fee)
            .expect("fixture earned fee does not exceed its lease fee");
        VpnLeaseRecordV1 {
            lease_id,
            session_id: relay_session_id_from_session_id(&record.session_id),
            quote_id: lease_id,
            client_account_id: record.account_id.clone(),
            operator_account_id: record.operator_account_id.clone(),
            metering_public_key: record.metering_public_key.clone(),
            asset_definition: parse_fee_asset_definition(&record.fee_asset_id).expect("fee asset"),
            lease_fee: record.lease_fee.clone(),
            custody_account_id: record.escrow_account_id.clone(),
            relay_id: record.relay_id,
            tariff: record.tariff.clone(),
            quote_policy: VpnQuotePolicyV1 {
                exit_class: VpnExitClassV1::try_from_label(&record.exit_class).expect("exit class"),
                relay_endpoint: record.relay_endpoint.clone(),
                relay_id: record.relay_id,
                descriptor_commit: record.descriptor_commit,
                tls_server_name: record.tls_server_name.clone(),
                relay_tls_spki_sha256: record.relay_tls_spki_sha256,
                relay_certificate_sha256: record.relay_certificate_sha256,
                directory_snapshot_digest: record.directory_snapshot_digest,
                relay_trust_valid_until_ms: record.relay_trust_valid_until_ms,
                lease_secs: record.lease_secs,
                meter_family: record.meter_family.clone(),
                fee_asset_id: record.fee_asset_id.clone(),
                escrow_account_id: record.escrow_account_id.clone(),
                route_pushes: record.route_pushes.clone(),
                excluded_routes: record.excluded_routes.clone(),
                dns_servers: record.dns_servers.clone(),
                tunnel_addresses: record.tunnel_addresses.clone(),
                mtu_bytes: record.mtu_bytes,
                flow_label_bits: record.flow_label_bits,
                padding_budget_ms: record.padding_budget_ms,
            },
            open_tx_hash: decode_hex_32(&record.payment_tx_hash, "payment").expect("payment hash"),
            status,
            opened_at_ms: record.connected_at_ms,
            expires_at_ms: record.expires_at_ms,
            settlement_grace_ms: 60_000,
            settled_at_ms: (status == VpnLeaseStatusV1::Settled).then(|| {
                relay_receipt
                    .as_ref()
                    .map(|receipt| receipt.ended_at_ms)
                    .unwrap_or(record.expires_at_ms)
            }),
            refunded_at_ms: None,
            highest_voucher_sequence: relay_receipt
                .as_ref()
                .map(|receipt| receipt.highest_voucher_sequence)
                .unwrap_or_default(),
            client_voucher_hash,
            relay_receipt_hash,
            settled_relay_receipt: relay_receipt,
            earned_fee,
            refunded_fee,
        }
    }

    fn settled_lease_for_account(account: &AccountId, ordinal: u16) -> VpnLeaseRecordV1 {
        let mut lease_id = [0_u8; 32];
        lease_id[..2].copy_from_slice(&ordinal.to_be_bytes());
        let mut session = sample_session_record(account);
        session.session_id = hex::encode(lease_id);
        session.quote_id = hex::encode(lease_id);
        session.payment_reference = hex::encode(lease_id);
        let settled_at_ms = 10_000_u64 + u64::from(ordinal);
        let relay_receipt = VpnSessionReceiptV1 {
            session_id: relay_session_id_from_session_id(&session.session_id),
            quote_id: lease_id,
            payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment"),
            account_hash: account_hash(account),
            relay_id: session.relay_id,
            ingress_bytes: u64::from(ordinal),
            egress_bytes: u64::from(ordinal),
            cover_bytes: 0,
            uptime_secs: 1,
            started_at_ms: session.connected_at_ms,
            ended_at_ms: settled_at_ms,
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0x44; 32],
            earned_fee: Quantity::from(1_u64),
            highest_voucher_sequence: u64::from(ordinal),
            client_voucher_hash: [0x55; 32],
        };
        lease_record_from_session_record(&session, VpnLeaseStatusV1::Settled, Some(relay_receipt))
    }

    #[test]
    fn persisted_session_rejects_trust_expiring_before_lease() {
        let account = checked_vpn_account(0x5E);
        let session = sample_session_record(&account);
        let mut lease = lease_record_from_session_record(&session, VpnLeaseStatusV1::Active, None);
        lease.quote_policy.relay_trust_valid_until_ms = lease.expires_at_ms - 1;

        let error = session_record_from_lease(&lease)
            .expect_err("persisted lease must remain bounded by authenticated trust");
        assert!(format!("{error:?}").contains("complete persisted lease"));
    }

    #[test]
    fn persisted_session_requires_current_authenticated_trust() {
        let account = checked_vpn_account(0x60);
        let session = sample_session_record(&account);
        let lease = lease_record_from_session_record(&session, VpnLeaseStatusV1::Active, None);
        let trust = test_vpn_relay_trust();

        ensure_lease_matches_authenticated_trust(&lease, &trust)
            .expect("exact authenticated trust must reconstruct the session");
        let mut wrong_trust = trust;
        wrong_trust.directory_snapshot_digest[0] ^= 1;
        let error = ensure_lease_matches_authenticated_trust(&lease, &wrong_trust)
            .expect_err("different authenticated snapshot must not reconstruct the session");
        assert!(format!("{error:?}").contains("authenticated relay trust"));
    }

    struct ReceiptFixture {
        body: Vec<u8>,
        relay_receipt: VpnSessionReceiptV1,
        voucher: VpnUsageVoucherV1,
        earned_fee: Quantity,
        quote_id: [u8; 32],
    }

    fn receipt_submit_body(
        relay_receipt: &VpnSessionReceiptV1,
        voucher: &VpnUsageVoucherV1,
    ) -> Vec<u8> {
        receipt_submit_body_with_lease_id(relay_receipt, voucher, String::new())
    }

    fn receipt_submit_body_with_lease_id(
        relay_receipt: &VpnSessionReceiptV1,
        voucher: &VpnUsageVoucherV1,
        lease_id_hex: String,
    ) -> Vec<u8> {
        norito::json::to_vec(&VpnReceiptSubmitRequestDto {
            relay_receipt_hex: hex::encode(relay_receipt.encode()),
            client_voucher_hex: hex::encode(voucher.encode()),
            lease_id_hex,
        })
        .expect("receipt request")
    }

    fn receipt_fixture_for_session(
        session: &VpnSessionResponseDto,
        record: &VpnSessionRecord,
        account: &AccountId,
        metering_keys: &KeyPair,
    ) -> ReceiptFixture {
        let relay_session_id = relay_session_id_from_session_id(&session.session_id);
        let quote_id = decode_hex_32(&session.quote_id, "quote").expect("quote id");
        assert_eq!(session.relay_id_hex, hex::encode(record.relay_id));
        let relay_id = record.relay_id;
        let voucher_body = VpnUsageVoucherBodyV1 {
            session_id: relay_session_id,
            quote_id,
            relay_id,
            sequence: 3,
            ingress_bytes: 1_024,
            egress_bytes: 2_048,
            active_ms: 10_000,
            issued_at_ms: now_ms(),
        };
        let voucher = VpnUsageVoucherV1::try_sign(voucher_body, metering_keys.private_key())
            .expect("checked usage voucher fixture");
        let earned_fee = session_earned_fee(record, &voucher).expect("fixture tariff arithmetic");
        let receipt = VpnSessionReceiptV1 {
            session_id: relay_session_id,
            quote_id,
            payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment"),
            account_hash: account_hash(account),
            relay_id,
            ingress_bytes: voucher.body.ingress_bytes,
            egress_bytes: voucher.body.egress_bytes,
            cover_bytes: 128,
            uptime_secs: 10,
            started_at_ms: session.connected_at_ms,
            ended_at_ms: now_ms(),
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0x44; 32],
            earned_fee: earned_fee.clone(),
            highest_voucher_sequence: voucher.body.sequence,
            client_voucher_hash: voucher.hash(),
        };
        let body = receipt_submit_body(&receipt, &voucher);
        ReceiptFixture {
            body,
            relay_receipt: receipt,
            voucher,
            earned_fee,
            quote_id,
        }
    }

    async fn active_wsv_receipt_fixture() -> (
        SharedAppState,
        AccountId,
        KeyPair,
        AccountId,
        KeyPair,
        KeyPair,
        ReceiptFixture,
    ) {
        let user_keys = checked_vpn_ed25519_keypair(0x55);
        let operator_keys = checked_vpn_ed25519_keypair(0x56);
        let user = account_id_for(&user_keys);
        let operator = account_id_for(&operator_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[user.clone(), operator.clone()]),
            &operator,
        );
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &active_record,
                VpnLeaseStatusV1::Active,
                None,
            ));
        app.vpn_sessions.clear();

        (
            app,
            user,
            user_keys,
            operator,
            operator_keys,
            metering_keys,
            fixture,
        )
    }

    async fn submit_receipt_expect_error(
        app: SharedAppState,
        operator: &AccountId,
        operator_keys: &KeyPair,
        relay_receipt: &VpnSessionReceiptV1,
        voucher: &VpnUsageVoucherV1,
        expected: &str,
    ) {
        let body = receipt_submit_body(relay_receipt, voucher);
        submit_receipt_body_expect_error(app, operator, operator_keys, body, expected).await;
    }

    async fn submit_receipt_body_expect_error(
        app: SharedAppState,
        operator: &AccountId,
        operator_keys: &KeyPair,
        body: Vec<u8>,
        expected: &str,
    ) {
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(operator, operator_keys, &method, &uri, body.as_ref());

        let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("adversarial receipt must fail");

        assert!(
            format!("{error:?}").contains(expected),
            "expected `{expected}` in {error:?}"
        );
    }

    async fn read_json<T>(response: axum::response::Response) -> T
    where
        T: norito::json::JsonDeserializeOwned,
    {
        let status = response.status();
        assert!(status.is_success() || status == StatusCode::NOT_FOUND);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response bytes");
        if status == StatusCode::NOT_FOUND {
            panic!("expected JSON response, got 404");
        }
        norito::json::from_slice(bytes.as_ref()).expect("json body")
    }

    #[tokio::test]
    async fn vpn_profile_uses_config_summary() {
        let account = checked_vpn_account(0x57);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);

        let response = handle_get_vpn_profile(app)
            .await
            .expect("profile")
            .into_response();
        let body: VpnProfileResponseDto = read_json(response).await;

        assert_eq!(body.default_exit_class, "standard");
        assert!(body.available);
        assert_eq!(
            body.supported_exit_classes,
            vec!["standard", "low-latency", "high-security"]
        );
        assert!(!body.relay_endpoint.trim().is_empty());
        assert_eq!(body.mtu_bytes, u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES));
        assert_eq!(body.tunnel_addresses, default_tunnel_addresses());
        assert_eq!(body.fee_asset_id, "xor#universal.universal");
        assert_eq!(body.route_pushes, vec!["0.0.0.0/0", "::/0"]);
        assert_eq!(body.dns_servers, vec!["1.1.1.1"]);
        assert_eq!(
            body.relay_id_hex,
            hex::encode(test_vpn_relay_trust().relay_id)
        );
        assert_eq!(body.descriptor_commit_hex, "cd".repeat(32));
        assert_eq!(body.tls_server_name, "relay.example");
        assert_eq!(body.relay_tls_spki_sha256_hex, "ab".repeat(32));
        assert_eq!(body.relay_certificate_sha256_hex, "ef".repeat(32));
        assert_eq!(body.directory_snapshot_digest_hex, "42".repeat(32));
    }

    #[tokio::test]
    async fn vpn_profile_hides_trust_that_cannot_cover_a_lease() {
        let account = checked_vpn_account(0x5F);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let mut inner = Arc::try_unwrap(app)
            .unwrap_or_else(|_| panic!("test app should be uniquely owned before trust update"));
        let mut trust = test_vpn_relay_trust();
        trust.valid_until_ms = now_ms();
        inner.vpn_relay_trust = Some(Arc::new(trust));
        let app = Arc::new(inner);

        let response = handle_get_vpn_profile(app)
            .await
            .expect("profile")
            .into_response();
        let body: VpnProfileResponseDto = read_json(response).await;

        assert!(!body.available);
        assert!(body.relay_endpoint.is_empty());
        assert!(body.relay_id_hex.is_empty());
        assert!(body.descriptor_commit_hex.is_empty());
        assert!(body.tls_server_name.is_empty());
        assert!(body.relay_tls_spki_sha256_hex.is_empty());
        assert!(body.relay_certificate_sha256_hex.is_empty());
        assert!(body.directory_snapshot_digest_hex.is_empty());
    }

    #[tokio::test]
    async fn create_vpn_quote_requires_trust_for_complete_lease() {
        let client_keys = checked_vpn_ed25519_keypair(0x5B);
        let client = account_id_for(&client_keys);
        let operator = checked_vpn_account(0x5C);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[client.clone(), operator.clone()]),
            &operator,
        );
        let mut inner = Arc::try_unwrap(app)
            .unwrap_or_else(|_| panic!("test app should be uniquely owned before trust update"));
        let mut trust = test_vpn_relay_trust();
        trust.valid_until_ms = now_ms().saturating_add(1);
        inner.vpn_relay_trust = Some(Arc::new(trust));
        let app = Arc::new(inner);

        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: "standard".to_owned(),
            metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x5D)),
        })
        .expect("quote body");
        let headers = signed_app_headers(&client, &client_keys, &method, &uri, body.as_ref());
        let error = match handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref()).await
        {
            Ok(_) => panic!("lease extending beyond authenticated trust must fail"),
            Err(error) => format!("{error:?}"),
        };

        assert!(error.contains("complete VPN lease"));
    }

    #[tokio::test]
    async fn create_vpn_quote_rejects_operator_owned_escrow() {
        let key_pair = checked_vpn_ed25519_keypair(0x58);
        let account = account_id_for(&key_pair);
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let mut cfg = crate::test_utils::mk_minimal_root_cfg();
        cfg.network.soranet_vpn.enabled = true;
        cfg.network.soranet_vpn.escrow_account_id = account.clone();
        cfg.network.soranet_vpn.operator_account_id = account.clone();
        let mut inner = match Arc::try_unwrap(app) {
            Ok(inner) => inner,
            Err(_) => panic!("test app should be uniquely owned"),
        };
        inner.kiso = KisoHandle::mock(&cfg);
        inner.vpn_helper_ticket_secret = Some([0x5A; 32]);
        inner.vpn_relay_trust = Some(Arc::new(test_vpn_relay_trust()));
        let app = Arc::new(inner);

        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: "standard".to_owned(),
            metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x59)),
        })
        .expect("quote body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());
        let error = match handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref()).await
        {
            Ok(_) => panic!("quote should reject operator-owned escrow"),
            Err(error) => format!("{error:?}"),
        };
        assert!(error.contains("escrow account"));
    }

    #[test]
    fn helper_ticket_uses_versioned_ticket_when_secret_is_present() {
        let secret = [0x5A; 32];
        let account = checked_vpn_account(0x5A);
        let record = sample_session_record(&account);
        let expires_at_ms = 50_000;
        let encoded =
            build_helper_ticket_hex(&record, expires_at_ms, Some(&secret)).expect("ticket");
        let parsed =
            VpnHelperTicketV1::parse_hex(&encoded, &secret, 1).expect("ticket should parse");
        assert_eq!(
            relay_session_id_from_session_id(&record.session_id),
            parsed.session_id
        );
        assert_eq!(
            decode_hex_32(&record.quote_id, "quote").unwrap(),
            parsed.quote_id
        );
        assert_eq!(
            decode_hex_32(&record.payment_tx_hash, "payment").unwrap(),
            parsed.payment_tx_hash
        );
        assert_eq!(account_hash(&record.account_id), parsed.account_hash);
        assert_eq!(record.relay_id, parsed.relay_id);
        assert_eq!(record.metering_public_key, parsed.metering_public_key);
        assert_eq!(record.tariff, parsed.tariff);
        assert_eq!(expires_at_ms, parsed.expires_at_ms);
    }

    #[test]
    fn settlement_lease_id_canonicalizes_explicit_prefixed_hex() {
        let request = VpnReceiptSubmitRequestDto {
            relay_receipt_hex: String::new(),
            client_voucher_hex: String::new(),
            lease_id_hex: format!("0X{}", "AB".repeat(32)),
        };
        let relay_receipt = VpnSessionReceiptV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            payment_tx_hash: [0x44; 32],
            account_hash: [0x55; 32],
            relay_id: [0x33; 32],
            ingress_bytes: 0,
            egress_bytes: 0,
            cover_bytes: 0,
            uptime_secs: 0,
            started_at_ms: 0,
            ended_at_ms: 0,
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0x66; 32],
            earned_fee: Quantity::zero(),
            highest_voucher_sequence: 0,
            client_voucher_hash: [0u8; 32],
        };

        let (lease_id, normalized_hex) =
            settlement_lease_id_from_request_or_receipt(&request, &relay_receipt)
                .expect("explicit lease id");

        assert_eq!(lease_id, [0xAB; 32]);
        assert_eq!(normalized_hex, "ab".repeat(32));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_canonicalizes_explicit_lease_id() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let canonical_lease_id = hex::encode(fixture.quote_id);
        let submitted_lease_id = format!("0X{}", canonical_lease_id.to_uppercase());
        let body = receipt_submit_body_with_lease_id(
            &fixture.relay_receipt,
            &fixture.voucher,
            submitted_lease_id.clone(),
        );
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());

        let response =
            handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
                .await
                .expect("uppercase explicit lease id should be accepted")
                .into_response();
        let receipt: VpnReceiptResponseDto = read_json(response).await;

        assert_eq!(receipt.lease_id_hex, canonical_lease_id);
        assert_ne!(receipt.lease_id_hex, submitted_lease_id);
        let stored = app
            .vpn_receipts
            .get(&user.to_string())
            .expect("stored receipt");
        assert_eq!(stored[0].lease_id_hex, canonical_lease_id);
    }

    #[tokio::test]
    async fn create_vpn_session_canonicalizes_payment_hash() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x8A);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let submitted_payment_hash = format!("0X{}", quote.quote_id.to_uppercase());
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: quote.exit_class.clone(),
            quote_id: quote.quote_id.clone(),
            payment_tx_hash: submitted_payment_hash.clone(),
            metering_public_key_hex: metering_public_key_hex(&metering_keys),
        })
        .expect("session body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let response =
            handle_create_vpn_session(app.clone(), &method, &uri, &headers, body.as_ref())
                .await
                .expect("uppercase payment hash should be accepted")
                .into_response();
        let session: VpnSessionResponseDto = read_json(response).await;

        assert_eq!(session.payment_tx_hash, quote.quote_id);
        assert_ne!(session.payment_tx_hash, submitted_payment_hash);
        let stored = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("stored session");
        assert_eq!(stored.payment_tx_hash, quote.quote_id);
        drop(stored);
        assert!(app.vpn_used_payments.contains_key(&quote.quote_id));
        assert!(!app.vpn_used_payments.contains_key(&submitted_payment_hash));
    }

    #[tokio::test]
    async fn create_vpn_session_requires_signed_headers() {
        let account = checked_vpn_account(0x5B);
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "standard".to_owned(),
            quote_id: String::new(),
            payment_tx_hash: String::new(),
            metering_public_key_hex: String::new(),
        })
        .expect("body");

        let error =
            match handle_create_vpn_session(app, &method, &uri, &HeaderMap::new(), body.as_ref())
                .await
            {
                Ok(_) => panic!("missing auth should fail"),
                Err(error) => error,
            };

        assert!(format!("{error:?}").contains("signed account headers are required"));
    }

    #[tokio::test]
    async fn vpn_request_handlers_reject_unknown_json_fields_after_auth() {
        fn assert_unknown_field(error: Error, payload_label: &str) {
            let message = format!("{error:?}");
            assert!(
                message.contains(payload_label),
                "expected {payload_label} context, got {message}"
            );
            assert!(
                message.contains("unknown field") && message.contains("unexpected"),
                "expected the unexpected field to be rejected, got {message}"
            );
        }

        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x5C);
        let account = account_id_for(&key_pair);
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;

        let quote_uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let quote_body = br#"{"metering_public_key_hex":"","unexpected":true}"#;
        let quote_headers =
            signed_app_headers(&account, &key_pair, &method, &quote_uri, quote_body);
        let quote_error =
            handle_create_vpn_quote(app.clone(), &method, &quote_uri, &quote_headers, quote_body)
                .await
                .expect_err("unknown quote field must fail after auth");
        assert_unknown_field(quote_error, "invalid vpn quote create payload");

        let session_uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let session_body =
            br#"{"quote_id":"","payment_tx_hash":"","metering_public_key_hex":"","unexpected":true}"#;
        let session_headers =
            signed_app_headers(&account, &key_pair, &method, &session_uri, session_body);
        let session_error = handle_create_vpn_session(
            app.clone(),
            &method,
            &session_uri,
            &session_headers,
            session_body,
        )
        .await
        .expect_err("unknown session field must fail after auth");
        assert_unknown_field(session_error, "invalid vpn session create payload");

        let receipt_uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let receipt_body = br#"{"relay_receipt_hex":"","client_voucher_hex":"","unexpected":true}"#;
        let receipt_headers =
            signed_app_headers(&account, &key_pair, &method, &receipt_uri, receipt_body);
        let receipt_error =
            handle_submit_vpn_receipt(app, &method, &receipt_uri, &receipt_headers, receipt_body)
                .await
                .expect_err("unknown receipt field must fail after auth");
        assert_unknown_field(receipt_error, "invalid vpn receipt payload");
    }

    #[tokio::test]
    async fn vpn_write_handlers_authenticate_before_parsing_malformed_json() {
        let account = checked_vpn_account(0x5C);
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;
        let headers = HeaderMap::new();
        let body = b"{not valid json";

        let quote_uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let quote_error = handle_create_vpn_quote(app.clone(), &method, &quote_uri, &headers, body)
            .await
            .expect_err("missing quote authentication must win over malformed JSON");

        let session_uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let session_error =
            handle_create_vpn_session(app.clone(), &method, &session_uri, &headers, body)
                .await
                .expect_err("missing session authentication must win over malformed JSON");

        let receipt_uri: Uri = "/v1/vpn/receipts".parse().expect("receipt uri");
        let receipt_error = handle_submit_vpn_receipt(app, &method, &receipt_uri, &headers, body)
            .await
            .expect_err("missing receipt authentication must win over malformed JSON");

        for error in [quote_error, session_error, receipt_error] {
            assert!(
                format!("{error:?}").contains("signed account headers are required"),
                "authentication must precede JSON parsing: {error:?}"
            );
        }
    }

    #[tokio::test]
    async fn create_vpn_quote_rejects_non_hex_metering_key() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x5D);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: "standard".to_owned(),
            metering_public_key_hex: "not-hex".to_owned(),
        })
        .expect("quote body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let error = handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("bad metering key must fail");

        assert!(format!("{error:?}").contains("metering_public_key_hex"));
    }

    #[tokio::test]
    async fn create_vpn_session_rejects_quote_owned_by_different_account() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let user_keys = checked_vpn_ed25519_keypair(0x5F);
        let other_keys = checked_vpn_ed25519_keypair(0x60);
        let user = account_id_for(&user_keys);
        let other = account_id_for(&other_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[user.clone(), other.clone()]),
            &user,
        );
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: quote.exit_class.clone(),
            quote_id: quote.quote_id.clone(),
            payment_tx_hash: quote.quote_id.clone(),
            metering_public_key_hex: metering_public_key_hex(&metering_keys),
        })
        .expect("session body");
        let headers = signed_app_headers(&other, &other_keys, &method, &uri, body.as_ref());

        let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("wrong account must not consume quote");

        assert!(format!("{error:?}").contains("different account"));
    }

    #[tokio::test]
    async fn create_vpn_session_rejects_exit_class_mismatch() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x61);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "low-latency").await;
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "standard".to_owned(),
            quote_id: quote.quote_id.clone(),
            payment_tx_hash: quote.quote_id.clone(),
            metering_public_key_hex: metering_public_key_hex(&metering_keys),
        })
        .expect("session body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("exit class mismatch must fail");

        assert!(format!("{error:?}").contains("exit class does not match"));
    }

    #[tokio::test]
    async fn create_vpn_session_rejects_metering_key_mismatch() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x62);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, _metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let wrong_metering_keys = checked_vpn_ed25519_keypair(0x63);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: quote.exit_class.clone(),
            quote_id: quote.quote_id.clone(),
            payment_tx_hash: quote.quote_id.clone(),
            metering_public_key_hex: metering_public_key_hex(&wrong_metering_keys),
        })
        .expect("session body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("metering key mismatch must fail");

        assert!(format!("{error:?}").contains("metering key does not match"));
    }

    #[tokio::test]
    async fn create_vpn_session_rejects_empty_payment_hash() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x64);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("session uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: quote.exit_class.clone(),
            quote_id: quote.quote_id.clone(),
            payment_tx_hash: String::new(),
            metering_public_key_hex: metering_public_key_hex(&metering_keys),
        })
        .expect("session body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let error = handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("empty payment hash must fail");

        assert!(format!("{error:?}").contains("payment_tx_hash must not be empty"));
    }

    #[tokio::test]
    async fn create_get_delete_and_list_vpn_session_roundtrip_for_signed_account() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x65);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "low-latency").await;
        assert_eq!(quote.lease_id_hex, quote.quote_id);
        assert_eq!(quote.session_id_hex.len(), 32);
        assert_eq!(quote.tx_instructions.len(), 1);
        assert_eq!(
            quote.open_lease_instruction.as_ref(),
            Some(&quote.tx_instructions[0])
        );
        let open_payload = hex::decode(&quote.tx_instructions[0].payload_hex).expect("open hex");
        let decoded_open = iroha_data_model::isi::decode_instruction_from_pair(
            &quote.tx_instructions[0].wire_id,
            &open_payload,
        )
        .expect("decode open vpn lease instruction");
        let open = decoded_open
            .as_any()
            .downcast_ref::<OpenVpnLeaseEscrow>()
            .expect("open vpn lease instruction");
        let expected_xor = parse_fee_asset_definition(
            iroha_config::parameters::defaults::soranet::vpn::fee_asset_id().as_str(),
        )
        .expect("canonical XOR asset id");
        assert_eq!(open.asset_definition, expected_xor);
        let quote_record = app.vpn_quotes.get(&quote.quote_id).expect("stored quote");
        assert!(open_lease_matches_quote(open, quote_record.value()).expect("open lease shape"));
        drop(quote_record);
        let session =
            create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys)
                .await;
        assert_eq!(session.account_id, account.to_string());
        assert_eq!(session.exit_class, "low-latency");
        assert_eq!(session.status, "active");
        assert_eq!(session.quote_id, quote.quote_id);
        assert_eq!(session.payment_tx_hash, quote.quote_id);
        assert!(!session.helper_ticket_hex.is_empty());
        assert_eq!(session.tunnel_addresses.len(), 2);
        assert_eq!(app.vpn_sessions.len(), 1);

        let get_method = Method::GET;
        let get_uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("get uri");
        let get_headers = signed_app_headers(&account, &key_pair, &get_method, &get_uri, &[]);
        let active = handle_get_vpn_session(
            app.clone(),
            &get_method,
            &get_uri,
            &get_headers,
            &session.session_id,
        )
        .await
        .expect("active")
        .into_response();
        let active_body: VpnSessionResponseDto = read_json(active).await;
        assert_eq!(active_body.session_id, session.session_id);
        assert_eq!(active_body.connected_at_ms, session.connected_at_ms);

        let delete_method = Method::DELETE;
        let delete_uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("delete uri");
        let delete_headers =
            signed_app_headers(&account, &key_pair, &delete_method, &delete_uri, &[]);
        let deleted = handle_delete_vpn_session(
            app.clone(),
            &delete_method,
            &delete_uri,
            &delete_headers,
            &session.session_id,
        )
        .await
        .expect("deleted")
        .into_response();
        let deleted_body: VpnReceiptResponseDto = read_json(deleted).await;
        assert_eq!(deleted_body.status, "disconnected");
        assert_eq!(deleted_body.receipt_source, "torii");
        assert_eq!(app.vpn_sessions.len(), 0);

        let receipts_method = Method::GET;
        let receipts_uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let receipts_headers =
            signed_app_headers(&account, &key_pair, &receipts_method, &receipts_uri, &[]);
        let receipts = handle_list_vpn_receipts(
            app.clone(),
            &receipts_method,
            &receipts_uri,
            &receipts_headers,
        )
        .await
        .expect("receipts")
        .into_response();
        let receipts_body: VpnReceiptListResponseDto = read_json(receipts).await;
        assert_eq!(receipts_body.total, 1);
        assert_eq!(receipts_body.items[0].session_id, session.session_id);
    }

    #[tokio::test]
    async fn get_vpn_session_reconstructs_active_record_from_wsv_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x66);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys)
                .await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &active_record,
                VpnLeaseStatusV1::Active,
                None,
            ));
        app.vpn_sessions.clear();

        let method = Method::GET;
        let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("get uri");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
        let response = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
            .await
            .expect("wsv session")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body: VpnSessionResponseDto = read_json(response).await;

        assert_eq!(body.session_id, session.session_id);
        assert_eq!(body.account_id, account.to_string());
        assert_eq!(body.payment_tx_hash, session.payment_tx_hash);
        assert_eq!(body.helper_ticket_hex, session.helper_ticket_hex);
    }

    #[tokio::test]
    async fn get_vpn_session_does_not_reconstruct_expired_wsv_lease() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x67);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys)
                .await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        let mut lease_record =
            lease_record_from_session_record(&active_record, VpnLeaseStatusV1::Active, None);
        lease_record.expires_at_ms = now_ms().saturating_sub(1);
        app.state.insert_vpn_lease_for_testing(lease_record);
        app.vpn_sessions.clear();

        let method = Method::GET;
        let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("get uri");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
        let response = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
            .await
            .expect("expired wsv session")
            .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_vpn_session_does_not_reconstruct_non_active_wsv_lease() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x68);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &account, &key_pair, &quote, &metering_keys)
                .await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &active_record,
                VpnLeaseStatusV1::Settled,
                None,
            ));
        app.vpn_sessions.clear();

        let method = Method::GET;
        let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("get uri");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
        let response = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
            .await
            .expect("settled wsv session")
            .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn get_vpn_session_rejects_wrong_account_after_wsv_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let owner_keys = checked_vpn_ed25519_keypair(0x69);
        let intruder_keys = checked_vpn_ed25519_keypair(0x6A);
        let owner = account_id_for(&owner_keys);
        let intruder = account_id_for(&intruder_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[owner.clone(), intruder.clone()]),
            &owner,
        );
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &owner, &owner_keys, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &owner, &owner_keys, &quote, &metering_keys)
                .await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &active_record,
                VpnLeaseStatusV1::Active,
                None,
            ));
        app.vpn_sessions.clear();

        let method = Method::GET;
        let uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("get uri");
        let headers = signed_app_headers(&intruder, &intruder_keys, &method, &uri, &[]);
        let error = handle_get_vpn_session(app, &method, &uri, &headers, &session.session_id)
            .await
            .expect_err("wrong account must fail");

        assert!(format!("{error:?}").contains("different account"));
    }

    #[tokio::test]
    async fn vpn_quote_create_rejects_replayed_nonce() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x6B);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: "standard".to_owned(),
            metering_public_key_hex: metering_public_key_hex(&checked_vpn_ed25519_keypair(0x6C)),
        })
        .expect("body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let first = handle_create_vpn_quote(app.clone(), &method, &uri, &headers, body.as_ref())
            .await
            .expect("first")
            .into_response();
        assert_eq!(first.status(), StatusCode::CREATED);

        let error = match handle_create_vpn_quote(app, &method, &uri, &headers, body.as_ref()).await
        {
            Ok(_) => panic!("replayed request should fail"),
            Err(error) => error,
        };
        assert!(format!("{error:?}").contains("nonce already used"));
    }

    #[tokio::test]
    async fn delete_vpn_session_rejects_different_account() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let owner_keys = checked_vpn_ed25519_keypair(0x6D);
        let intruder_keys = checked_vpn_ed25519_keypair(0x6E);
        let owner = account_id_for(&owner_keys);
        let intruder = account_id_for(&intruder_keys);
        let world = world_with_accounts(&[owner.clone(), intruder.clone()]);
        let app = vpn_enabled_app_with_operator(world, &owner);
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &owner, &owner_keys, "high-security").await;
        let session =
            create_session_for_quote(app.clone(), &owner, &owner_keys, &quote, &metering_keys)
                .await;

        let delete_method = Method::DELETE;
        let delete_uri: Uri = format!("/v1/vpn/sessions/{}", session.session_id)
            .parse()
            .expect("delete uri");
        let delete_headers =
            signed_app_headers(&intruder, &intruder_keys, &delete_method, &delete_uri, &[]);

        let error = match handle_delete_vpn_session(
            app,
            &delete_method,
            &delete_uri,
            &delete_headers,
            &session.session_id,
        )
        .await
        {
            Ok(_) => panic!("wrong account should fail"),
            Err(error) => error,
        };
        assert!(format!("{error:?}").contains("different account"));
    }

    #[tokio::test]
    async fn recreating_session_moves_previous_session_into_receipts() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x6F);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);

        let (first_quote, first_metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "standard").await;
        let _ = create_session_for_quote(
            app.clone(),
            &account,
            &key_pair,
            &first_quote,
            &first_metering_keys,
        )
        .await;

        let (second_quote, second_metering_keys) =
            create_quote_for_account(app.clone(), &account, &key_pair, "low-latency").await;
        let _ = create_session_for_quote(
            app.clone(),
            &account,
            &key_pair,
            &second_quote,
            &second_metering_keys,
        )
        .await;

        let receipts_method = Method::GET;
        let receipts_uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let receipts_headers =
            signed_app_headers(&account, &key_pair, &receipts_method, &receipts_uri, &[]);
        let receipts = handle_list_vpn_receipts(
            app.clone(),
            &receipts_method,
            &receipts_uri,
            &receipts_headers,
        )
        .await
        .expect("receipts")
        .into_response();
        let receipts_body: VpnReceiptListResponseDto = read_json(receipts).await;
        assert_eq!(receipts_body.total, 1);
        assert_eq!(receipts_body.items[0].status, "replaced");
        assert_eq!(app.vpn_sessions.len(), 1);
    }

    #[tokio::test]
    async fn list_vpn_receipts_reconstructs_settled_records_from_wsv() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = checked_vpn_ed25519_keypair(0x70);
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let record = sample_session_record(&account);
        let relay_receipt = VpnSessionReceiptV1 {
            session_id: relay_session_id_from_session_id(&record.session_id),
            quote_id: decode_hex_32(&record.quote_id, "quote").expect("quote"),
            payment_tx_hash: decode_hex_32(&record.payment_tx_hash, "payment").expect("payment"),
            account_hash: account_hash(&account),
            relay_id: record.relay_id,
            ingress_bytes: 128,
            egress_bytes: 256,
            cover_bytes: 0,
            uptime_secs: 10,
            started_at_ms: record.connected_at_ms,
            ended_at_ms: record.connected_at_ms + 10_000,
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0x44; 32],
            earned_fee: Quantity::from(100_u64),
            highest_voucher_sequence: 7,
            client_voucher_hash: [0x55; 32],
        };
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &record,
                VpnLeaseStatusV1::Settled,
                Some(relay_receipt),
            ));

        let method = Method::GET;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, &[]);
        let response = handle_list_vpn_receipts(app, &method, &uri, &headers)
            .await
            .expect("receipts")
            .into_response();
        let body: VpnReceiptListResponseDto = read_json(response).await;

        assert_eq!(body.total, 1);
        assert_eq!(body.items[0].receipt_source, "wsv");
        assert_eq!(body.items[0].status, "settled");
        assert_eq!(body.items[0].earned_fee, Quantity::from(100_u64));
    }

    #[test]
    fn list_vpn_receipts_uses_bounded_account_projection() {
        let account = checked_vpn_account(0x74);
        let unrelated_account = checked_vpn_account(0x75);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);

        for ordinal in 1..=30_u16 {
            app.state
                .insert_vpn_lease_for_testing(settled_lease_for_account(&account, ordinal));
        }
        for ordinal in 100..200_u16 {
            app.state
                .insert_vpn_lease_for_testing(settled_lease_for_account(
                    &unrelated_account,
                    ordinal,
                ));
        }

        let world = app.state.world_view();
        assert_eq!(
            world
                .vpn_settled_leases_by_account()
                .get(&account)
                .map(BTreeSet::len),
            Some(MAX_RECEIPTS_PER_ACCOUNT)
        );
        assert_eq!(
            world
                .vpn_settled_leases_by_account()
                .get(&unrelated_account)
                .map(BTreeSet::len),
            Some(MAX_RECEIPTS_PER_ACCOUNT)
        );
        drop(world);

        let receipts = list_receipts_for_account(&app, &account).expect("bounded receipt page");
        assert_eq!(receipts.len(), MAX_RECEIPTS_PER_ACCOUNT);
        assert_eq!(receipts[0].disconnected_at_ms, 10_030);
        assert_eq!(
            receipts[MAX_RECEIPTS_PER_ACCOUNT - 1].disconnected_at_ms,
            10_007
        );
        assert!(
            receipts
                .iter()
                .all(|receipt| receipt.account_id == account.to_string())
        );
    }

    #[test]
    fn list_vpn_receipts_fails_closed_on_stale_projection() {
        let account = checked_vpn_account(0x76);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        app.state
            .insert_vpn_settled_lease_index_entry_for_testing(account.clone(), 1, [0xA5; 32]);

        let error = list_receipts_for_account(&app, &account)
            .expect_err("missing indexed lease must fail closed");
        assert!(matches!(
            error,
            Error::AppServiceUnavailable {
                code: "vpn_state_inconsistent",
                ..
            }
        ));
    }

    #[test]
    fn list_vpn_receipts_cannot_reintroduce_a_global_lease_scan() {
        let source = include_str!("vpn.rs");
        let start = source
            .find("fn list_receipts_for_account(")
            .expect("receipt projection function");
        let tail = &source[start..];
        let end = tail
            .find("fn external_signed_transaction_results(")
            .expect("receipt projection terminator");
        let implementation = &tail[..end];

        assert!(implementation.contains("vpn_settled_leases_by_account()"));
        assert!(!implementation.contains("vpn_leases().iter()"));
    }

    #[tokio::test]
    async fn vpn_address_allocator_avoids_collisions_across_active_sessions() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let first_keys = checked_vpn_ed25519_keypair(0x71);
        let second_keys = checked_vpn_ed25519_keypair(0x72);
        let third_keys = checked_vpn_ed25519_keypair(0x73);
        let first_account = account_id_for(&first_keys);
        let second_account = account_id_for(&second_keys);
        let third_account = account_id_for(&third_keys);
        let world = world_with_accounts(&[
            first_account.clone(),
            second_account.clone(),
            third_account.clone(),
        ]);
        let app = vpn_enabled_app_with_operator(world, &first_account);

        let (first_quote, first_metering_keys) =
            create_quote_for_account(app.clone(), &first_account, &first_keys, "standard").await;
        let first_session = create_session_for_quote(
            app.clone(),
            &first_account,
            &first_keys,
            &first_quote,
            &first_metering_keys,
        )
        .await;

        let (second_quote, second_metering_keys) =
            create_quote_for_account(app.clone(), &second_account, &second_keys, "standard").await;
        let second_session = create_session_for_quote(
            app.clone(),
            &second_account,
            &second_keys,
            &second_quote,
            &second_metering_keys,
        )
        .await;

        assert_ne!(
            first_session.tunnel_addresses,
            second_session.tunnel_addresses
        );
        let delete_method = Method::DELETE;
        let delete_uri: Uri = format!("/v1/vpn/sessions/{}", first_session.session_id)
            .parse()
            .expect("delete uri");
        let delete_headers = signed_app_headers(
            &first_account,
            &first_keys,
            &delete_method,
            &delete_uri,
            &[],
        );
        handle_delete_vpn_session(
            app.clone(),
            &delete_method,
            &delete_uri,
            &delete_headers,
            &first_session.session_id,
        )
        .await
        .expect("deleted first session");

        let (third_quote, third_metering_keys) =
            create_quote_for_account(app.clone(), &third_account, &third_keys, "standard").await;
        let third_session = create_session_for_quote(
            app.clone(),
            &third_account,
            &third_keys,
            &third_quote,
            &third_metering_keys,
        )
        .await;

        assert_ne!(
            third_session.tunnel_addresses,
            second_session.tunnel_addresses
        );
        assert_eq!(app.vpn_sessions.len(), 2);
    }

    #[tokio::test]
    async fn submit_vpn_receipt_allows_expired_session_within_wsv_grace() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let user_keys = checked_vpn_ed25519_keypair(0x74);
        let operator_keys = checked_vpn_ed25519_keypair(0x75);
        let user = account_id_for(&user_keys);
        let operator = account_id_for(&operator_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[user.clone(), operator.clone()]),
            &operator,
        );
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
        let mut lease_record =
            lease_record_from_session_record(&active_record, VpnLeaseStatusV1::Active, None);
        lease_record.expires_at_ms = now_ms().saturating_sub(1_000);
        lease_record.settlement_grace_ms = 60_000;
        app.state.insert_vpn_lease_for_testing(lease_record);
        app.vpn_sessions.clear();

        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(
            &operator,
            &operator_keys,
            &method,
            &uri,
            fixture.body.as_ref(),
        );
        let response =
            handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
                .await
                .expect("settled within grace")
                .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
        let settled: VpnReceiptResponseDto = read_json(response).await;
        assert_eq!(settled.status, "settled");
        assert_eq!(settled.earned_fee, fixture.earned_fee);
        assert_eq!(settled.lease_id_hex, hex::encode(fixture.quote_id));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_after_wsv_grace() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let user_keys = checked_vpn_ed25519_keypair(0x76);
        let operator_keys = checked_vpn_ed25519_keypair(0x77);
        let user = account_id_for(&user_keys);
        let operator = account_id_for(&operator_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[user.clone(), operator.clone()]),
            &operator,
        );
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
        let mut lease_record =
            lease_record_from_session_record(&active_record, VpnLeaseStatusV1::Active, None);
        lease_record.expires_at_ms = now_ms().saturating_sub(10_000);
        lease_record.settlement_grace_ms = 1;
        app.state.insert_vpn_lease_for_testing(lease_record);
        app.vpn_sessions.clear();

        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(
            &operator,
            &operator_keys,
            &method,
            &uri,
            fixture.body.as_ref(),
        );
        let error =
            handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
                .await
                .expect_err("settlement after grace must fail");

        assert!(format!("{error:?}").contains("grace window expired"));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_non_operator_signature_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, user, user_keys, _operator, _operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&user, &user_keys, &method, &uri, fixture.body.as_ref());

        let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect_err("non-operator receipt signer must fail");

        assert!(format!("{error:?}").contains("configured operator account"));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_exact_signed_request_replay() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(
            &operator,
            &operator_keys,
            &method,
            &uri,
            fixture.body.as_ref(),
        );

        let first =
            handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, fixture.body.as_ref())
                .await
                .expect("first settlement")
                .into_response();
        assert_eq!(first.status(), StatusCode::CREATED);

        let replay = handle_submit_vpn_receipt(app, &method, &uri, &headers, fixture.body.as_ref())
            .await
            .expect_err("exact request replay must fail");

        assert!(format!("{replay:?}").contains("nonce already used"));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_explicit_lease_id_for_different_active_lease() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let user_keys = checked_vpn_ed25519_keypair(0x78);
        let other_user_keys = checked_vpn_ed25519_keypair(0x79);
        let operator_keys = checked_vpn_ed25519_keypair(0x7A);
        let user = account_id_for(&user_keys);
        let other_user = account_id_for(&other_user_keys);
        let operator = account_id_for(&operator_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[user.clone(), other_user.clone(), operator.clone()]),
            &operator,
        );

        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        let fixture = receipt_fixture_for_session(&session, &active_record, &user, &metering_keys);
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &active_record,
                VpnLeaseStatusV1::Active,
                None,
            ));

        let (other_quote, other_metering_keys) =
            create_quote_for_account(app.clone(), &other_user, &other_user_keys, "standard").await;
        let other_session = create_session_for_quote(
            app.clone(),
            &other_user,
            &other_user_keys,
            &other_quote,
            &other_metering_keys,
        )
        .await;
        let other_record = app
            .vpn_sessions
            .get(&other_session.session_id)
            .expect("other active session")
            .clone();
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &other_record,
                VpnLeaseStatusV1::Active,
                None,
            ));
        app.vpn_sessions.clear();

        let body = receipt_submit_body_with_lease_id(
            &fixture.relay_receipt,
            &fixture.voucher,
            other_session.quote_id.clone(),
        );
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());

        let error = handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("explicit mismatched lease id must fail");

        assert!(format!("{error:?}").contains("vpn receipt"));
        assert!(app.vpn_receipts.is_empty());
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_wrong_metering_key_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let wrong_metering_keys = checked_vpn_ed25519_keypair(0x7B);
        let voucher =
            VpnUsageVoucherV1::try_sign(fixture.voucher.body, wrong_metering_keys.private_key())
                .expect("checked wrong-metering-key voucher");
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.client_voucher_hash = voucher.hash();
        let body = receipt_submit_body(&relay_receipt, &voucher);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());

        let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("wrong metering key must fail");

        assert!(format!("{error:?}").contains("public key does not match"));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_relay_earned_fee_inflation_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.earned_fee = fixture
            .earned_fee
            .checked_add(&Quantity::one())
            .expect("tampered earned fee remains representable");
        let body = receipt_submit_body(&relay_receipt, &fixture.voucher);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());

        let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("inflated earned fee must fail");

        assert!(format!("{error:?}").contains("earned fee does not match"));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_voucher_hash_substitution_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut voucher = fixture.voucher.clone();
        voucher.body.sequence = voucher.body.sequence.saturating_add(1);
        voucher = VpnUsageVoucherV1::try_sign(voucher.body, metering_keys.private_key())
            .expect("checked changed voucher");
        let body = receipt_submit_body(&fixture.relay_receipt, &voucher);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());

        let error = handle_submit_vpn_receipt(app, &method, &uri, &headers, body.as_ref())
            .await
            .expect_err("voucher substitution must fail");

        assert!(format!("{error:?}").contains("does not commit"));
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_payment_hash_substitution_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.payment_tx_hash[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "payment hash does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_account_hash_substitution_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.account_hash[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "account hash does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_relay_id_substitution_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.relay_id[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "relay id does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_byte_counter_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.ingress_bytes = relay_receipt.ingress_bytes.saturating_add(1);

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "byte counters do not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_uptime_below_voucher_active_time_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.uptime_secs = 0;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "uptime is below",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_end_before_start_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.started_at_ms = 10_000;
        relay_receipt.ended_at_ms = 9_999;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "end timestamp precedes",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_voucher_signature_tamper_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut voucher = fixture.voucher.clone();
        voucher.body.issued_at_ms = voucher.body.issued_at_ms.saturating_add(1);
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.client_voucher_hash = voucher.hash();

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &voucher,
            "signature failed",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_voucher_sequence_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.highest_voucher_sequence =
            relay_receipt.highest_voucher_sequence.saturating_add(1);

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "voucher sequence does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_receipt_session_id_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.session_id[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "session id does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_voucher_session_id_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut voucher = fixture.voucher.clone();
        voucher.body.session_id[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &fixture.relay_receipt,
            &voucher,
            "session id does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_receipt_quote_id_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.quote_id[0] ^= 0x01;
        let body = receipt_submit_body_with_lease_id(
            &relay_receipt,
            &fixture.voucher,
            hex::encode(fixture.quote_id),
        );

        submit_receipt_body_expect_error(
            app,
            &operator,
            &operator_keys,
            body,
            "quote id does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_voucher_quote_id_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut voucher = fixture.voucher.clone();
        voucher.body.quote_id[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &fixture.relay_receipt,
            &voucher,
            "quote id does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_voucher_relay_id_mismatch_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut voucher = fixture.voucher.clone();
        voucher.body.relay_id[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &fixture.relay_receipt,
            &voucher,
            "relay id does not match",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_malformed_relay_receipt_hex() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
            relay_receipt_hex: "not-hex".to_owned(),
            client_voucher_hex: hex::encode(fixture.voucher.encode()),
            lease_id_hex: String::new(),
        })
        .expect("receipt request");

        submit_receipt_body_expect_error(app, &operator, &operator_keys, body, "relay_receipt_hex")
            .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_malformed_client_voucher_hex() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
            relay_receipt_hex: hex::encode(fixture.relay_receipt.encode()),
            client_voucher_hex: "not-hex".to_owned(),
            lease_id_hex: String::new(),
        })
        .expect("receipt request");

        submit_receipt_body_expect_error(
            app,
            &operator,
            &operator_keys,
            body,
            "client_voucher_hex",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_client_voucher_trailing_norito_bytes() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut encoded_voucher = fixture.voucher.encode();
        encoded_voucher.push(0);
        let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
            relay_receipt_hex: hex::encode(fixture.relay_receipt.encode()),
            client_voucher_hex: hex::encode(encoded_voucher),
            lease_id_hex: String::new(),
        })
        .expect("receipt request");

        submit_receipt_body_expect_error(
            app,
            &operator,
            &operator_keys,
            body,
            "client_voucher_hex is not valid Norito",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_explicit_lease_id_wrong_length() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let body = receipt_submit_body_with_lease_id(
            &fixture.relay_receipt,
            &fixture.voucher,
            "aa".to_owned(),
        );

        submit_receipt_body_expect_error(
            app,
            &operator,
            &operator_keys,
            body,
            "lease_id_hex must decode to 32 bytes",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_explicit_lease_id_non_hex() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let body = receipt_submit_body_with_lease_id(
            &fixture.relay_receipt,
            &fixture.voucher,
            "not-hex".to_owned(),
        );

        submit_receipt_body_expect_error(app, &operator, &operator_keys, body, "lease_id_hex")
            .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_unknown_receipt_lease_id_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut relay_receipt = fixture.relay_receipt;
        relay_receipt.quote_id[0] ^= 0x01;

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &relay_receipt,
            &fixture.voucher,
            "does not match an on-chain native VPN lease",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_settled_lease_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut lease = lease_record_by_id(&app, &fixture.quote_id).expect("active lease");
        lease.status = VpnLeaseStatusV1::Settled;
        lease.settled_at_ms = Some(fixture.relay_receipt.ended_at_ms);
        lease.highest_voucher_sequence = fixture.relay_receipt.highest_voucher_sequence;
        lease.client_voucher_hash = Some(fixture.voucher.hash());
        lease.relay_receipt_hash = Some(fixture.relay_receipt.hash());
        lease.settled_relay_receipt = Some(fixture.relay_receipt.clone());
        lease.earned_fee = fixture.earned_fee.clone();
        lease.refunded_fee = lease
            .lease_fee
            .checked_sub(&fixture.earned_fee)
            .expect("fixture earned fee does not exceed lease fee");
        app.state.insert_vpn_lease_for_testing(lease);

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &fixture.relay_receipt,
            &fixture.voucher,
            "vpn lease is not active",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_rejects_refunded_lease_after_cache_loss() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let (app, _user, _user_keys, operator, operator_keys, _metering_keys, fixture) =
            active_wsv_receipt_fixture().await;
        let mut lease = lease_record_by_id(&app, &fixture.quote_id).expect("active lease");
        lease.status = VpnLeaseStatusV1::Refunded;
        lease.refunded_at_ms = Some(now_ms());
        app.state.insert_vpn_lease_for_testing(lease);

        submit_receipt_expect_error(
            app,
            &operator,
            &operator_keys,
            &fixture.relay_receipt,
            &fixture.voucher,
            "vpn lease is not active",
        )
        .await;
    }

    #[tokio::test]
    async fn submit_vpn_receipt_requires_operator_and_client_voucher() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let user_keys = checked_vpn_ed25519_keypair(0x7D);
        let operator_keys = checked_vpn_ed25519_keypair(0x7E);
        let user = account_id_for(&user_keys);
        let operator = account_id_for(&operator_keys);
        let app = vpn_enabled_app_with_operator(
            world_with_accounts(&[user.clone(), operator.clone()]),
            &operator,
        );
        let (quote, metering_keys) =
            create_quote_for_account(app.clone(), &user, &user_keys, "standard").await;
        let session =
            create_session_for_quote(app.clone(), &user, &user_keys, &quote, &metering_keys).await;
        let active_record = app
            .vpn_sessions
            .get(&session.session_id)
            .expect("active session")
            .clone();
        app.state
            .insert_vpn_lease_for_testing(lease_record_from_session_record(
                &active_record,
                VpnLeaseStatusV1::Active,
                None,
            ));

        let relay_session_id = relay_session_id_from_session_id(&session.session_id);
        let quote_id = decode_hex_32(&session.quote_id, "quote").expect("quote id");
        assert_eq!(session.relay_id_hex, hex::encode(active_record.relay_id));
        let relay_id = active_record.relay_id;
        let voucher_body = VpnUsageVoucherBodyV1 {
            session_id: relay_session_id,
            quote_id,
            relay_id,
            sequence: 3,
            ingress_bytes: 1_024,
            egress_bytes: 2_048,
            active_ms: 10_000,
            issued_at_ms: now_ms(),
        };
        let voucher = VpnUsageVoucherV1::try_sign(voucher_body, metering_keys.private_key())
            .expect("checked usage voucher fixture");
        let earned_fee = {
            let record = app
                .vpn_sessions
                .get(&session.session_id)
                .expect("active session record");
            session_earned_fee(&record, &voucher).expect("fixture tariff arithmetic")
        };
        let receipt = VpnSessionReceiptV1 {
            session_id: relay_session_id,
            quote_id,
            payment_tx_hash: decode_hex_32(&session.payment_tx_hash, "payment").expect("payment"),
            account_hash: account_hash(&user),
            relay_id,
            ingress_bytes: 1_024,
            egress_bytes: 2_048,
            cover_bytes: 128,
            uptime_secs: 10,
            started_at_ms: session.connected_at_ms,
            ended_at_ms: now_ms(),
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0x44; 32],
            earned_fee: earned_fee.clone(),
            highest_voucher_sequence: voucher.body.sequence,
            client_voucher_hash: voucher.hash(),
        };
        let body = norito::json::to_vec(&VpnReceiptSubmitRequestDto {
            relay_receipt_hex: hex::encode(receipt.encode()),
            client_voucher_hex: hex::encode(voucher.encode()),
            lease_id_hex: String::new(),
        })
        .expect("receipt request");
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/receipts".parse().expect("receipts uri");
        let headers = signed_app_headers(&operator, &operator_keys, &method, &uri, body.as_ref());
        app.vpn_sessions.clear();

        let response =
            handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
                .await
                .expect("settled")
                .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let settled: VpnReceiptResponseDto = read_json(response).await;
        assert_eq!(settled.status, "settled");
        assert_eq!(settled.receipt_source, "relay");
        assert_eq!(settled.earned_fee, earned_fee);
        assert_eq!(
            settled.refunded_fee,
            session
                .lease_fee
                .checked_sub(&earned_fee)
                .expect("fixture earned fee does not exceed lease fee")
        );
        assert_eq!(settled.lease_id_hex, session.quote_id);
        assert_eq!(settled.tx_instructions.len(), 1);
        let settle_instruction = settled
            .settle_lease_instruction
            .as_ref()
            .expect("native settle instruction");
        assert_eq!(settled.tx_instructions[0], *settle_instruction);
        let settle_payload = hex::decode(&settle_instruction.payload_hex).expect("payload hex");
        let decoded_settle = iroha_data_model::isi::decode_instruction_from_pair(
            &settle_instruction.wire_id,
            &settle_payload,
        )
        .expect("decode native settle instruction");
        let settle = decoded_settle
            .as_any()
            .downcast_ref::<SettleVpnLease>()
            .expect("settle vpn lease instruction");
        assert_eq!(settle.lease_id, quote_id);
        assert_eq!(settle.relay_receipt, receipt);
        assert_eq!(settle.client_voucher, voucher);
        assert_eq!(app.vpn_sessions.len(), 0);
    }
}
