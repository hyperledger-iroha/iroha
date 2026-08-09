use std::{
    collections::{HashMap, HashSet},
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
    smartcontracts::isi::vpn::vpn_lease_custody_account_id,
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
    permission::Permission,
    query::error::QueryExecutionFail,
    soranet::vpn::{
        VPN_DEFAULT_TUNNEL_MTU_BYTES, VpnAddressSlotV1, VpnExitClassV1, VpnHelperTicketV1,
        VpnLeaseRecordV1, VpnLeaseStatusV1, VpnQuoteBodyV1, VpnQuotePolicyV1, VpnSessionReceiptV1,
        VpnSignedQuoteV1, VpnTariffV1, VpnUsageVoucherV1, derive_vpn_address_plan_v1,
        derive_vpn_address_slot_v1, derive_vpn_lease_id_v1, derive_vpn_session_id_v1,
    },
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_executor_data_model::permission::soranet::CanIssueSoranetVpnQuote;
use iroha_primitives::numeric::{Numeric, Quantity, RoundingMode};
use mv::storage::StorageReadOnly;
use sha2::{Digest as _, Sha256};

use crate::{Error, SharedAppState};

const SUPPORTED_EXIT_CLASSES: [&str; 3] = ["standard", "low-latency", "high-security"];
const DEFAULT_TUNNEL_ADDRESSES: [&str; 2] = ["10.208.0.2/32", "fd53:7261:6574::2/128"];
// Runtime VPN state is deliberately bounded independently of the number of
// registered accounts. At most one quote and one session are retained per
// account, and a full cache fails closed instead of evicting unrelated users.
const VPN_RUNTIME_ACCOUNT_CAPACITY: usize = 4_096;

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
    pub open_lease_instruction: VpnTxInstructionDto,
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
    pub lease_id: [u8; 32],
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
    pub lease_id: [u8; 32],
    pub session_id: [u8; 16],
    pub signed_quote: VpnSignedQuoteV1,
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

/// Reverse indexes, in-flight reservations, and bounds for the compound VPN
/// runtime caches.
///
/// The enclosing `vpn_state_lock` protects these indexes together with every
/// mutation of `vpn_quotes`, `vpn_sessions`, and `vpn_used_payments`. Keeping
/// the indexes behind the same lock makes replacement and exact removal atomic
/// without requiring request-time scans of the record maps.
#[derive(Debug)]
pub(crate) struct VpnRuntimeState {
    quote_ids_by_account: HashMap<String, String>,
    session_ids_by_account: HashMap<String, String>,
    settling_session_ids: HashSet<String>,
    quote_capacity: usize,
    session_capacity: usize,
    #[cfg(test)]
    quote_account_lookups: usize,
    #[cfg(test)]
    session_account_lookups: usize,
}

impl Default for VpnRuntimeState {
    fn default() -> Self {
        Self {
            quote_ids_by_account: HashMap::new(),
            session_ids_by_account: HashMap::new(),
            settling_session_ids: HashSet::new(),
            quote_capacity: VPN_RUNTIME_ACCOUNT_CAPACITY,
            session_capacity: VPN_RUNTIME_ACCOUNT_CAPACITY,
            #[cfg(test)]
            quote_account_lookups: 0,
            #[cfg(test)]
            session_account_lookups: 0,
        }
    }
}

#[cfg(test)]
impl VpnRuntimeState {
    fn with_capacities(quote_capacity: usize, session_capacity: usize) -> Self {
        Self {
            quote_capacity,
            session_capacity,
            ..Self::default()
        }
    }
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

fn relay_session_id_from_session_id(session_id: &str) -> [u8; 16] {
    let normalized = session_id
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    let decoded = hex::decode(normalized).expect("stored VPN session id must be canonical hex");
    decoded
        .try_into()
        .expect("stored VPN session id must encode exactly 16 bytes")
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

fn build_session_id_from_quote(quote: &VpnQuoteRecord) -> String {
    hex::encode(quote.session_id)
}

fn default_lease_id_hex(record: &VpnSessionRecord) -> String {
    hex::encode(record.lease_id)
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
    record.signed_quote.body.policy.clone()
}

fn validate_quote_record_projection(record: &VpnQuoteRecord) -> Result<(), Error> {
    record.signed_quote.verify().map_err(|error| {
        inconsistent_vpn_state(format!("stored VPN quote signature is invalid: {error}"))
    })?;
    let body = &record.signed_quote.body;
    let policy = &body.policy;
    let quote_id = hex::encode(body.quote_id);
    let canonical_lease_id =
        derive_vpn_lease_id_v1(&body.chain_id, body.quote_id, &body.client_account_id);
    let canonical_session_id = derive_vpn_session_id_v1(
        &body.chain_id,
        body.quote_id,
        &body.client_account_id,
        body.address_slot,
    );
    let signed_fee_asset_id = body.asset_definition.to_string();
    let signed_exit_class = policy.exit_class.as_label();
    if body.lease_id != canonical_lease_id || body.session_id != canonical_session_id {
        return Err(inconsistent_vpn_state(
            "stored VPN quote contains non-canonical lease or session identifiers",
        ));
    }

    macro_rules! require_projection {
        ($label:literal, $record_value:expr, $signed_value:expr) => {
            if $record_value != $signed_value {
                return Err(inconsistent_vpn_state(concat!(
                    "stored VPN quote ",
                    $label,
                    " projection differs from the signed quote"
                )));
            }
        };
    }

    require_projection!("quote id", &record.quote_id, &quote_id);
    require_projection!("payment reference", &record.payment_reference, &quote_id);
    require_projection!("lease id", &record.lease_id, &body.lease_id);
    require_projection!("session id", &record.session_id, &body.session_id);
    require_projection!("client", &record.account_id, &body.client_account_id);
    require_projection!(
        "operator",
        &record.operator_account_id,
        &body.operator_account_id
    );
    require_projection!(
        "metering key",
        &record.metering_public_key,
        &body.metering_public_key
    );
    require_projection!("tariff", &record.tariff, &body.tariff);
    require_projection!("lease fee", &record.lease_fee, &body.tariff.lease_fee);
    require_projection!("fee asset", &record.fee_asset_id, &signed_fee_asset_id);
    require_projection!(
        "quote expiry",
        &record.quote_expires_at_ms,
        &body.expires_at_ms
    );
    require_projection!(
        "settlement grace",
        &record.settlement_grace_ms,
        &body.settlement_grace_ms
    );
    if record.exit_class != signed_exit_class {
        return Err(inconsistent_vpn_state(
            "stored VPN quote exit class projection differs from the signed quote",
        ));
    }
    require_projection!(
        "relay endpoint",
        &record.relay_endpoint,
        &policy.relay_endpoint
    );
    require_projection!("lease duration", &record.lease_secs, &policy.lease_secs);
    require_projection!(
        "escrow",
        &record.escrow_account_id,
        &policy.escrow_account_id
    );
    require_projection!("meter family", &record.meter_family, &policy.meter_family);
    require_projection!("routes", &record.route_pushes, &policy.route_pushes);
    require_projection!(
        "excluded routes",
        &record.excluded_routes,
        &policy.excluded_routes
    );
    require_projection!("DNS", &record.dns_servers, &policy.dns_servers);
    require_projection!(
        "tunnel addresses",
        &record.tunnel_addresses,
        &policy.tunnel_addresses
    );
    require_projection!("MTU", &record.mtu_bytes, &policy.mtu_bytes);
    require_projection!(
        "flow label",
        &record.flow_label_bits,
        &policy.flow_label_bits
    );
    require_projection!(
        "padding budget",
        &record.padding_budget_ms,
        &policy.padding_budget_ms
    );
    require_projection!("relay id", &record.relay_id, &policy.relay_id);
    require_projection!(
        "descriptor commitment",
        &record.descriptor_commit,
        &policy.descriptor_commit
    );
    require_projection!(
        "TLS server name",
        &record.tls_server_name,
        &policy.tls_server_name
    );
    require_projection!(
        "TLS SPKI",
        &record.relay_tls_spki_sha256,
        &policy.relay_tls_spki_sha256
    );
    require_projection!(
        "relay certificate",
        &record.relay_certificate_sha256,
        &policy.relay_certificate_sha256
    );
    require_projection!(
        "directory snapshot",
        &record.directory_snapshot_digest,
        &policy.directory_snapshot_digest
    );
    require_projection!(
        "relay trust expiry",
        &record.relay_trust_valid_until_ms,
        &policy.relay_trust_valid_until_ms
    );
    Ok(())
}

fn open_lease_instruction(record: &VpnQuoteRecord) -> Result<VpnTxInstructionDto, Error> {
    validate_quote_record_projection(record)?;
    let instruction: InstructionBox = OpenVpnLeaseEscrow::new(record.signed_quote.clone()).into();
    Ok(tx_instr_from_box(instruction))
}

fn ensure_vpn_quote_operator_authorized(
    app: &SharedAppState,
    operator_account_id: &AccountId,
) -> Result<(), Error> {
    let required_permission: Permission = CanIssueSoranetVpnQuote.into();
    let world = app.state.world_view();
    let direct = world
        .account_permissions_iter(operator_account_id)
        .map_err(|error| {
            not_permitted_error(format!(
                "cannot resolve configured VPN operator authority: {error}"
            ))
        })?
        .any(|permission| permission == &required_permission);
    let through_role = world
        .account_roles_iter(operator_account_id)
        .any(|role_id| {
            world.roles().get(role_id).is_some_and(|role| {
                role.permissions()
                    .any(|permission| permission == &required_permission)
            })
        });
    if direct || through_role {
        Ok(())
    } else {
        Err(not_permitted_error(
            "configured VPN operator does not hold CanIssueSoranetVpnQuote",
        ))
    }
}

fn settlement_lease_id_from_request_or_index(
    request: &VpnReceiptSubmitRequestDto,
    indexed_lease_id: [u8; 32],
) -> Result<([u8; 32], String), Error> {
    let lease_id_hex = request.lease_id_hex.trim();
    if lease_id_hex.is_empty() {
        return Ok((indexed_lease_id, hex::encode(indexed_lease_id)));
    }
    let lease_id = decode_hex_32(lease_id_hex, "lease_id_hex")?;
    if lease_id != indexed_lease_id {
        return Err(not_permitted_error(
            "lease_id_hex does not match the consensus-indexed VPN session",
        ));
    }
    Ok((lease_id, hex::encode(lease_id)))
}

fn quote_response_from_record(record: &VpnQuoteRecord) -> Result<VpnQuoteResponseDto, Error> {
    let open_lease_instruction = open_lease_instruction(record)?;
    Ok(VpnQuoteResponseDto {
        quote_id: record.quote_id.clone(),
        lease_id_hex: hex::encode(record.lease_id),
        session_id_hex: hex::encode(record.session_id),
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
        open_lease_instruction,
    })
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

fn account_runtime_key(account_id: &AccountId) -> String {
    account_id.to_string()
}

fn lock_vpn_runtime(app: &SharedAppState) -> std::sync::MutexGuard<'_, VpnRuntimeState> {
    app.vpn_state_lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn remove_quote_by_id_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    quote_id: &str,
) -> Option<VpnQuoteRecord> {
    let (_, record) = app.vpn_quotes.remove(quote_id)?;
    let account_key = account_runtime_key(&record.account_id);
    if state
        .quote_ids_by_account
        .get(&account_key)
        .is_some_and(|indexed_id| indexed_id == quote_id)
    {
        state.quote_ids_by_account.remove(&account_key);
    }
    Some(record)
}

fn quote_for_account_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    account_id: &AccountId,
) -> Option<VpnQuoteRecord> {
    #[cfg(test)]
    {
        state.quote_account_lookups += 1;
    }
    let account_key = account_runtime_key(account_id);
    let quote_id = state.quote_ids_by_account.get(&account_key)?.clone();
    let record = app.vpn_quotes.get(&quote_id).map(|entry| entry.clone());
    match record {
        Some(record) if &record.account_id == account_id => Some(record),
        _ => {
            // Self-heal only the authenticated account's stale reverse entry.
            state.quote_ids_by_account.remove(&account_key);
            None
        }
    }
}

fn expire_quote_for_account_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    account_id: &AccountId,
    current_ms: u64,
) {
    let Some(record) = quote_for_account_locked(app, state, account_id) else {
        return;
    };
    if record.quote_expires_at_ms <= current_ms {
        let _ = remove_quote_by_id_locked(app, state, &record.quote_id);
    }
}

fn quote_by_id_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    quote_id: &str,
    current_ms: u64,
) -> Option<VpnQuoteRecord> {
    let record = app.vpn_quotes.get(quote_id).map(|entry| entry.clone())?;
    if record.quote_expires_at_ms <= current_ms {
        let _ = remove_quote_by_id_locked(app, state, quote_id);
        return None;
    }
    Some(record)
}

fn insert_quote_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    record: VpnQuoteRecord,
) -> Result<(), Error> {
    validate_quote_record_projection(&record)?;
    let account_key = account_runtime_key(&record.account_id);
    let existing = quote_for_account_locked(app, state, &record.account_id);
    if existing.is_none() && state.quote_ids_by_account.len() >= state.quote_capacity {
        return Err(not_permitted_error(
            "vpn runtime quote capacity is exhausted",
        ));
    }
    if let Some(collision) = app.vpn_quotes.get(&record.quote_id) {
        if collision.account_id != record.account_id {
            return Err(not_permitted_error(
                "vpn quote identifier collides with another account",
            ));
        }
    }
    if let Some(existing) = existing {
        let _ = remove_quote_by_id_locked(app, state, &existing.quote_id);
    }
    state
        .quote_ids_by_account
        .insert(account_key, record.quote_id.clone());
    app.vpn_quotes.insert(record.quote_id.clone(), record);
    Ok(())
}

fn remove_session_by_id_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    session_id: &str,
) -> Option<VpnSessionRecord> {
    let record = app
        .vpn_sessions
        .get(session_id)
        .map(|entry| entry.clone())?;
    remove_session_record_locked(app, state, &record)
}

fn remove_session_record_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    expected: &VpnSessionRecord,
) -> Option<VpnSessionRecord> {
    let removed = app
        .vpn_sessions
        .remove(&expected.session_id)
        .map(|(_, record)| record);
    let account_key = account_runtime_key(&expected.account_id);
    if state
        .session_ids_by_account
        .get(&account_key)
        .is_some_and(|indexed_id| indexed_id == &expected.session_id)
    {
        state.session_ids_by_account.remove(&account_key);
    }
    app.vpn_used_payments.remove(&expected.payment_tx_hash);
    if let Some(record) = removed.as_ref() {
        app.vpn_used_payments.remove(&record.payment_tx_hash);
    }
    removed
}

fn session_for_account_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    account_id: &AccountId,
) -> Option<VpnSessionRecord> {
    #[cfg(test)]
    {
        state.session_account_lookups += 1;
    }
    let account_key = account_runtime_key(account_id);
    let session_id = state.session_ids_by_account.get(&account_key)?.clone();
    let record = app.vpn_sessions.get(&session_id).map(|entry| entry.clone());
    match record {
        Some(record) if &record.account_id == account_id => Some(record),
        _ => {
            // Self-heal only the authenticated account's stale reverse entry.
            state.session_ids_by_account.remove(&account_key);
            None
        }
    }
}

fn expire_session_record_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    record: &VpnSessionRecord,
    current_ms: u64,
) -> bool {
    if record.expires_at_ms > current_ms {
        return false;
    }
    if state.settling_session_ids.contains(&record.session_id) {
        return true;
    }
    if let Some(removed) = remove_session_record_locked(app, state, record) {
        store_receipt(app, build_receipt_record(&removed, current_ms, "expired"));
    }
    true
}

fn expire_session_for_account_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    account_id: &AccountId,
    current_ms: u64,
) {
    let Some(record) = session_for_account_locked(app, state, account_id) else {
        return;
    };
    let _ = expire_session_record_locked(app, state, &record, current_ms);
}

fn session_by_id_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    session_id: &str,
    current_ms: u64,
) -> Option<VpnSessionRecord> {
    let record = app
        .vpn_sessions
        .get(session_id)
        .map(|entry| entry.clone())?;
    if expire_session_record_locked(app, state, &record, current_ms) {
        return None;
    }
    Some(record)
}

fn reserve_session_settlement_locked(
    state: &mut VpnRuntimeState,
    session_id: &str,
) -> Result<(), Error> {
    if state.settling_session_ids.contains(session_id) {
        return Err(not_permitted_error(
            "vpn session receipt settlement is already in progress",
        ));
    }
    if state.settling_session_ids.len() >= state.session_capacity {
        return Err(not_permitted_error(
            "vpn runtime settlement capacity is exhausted",
        ));
    }
    state.settling_session_ids.insert(session_id.to_owned());
    Ok(())
}

struct VpnSettlementReservation {
    app: SharedAppState,
    session_id: String,
    active: bool,
}

impl VpnSettlementReservation {
    fn reserve(app: &SharedAppState, session_id: String) -> Result<Self, Error> {
        {
            let mut state = lock_vpn_runtime(app);
            reserve_session_settlement_locked(&mut state, &session_id)?;
        }
        Ok(Self {
            app: app.clone(),
            session_id,
            active: true,
        })
    }

    fn finish(mut self, record: &VpnSessionRecord, receipt: VpnReceiptRecord) {
        self.active = false;
        let mut state = lock_vpn_runtime(&self.app);
        state.settling_session_ids.remove(&self.session_id);
        let _ = remove_session_record_locked(&self.app, &mut state, record);
        store_receipt(&self.app, receipt);
    }
}

impl Drop for VpnSettlementReservation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = lock_vpn_runtime(&self.app);
        state.settling_session_ids.remove(&self.session_id);
    }
}

fn insert_session_locked(
    app: &SharedAppState,
    state: &mut VpnRuntimeState,
    record: VpnSessionRecord,
    current_ms: u64,
) -> Result<(), Error> {
    let account_key = account_runtime_key(&record.account_id);
    let existing = session_for_account_locked(app, state, &record.account_id);
    if existing.is_none() && state.session_ids_by_account.len() >= state.session_capacity {
        return Err(not_permitted_error(
            "vpn runtime session capacity is exhausted",
        ));
    }
    if app.vpn_used_payments.contains_key(&record.payment_tx_hash) {
        return Err(not_permitted_error(
            "vpn payment transaction was already used for a session",
        ));
    }
    if let Some(collision) = app.vpn_sessions.get(&record.session_id) {
        if collision.account_id != record.account_id {
            return Err(not_permitted_error(
                "vpn session identifier collides with another account",
            ));
        }
    }
    if let Some(existing) = existing {
        if state.settling_session_ids.contains(&existing.session_id) {
            return Err(not_permitted_error(
                "vpn session receipt settlement is in progress",
            ));
        }
        if let Some(removed) = remove_session_record_locked(app, state, &existing) {
            let status = if removed.expires_at_ms <= current_ms {
                "expired"
            } else {
                "replaced"
            };
            store_receipt(app, build_receipt_record(&removed, current_ms, status));
        }
    }
    app.vpn_used_payments
        .insert(record.payment_tx_hash.clone(), ());
    state
        .session_ids_by_account
        .insert(account_key, record.session_id.clone());
    app.vpn_sessions.insert(record.session_id.clone(), record);
    Ok(())
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
                TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                    return None;
                }
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
    open.quote.verify().map_err(|error| {
        conversion_error(format!("payment VPN quote signature is invalid: {error}"))
    })?;
    Ok(open.quote == quote.signed_quote)
}

fn verify_vpn_payment(
    app: &SharedAppState,
    quote: &VpnQuoteRecord,
    payment_tx_hash: &str,
) -> Result<(), Error> {
    validate_quote_record_projection(quote)?;
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
    let lease = lease_record_by_id(app, &quote.lease_id).ok_or_else(|| {
        not_permitted_error("vpn payment transaction did not retain the quoted native VPN lease")
    })?;
    if lease.status != VpnLeaseStatusV1::Active
        || lease.signed_quote != quote.signed_quote
        || lease.client_account_id != quote.account_id
        || lease.lease_id != quote.lease_id
        || lease.session_id != quote.session_id
    {
        return Err(not_permitted_error(
            "vpn payment does not resolve to the exact active consensus VPN lease",
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

fn lease_id_from_session_lookup(app: &SharedAppState, session_id: &str) -> Option<[u8; 32]> {
    let normalized = session_id
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if normalized.len() != 32 {
        return None;
    }
    let decoded = hex::decode(normalized).ok()?;
    let session_id: [u8; 16] = decoded.try_into().ok()?;
    let slot = VpnAddressSlotV1::from_session_id(session_id);
    let world = app.state.world_view();
    let lease_id = *world.vpn_active_lease_by_address_slot().get(&slot)?;
    let lease = world.vpn_leases().get(&lease_id)?;
    (lease.session_id == session_id).then_some(lease_id)
}

fn session_id_hex_from_lease(record: &VpnLeaseRecordV1) -> String {
    hex::encode(record.session_id)
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
        lease_id: record.lease_id,
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
    let Some(lease_id) = lease_id_from_session_lookup(app, session_id) else {
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
    let expected_exit_class = VpnExitClassV1::try_from_label(&record.exit_class)
        .map_err(|error| conversion_error(error.to_string()))?;
    if relay_receipt.exit_class != expected_exit_class {
        return Err(not_permitted_error(
            "vpn receipt exit class does not match the signed session",
        ));
    }
    if relay_receipt.started_at_ms < record.connected_at_ms
        || relay_receipt.ended_at_ms > record.expires_at_ms
    {
        return Err(not_permitted_error(
            "vpn receipt service interval falls outside the signed session",
        ));
    }
    if voucher.body.issued_at_ms < record.connected_at_ms
        || voucher.body.issued_at_ms > record.expires_at_ms
    {
        return Err(not_permitted_error(
            "vpn voucher issuance timestamp falls outside the signed session",
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
    let operator_account_id =
        parse_profile_account_id(&profile.operator_account_id, "operator_account_id")?;
    let quote_signer_account_id =
        AccountId::new(app.torii_proxy_bridge_signer.public_key().clone());
    if operator_account_id != quote_signer_account_id {
        return Err(not_permitted_error(
            "vpn operator account must match this Torii node's quote signing key",
        ));
    }
    ensure_vpn_quote_operator_authorized(&app, &operator_account_id)?;
    let metering_public_key = parse_metering_public_key(&request.metering_public_key_hex)?;

    let nonce = headers
        .get(crate::HEADER_NONCE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("vpn-quote");
    let quote_id = build_quote_id(&account_id, &exit_class, nonce, current_ms);
    let quote_id_bytes = decode_hex_32(&quote_id, "quote_id")?;
    let address_slot = derive_vpn_address_slot_v1(quote_id_bytes);
    let lease_id = derive_vpn_lease_id_v1(app.chain_id.as_ref(), quote_id_bytes, &account_id);
    let session_id = derive_vpn_session_id_v1(
        app.chain_id.as_ref(),
        quote_id_bytes,
        &account_id,
        address_slot,
    );
    let address_plan = derive_vpn_address_plan_v1(address_slot);
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
    let settlement_grace_ms = profile
        .settlement_grace_secs
        .checked_mul(1_000)
        .ok_or_else(|| conversion_error("vpn settlement grace overflows milliseconds"))?;
    let asset_definition = xor_asset_definition_id();
    let escrow_account_id =
        vpn_lease_custody_account_id(app.chain_id.as_ref(), &lease_id, &asset_definition).map_err(
            |error| conversion_error(format!("cannot derive VPN custody account: {error}")),
        )?;
    if escrow_account_id == operator_account_id {
        return Err(not_permitted_error(
            "vpn protocol custody account must differ from the relay operator",
        ));
    }
    let quote_policy = VpnQuotePolicyV1 {
        exit_class: VpnExitClassV1::try_from_label(&exit_class)
            .map_err(|error| conversion_error(error.to_string()))?,
        relay_endpoint: profile.relay_endpoint.clone(),
        relay_id: trust.relay_id,
        descriptor_commit: trust.descriptor_commit,
        tls_server_name: trust.tls_server_name.clone(),
        relay_tls_spki_sha256: trust.relay_tls_spki_sha256,
        relay_certificate_sha256: trust.relay_certificate_sha256,
        directory_snapshot_digest: trust.directory_snapshot_digest,
        relay_trust_valid_until_ms: trust.valid_until_ms,
        lease_secs: profile.lease_secs,
        meter_family: profile.meter_family.clone(),
        fee_asset_id: asset_definition.to_string(),
        escrow_account_id: escrow_account_id.clone(),
        route_pushes: profile.route_pushes.clone(),
        excluded_routes: profile.excluded_routes.clone(),
        dns_servers: profile.dns_servers.clone(),
        tunnel_addresses: address_plan.client_tunnel_addresses.clone(),
        mtu_bytes: profile.mtu_bytes,
        flow_label_bits: profile.flow_label_bits,
        padding_budget_ms: profile.padding_budget_ms,
    };
    let signed_quote = VpnSignedQuoteV1::try_sign(
        VpnQuoteBodyV1 {
            chain_id: app.chain_id.as_ref().clone(),
            quote_id: quote_id_bytes,
            lease_id,
            session_id,
            address_slot,
            client_account_id: account_id.clone(),
            operator_account_id: operator_account_id.clone(),
            metering_public_key: metering_public_key.clone(),
            asset_definition,
            tariff: tariff.clone(),
            policy: quote_policy,
            valid_after_ms: current_ms,
            expires_at_ms: quote_expires_at_ms,
            settlement_grace_ms,
        },
        app.torii_proxy_bridge_signer.private_key(),
    )
    .map_err(|error| conversion_error(format!("cannot sign canonical VPN quote: {error}")))?;
    let record = VpnQuoteRecord {
        quote_id: quote_id.clone(),
        lease_id,
        session_id,
        signed_quote,
        account_id,
        exit_class,
        relay_endpoint: profile.relay_endpoint,
        lease_secs: profile.lease_secs,
        quote_expires_at_ms,
        payment_reference: quote_id.clone(),
        fee_asset_id: xor_asset_definition_id().to_string(),
        escrow_account_id,
        operator_account_id,
        lease_fee: profile.lease_fee,
        tariff,
        settlement_grace_ms,
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
    let mut vpn_state = lock_vpn_runtime(&app);
    expire_quote_for_account_locked(&app, &mut vpn_state, &record.account_id, current_ms);
    expire_session_for_account_locked(&app, &mut vpn_state, &record.account_id, current_ms);
    insert_quote_locked(&app, &mut vpn_state, record)?;
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
    let quote_id = request.quote_id.trim();
    if quote_id.is_empty() {
        return Err(conversion_error("quote_id must not be empty"));
    }
    let exit_class = normalize_exit_class(&request.exit_class, &vpn.exit_class)?;
    let metering_public_key = parse_metering_public_key(&request.metering_public_key_hex)?;
    if request.payment_tx_hash.trim().is_empty() {
        return Err(conversion_error("payment_tx_hash must not be empty"));
    }
    let payment_tx_hash = hex::encode(decode_hex_32(&request.payment_tx_hash, "payment_tx_hash")?);
    let quote = {
        let mut vpn_state = lock_vpn_runtime(&app);
        expire_quote_for_account_locked(&app, &mut vpn_state, &account_id, current_ms);
        expire_session_for_account_locked(&app, &mut vpn_state, &account_id, current_ms);
        let Some(quote) = quote_by_id_locked(&app, &mut vpn_state, quote_id, current_ms) else {
            return Err(not_permitted_error(
                "vpn quote is missing, expired, or already consumed",
            ));
        };
        if quote.account_id != account_id {
            return Err(not_permitted_error(
                "vpn quote belongs to a different account",
            ));
        }
        if quote.exit_class != exit_class {
            return Err(not_permitted_error(
                "vpn quote exit class does not match session request",
            ));
        }
        if metering_public_key != quote.metering_public_key {
            return Err(not_permitted_error(
                "vpn session metering key does not match the quoted native lease",
            ));
        }
        remove_quote_by_id_locked(&app, &mut vpn_state, quote_id)
            .ok_or_else(|| conversion_error("vpn quote disappeared while creating the session"))?
    };

    // Payment verification reads committed WSV/block state and must not hold
    // the compound runtime-cache lock while doing so.
    verify_vpn_payment(&app, &quote, &payment_tx_hash)?;

    let session_id = build_session_id_from_quote(&quote);
    let expires_at_ms = quote.quote_expires_at_ms;
    let mut record = VpnSessionRecord {
        session_id: session_id.clone(),
        lease_id: quote.lease_id,
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
    let mut vpn_state = lock_vpn_runtime(&app);
    insert_session_locked(&app, &mut vpn_state, record, current_ms)?;
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
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Err(conversion_error("session_id must not be empty"));
    }
    let cached_record = {
        let mut vpn_state = lock_vpn_runtime(&app);
        session_by_id_locked(&app, &mut vpn_state, normalized_session_id, current_ms)
    };
    let record = if let Some(record) = cached_record {
        record
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
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Err(conversion_error("session_id must not be empty"));
    }
    let mut vpn_state = lock_vpn_runtime(&app);
    let Some(record) =
        session_by_id_locked(&app, &mut vpn_state, normalized_session_id, current_ms)
    else {
        return Ok(StatusCode::NOT_FOUND.into_response());
    };
    if record.account_id != account_id {
        return Err(not_permitted_error(
            "vpn session belongs to a different account",
        ));
    }
    if vpn_state
        .settling_session_ids
        .contains(normalized_session_id)
    {
        return Err(not_permitted_error(
            "vpn session receipt settlement is in progress",
        ));
    }
    let removed = remove_session_by_id_locked(&app, &mut vpn_state, normalized_session_id)
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
    {
        let mut vpn_state = lock_vpn_runtime(&app);
        expire_session_for_account_locked(&app, &mut vpn_state, &account_id, now_ms());
    }
    // Receipt projection can read bounded WSV indexes after the short runtime
    // mutation critical section has completed.
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
    let session_id_hex = hex::encode(relay_receipt.session_id);
    let settlement = VpnSettlementReservation::reserve(&app, session_id_hex.clone())?;
    // The reservation prevents delete/replacement races without retaining the
    // runtime-state lock across WSV reads or receipt signature verification.
    let indexed_lease_id =
        lease_id_from_session_lookup(&app, &session_id_hex).ok_or_else(|| {
            not_permitted_error(
                "vpn receipt does not match an active consensus-indexed VPN session",
            )
        })?;
    let (lease_id, lease_id_hex) =
        settlement_lease_id_from_request_or_index(&request, indexed_lease_id)?;
    let lease_record = lease_record_by_id(&app, &lease_id).ok_or_else(|| {
        not_permitted_error("vpn receipt does not match an on-chain native VPN lease")
    })?;
    if lease_record.status != VpnLeaseStatusV1::Active {
        return Err(not_permitted_error("vpn lease is not active"));
    }
    if current_ms >= lease_record.refund_available_at_ms() {
        return Err(not_permitted_error(
            "vpn lease settlement grace window expired",
        ));
    }
    if relay_receipt.ended_at_ms > current_ms || voucher.body.issued_at_ms > current_ms {
        return Err(not_permitted_error(
            "vpn receipt and voucher must not be dated in the future",
        ));
    }
    let record = session_record_from_lease(&lease_record)?;
    if signed_account != record.operator_account_id {
        return Err(not_permitted_error(
            "vpn receipt submission must be signed by the configured operator account",
        ));
    }
    verify_relay_receipt_for_session(&record, &relay_receipt, &voucher)?;
    let receipt = build_settled_receipt_record(
        &record,
        &relay_receipt,
        &voucher,
        lease_id,
        lease_id_hex,
        current_ms,
    )?;
    settlement.finish(&record, receipt.clone());
    Ok((
        StatusCode::CREATED,
        crate::utils::JsonBody(receipt_response_from_record(&receipt)),
    )
        .into_response())
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    include!("vpn_tests.rs");
}
