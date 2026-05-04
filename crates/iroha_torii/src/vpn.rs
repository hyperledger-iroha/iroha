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
use iroha_core::kiso::KisoHandle;
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
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
        VPN_DEFAULT_TUNNEL_MTU_BYTES, VpnExitClassV1, VpnHelperTicketV1, VpnSessionReceiptV1,
        VpnTariffV1, VpnUsageVoucherV1, derive_vpn_session_address_plan_v1,
    },
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};

use crate::{Error, SharedAppState};

const SUPPORTED_EXIT_CLASSES: [&str; 3] = ["standard", "low-latency", "high-security"];
const DEFAULT_TUNNEL_ADDRESSES: [&str; 2] = ["10.208.0.2/32", "fd53:7261:6574::2/128"];
const MAX_RECEIPTS_PER_ACCOUNT: usize = 24;
const MAX_SESSION_ADDRESS_ALLOCATION_ATTEMPTS: u32 = 4_096;

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
    pub lease_fee_nanos: u64,
    pub settlement_grace_secs: u64,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_tls_spki_sha256_hex: Option<String>,
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
    pub lease_fee_nanos: u64,
    pub route_pushes: Vec<String>,
    pub excluded_routes: Vec<String>,
    pub dns_servers: Vec<String>,
    pub tunnel_addresses: Vec<String>,
    pub mtu_bytes: u64,
    pub meter_family: String,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_tls_spki_sha256_hex: Option<String>,
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
    pub lease_fee_nanos: u64,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_tls_spki_sha256_hex: Option<String>,
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
    pub lease_fee_nanos: u64,
    pub earned_fee_nanos: u64,
    pub refunded_fee_nanos: u64,
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
    pub lease_fee_nanos: u64,
    pub tariff: VpnTariffV1,
    pub flow_label_bits: u8,
    pub padding_budget_ms: u16,
    pub relay_tls_spki_sha256_hex: Option<String>,
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
    pub lease_fee_nanos: u64,
    pub earned_fee_nanos: u64,
    pub refunded_fee_nanos: u64,
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
    pub lease_fee_nanos: u64,
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
    pub relay_tls_spki_sha256_hex: Option<String>,
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

fn build_profile(dto: &ConfigGetDTO) -> VpnProfileResponseDto {
    let vpn = &dto.network.soranet_vpn;
    let relay_endpoint = dto.transport.streaming.soranet.exit_multiaddr.clone();
    let default_exit_class = vpn.exit_class.trim().to_owned();
    let supported_exit_classes = SUPPORTED_EXIT_CLASSES
        .iter()
        .map(|item| (*item).to_owned())
        .collect::<Vec<_>>();
    VpnProfileResponseDto {
        available: vpn.enabled,
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
            "{default_exit_class} · {} · {} nano-XOR",
            vpn.meter_family, vpn.lease_fee_nanos
        ),
        fee_asset_id: vpn.fee_asset_id.clone(),
        escrow_account_id: vpn.escrow_account_id.clone(),
        operator_account_id: vpn.operator_account_id.clone(),
        lease_fee_nanos: vpn.lease_fee_nanos,
        settlement_grace_secs: vpn.settlement_grace_secs,
        flow_label_bits: vpn.flow_label_bits,
        padding_budget_ms: vpn.padding_budget_ms,
        relay_tls_spki_sha256_hex: vpn.relay_tls_spki_sha256_hex.clone(),
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

fn relay_id_from_endpoint(endpoint: &str) -> [u8; 32] {
    fixed_hash_bytes(endpoint)
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

fn public_key_payload_hex(public_key: &PublicKey) -> String {
    let (_, payload) = public_key.to_bytes();
    hex::encode(payload)
}

fn xor_asset_definition_id() -> AssetDefinitionId {
    let domain =
        DomainId::parse_fully_qualified("universal.universal").expect("static XOR domain id");
    let name = Name::from_str("xor").expect("static XOR asset name");
    AssetDefinitionId::new(domain, name)
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

fn active_fee_nanos_per_minute(lease_fee_nanos: u64, lease_secs: u64) -> u64 {
    let numerator = u128::from(lease_fee_nanos).saturating_mul(60);
    let denominator = u128::from(lease_secs.max(1));
    u64::try_from(numerator.div_ceil(denominator)).unwrap_or(u64::MAX)
}

fn vpn_tariff_for_lease(lease_fee_nanos: u64, lease_secs: u64) -> VpnTariffV1 {
    VpnTariffV1 {
        lease_fee_nanos,
        active_fee_nanos_per_minute: active_fee_nanos_per_minute(lease_fee_nanos, lease_secs),
        ingress_fee_nanos_per_mib: 0,
        egress_fee_nanos_per_mib: 0,
    }
}

fn build_helper_ticket_hex(
    record: &VpnSessionRecord,
    expires_at_ms: u64,
    secret: Option<&[u8; 32]>,
) -> Result<String, Error> {
    let secret = secret.ok_or_else(|| {
        not_permitted_error("vpn helper ticket secret is not configured on this Torii node")
    })?;
    Ok(VpnHelperTicketV1 {
        session_id: relay_session_id_from_session_id(&record.session_id),
        quote_id: decode_hex_32(&record.quote_id, "quote_id")?,
        account_hash: account_hash(&record.account_id),
        relay_id: relay_id_from_endpoint(&record.relay_endpoint),
        payment_tx_hash: decode_hex_32(&record.payment_tx_hash, "payment_tx_hash")?,
        expires_at_ms,
    }
    .to_hex(secret))
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

fn open_lease_instruction(record: &VpnQuoteRecord) -> Result<VpnTxInstructionDto, Error> {
    let lease_id = decode_hex_32(&record.quote_id, "quote_id")?;
    let asset_definition = parse_fee_asset_definition(&record.fee_asset_id)?;
    let instruction: InstructionBox = OpenVpnLeaseEscrow::new(
        lease_id,
        relay_session_id_from_session_id(&record.quote_id),
        lease_id,
        relay_id_from_endpoint(&record.relay_endpoint),
        record.operator_account_id.clone(),
        record.metering_public_key.clone(),
        asset_definition,
        record.tariff.lease_fee_numeric(),
        record.tariff,
        record.quote_expires_at_ms,
        record.settlement_grace_ms,
    )
    .into();
    Ok(tx_instr_from_box(instruction))
}

fn settlement_lease_id(
    request: &VpnReceiptSubmitRequestDto,
    record: &VpnSessionRecord,
) -> Result<([u8; 32], String), Error> {
    let lease_id_hex = request.lease_id_hex.trim();
    if lease_id_hex.is_empty() {
        let fallback = default_lease_id_hex(record);
        return Ok((decode_hex_32(&fallback, "lease_id_hex")?, fallback));
    }
    let lease_id = decode_hex_32(lease_id_hex, "lease_id_hex")?;
    Ok((
        lease_id,
        lease_id_hex
            .trim_start_matches("0x")
            .trim_start_matches("0X")
            .to_owned(),
    ))
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
        lease_fee_nanos: record.lease_fee_nanos,
        route_pushes: record.route_pushes.clone(),
        excluded_routes: record.excluded_routes.clone(),
        dns_servers: record.dns_servers.clone(),
        tunnel_addresses: record.tunnel_addresses.clone(),
        mtu_bytes: record.mtu_bytes,
        meter_family: record.meter_family.clone(),
        flow_label_bits: record.flow_label_bits,
        padding_budget_ms: record.padding_budget_ms,
        relay_tls_spki_sha256_hex: record.relay_tls_spki_sha256_hex.clone(),
        metering_public_key_hex: public_key_payload_hex(&record.metering_public_key),
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
        lease_fee_nanos: record.lease_fee_nanos,
        flow_label_bits: record.flow_label_bits,
        padding_budget_ms: record.padding_budget_ms,
        relay_tls_spki_sha256_hex: record.relay_tls_spki_sha256_hex.clone(),
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
        lease_fee_nanos: record.lease_fee_nanos,
        earned_fee_nanos: record.earned_fee_nanos,
        refunded_fee_nanos: record.refunded_fee_nanos,
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
        lease_fee_nanos: record.lease_fee_nanos,
        earned_fee_nanos: 0,
        refunded_fee_nanos: record.lease_fee_nanos,
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
) -> VpnReceiptRecord {
    let duration_ms = disconnected_at_ms.saturating_sub(record.connected_at_ms);
    let earned_fee_nanos = relay_receipt.earned_fee_nanos.min(record.lease_fee_nanos);
    VpnReceiptRecord {
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
        lease_fee_nanos: record.lease_fee_nanos,
        earned_fee_nanos,
        refunded_fee_nanos: record.lease_fee_nanos.saturating_sub(earned_fee_nanos),
        lease_id_hex,
        settle_lease_instruction: Some(settle_lease_instruction(
            lease_id,
            *relay_receipt,
            voucher.clone(),
        )),
    }
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
) -> Vec<VpnReceiptResponseDto> {
    app.vpn_receipts
        .get(&account_id.to_string())
        .map(|entry| {
            entry
                .iter()
                .map(receipt_response_from_record)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
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
        && open.relay_id == relay_id_from_endpoint(&quote.relay_endpoint)
        && open.operator_account_id == quote.operator_account_id
        && open.metering_public_key == quote.metering_public_key
        && open.asset_definition == asset_definition
        && open.lease_fee == quote.tariff.lease_fee_numeric()
        && open.tariff == quote.tariff
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
    let Executable::Instructions(instructions) = tx.instructions() else {
        return Err(not_permitted_error(
            "vpn payment transaction must be a native instruction transaction",
        ));
    };
    let mut matched = false;
    for instruction in instructions {
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
            "vpn payment transaction must open the quoted native XOR VPN lease escrow",
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

fn session_record_for_relay_receipt(
    app: &SharedAppState,
    relay_receipt: &VpnSessionReceiptV1,
) -> Option<VpnSessionRecord> {
    app.vpn_sessions.iter().find_map(|entry| {
        let record = entry.value();
        if relay_session_id_from_session_id(&record.session_id) == relay_receipt.session_id {
            Some(record.clone())
        } else {
            None
        }
    })
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
    let expected_relay_id = relay_id_from_endpoint(&record.relay_endpoint);
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

    let expected_earned_fee = legacy_session_earned_fee_nanos(record, voucher);
    if relay_receipt.earned_fee_nanos != expected_earned_fee {
        return Err(not_permitted_error(
            "vpn receipt earned fee does not match the session tariff",
        ));
    }
    Ok(())
}

fn legacy_session_earned_fee_nanos(record: &VpnSessionRecord, voucher: &VpnUsageVoucherV1) -> u64 {
    record.tariff.earned_fee_nanos(&voucher.body)
}

pub(crate) async fn handle_get_vpn_profile(kiso: KisoHandle) -> Result<Response, Error> {
    let dto = kiso.get_dto().await?;
    Ok(crate::utils::JsonBody(build_profile(&dto)).into_response())
}

pub(crate) async fn handle_create_vpn_quote(
    app: SharedAppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<Response, Error> {
    let request: VpnQuoteCreateRequestDto = norito::json::from_slice(body)
        .map_err(|err| conversion_error(format!("invalid vpn quote create payload: {err}")))?;
    let account_id = require_signed_request(&app, headers, method, uri, body)?;
    let dto = app.kiso.get_dto().await?;
    let profile = build_profile(&dto);
    if !profile.available {
        return Err(not_permitted_error("vpn is disabled on this Torii node"));
    }
    if app.vpn_helper_ticket_secret.is_none() {
        return Err(not_permitted_error(
            "vpn helper ticket secret is not configured on this Torii node",
        ));
    }
    let relay_tls_spki_sha256_hex = profile
        .relay_tls_spki_sha256_hex
        .clone()
        .ok_or_else(|| not_permitted_error("vpn relay TLS SPKI pin is not configured"))?;
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

    let current_ms = now_ms();
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
    let quote_expires_at_ms = current_ms.saturating_add(profile.lease_secs.saturating_mul(1_000));
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
        lease_fee_nanos: profile.lease_fee_nanos,
        tariff: vpn_tariff_for_lease(profile.lease_fee_nanos, profile.lease_secs),
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
        relay_tls_spki_sha256_hex: Some(relay_tls_spki_sha256_hex),
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
    let request: VpnSessionCreateRequestDto = norito::json::from_slice(body)
        .map_err(|err| conversion_error(format!("invalid vpn session create payload: {err}")))?;
    let account_id = require_signed_request(&app, headers, method, uri, body)?;
    let dto = app.kiso.get_dto().await?;
    let profile = build_profile(&dto);
    if !profile.available {
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
    let exit_class = normalize_exit_class(&request.exit_class, &profile.default_exit_class)?;
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
    verify_vpn_payment(&app, &quote, &request.payment_tx_hash)?;

    remove_existing_sessions_for_account(&app, &account_id, current_ms);

    let payment_tx_hash = request
        .payment_tx_hash
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .to_owned();
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
        lease_fee_nanos: quote.lease_fee_nanos,
        tariff: quote.tariff,
        flow_label_bits: quote.flow_label_bits,
        padding_budget_ms: quote.padding_budget_ms,
        relay_tls_spki_sha256_hex: quote.relay_tls_spki_sha256_hex,
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
    let Some(record) = app.vpn_sessions.get(normalized_session_id) else {
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
    let items = list_receipts_for_account(&app, &account_id);
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
    let request: VpnReceiptSubmitRequestDto = norito::json::from_slice(body)
        .map_err(|err| conversion_error(format!("invalid vpn receipt payload: {err}")))?;
    let signed_account = require_signed_request(&app, headers, method, uri, body)?;
    let relay_receipt: VpnSessionReceiptV1 =
        decode_norito_hex(&request.relay_receipt_hex, "relay_receipt_hex")?;
    let voucher: VpnUsageVoucherV1 =
        decode_norito_hex(&request.client_voucher_hex, "client_voucher_hex")?;

    let current_ms = now_ms();
    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_sessions(&app, current_ms);
    let record = session_record_for_relay_receipt(&app, &relay_receipt)
        .ok_or_else(|| not_permitted_error("vpn receipt does not match an active session"))?;
    if signed_account != record.operator_account_id {
        return Err(not_permitted_error(
            "vpn receipt submission must be signed by the configured operator account",
        ));
    }
    verify_relay_receipt_for_session(&record, &relay_receipt, &voucher)?;
    let (lease_id, lease_id_hex) = settlement_lease_id(&request, &record)?;
    app.vpn_sessions.remove(&record.session_id);
    let receipt = build_settled_receipt_record(
        &record,
        &relay_receipt,
        &voucher,
        lease_id,
        lease_id_hex,
        current_ms,
    );
    store_receipt(&app, receipt.clone());
    Ok((
        StatusCode::CREATED,
        crate::utils::JsonBody(receipt_response_from_record(&receipt)),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{body::to_bytes, response::IntoResponse};
    use iroha_core::state::World;
    use iroha_crypto::{KeyPair, Signature};
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
        cfg.network.soranet_vpn.relay_tls_spki_sha256_hex = Some("ab".repeat(32));
        cfg.network.soranet_vpn.operator_account_id = operator_account_id.clone();

        let mut inner = match Arc::try_unwrap(app) {
            Ok(inner) => inner,
            Err(_) => panic!("test app should be uniquely owned before VPN reconfiguration"),
        };
        inner.kiso = KisoHandle::mock(&cfg);
        inner.vpn_helper_ticket_secret = Some([0x5A; 32]);
        Arc::new(inner)
    }

    fn metering_public_key_hex(key_pair: &KeyPair) -> String {
        let (_, payload) = key_pair.public_key().to_bytes();
        hex::encode(payload)
    }

    async fn create_quote_for_account(
        app: SharedAppState,
        account: &AccountId,
        key_pair: &KeyPair,
        exit_class: &str,
    ) -> (VpnQuoteResponseDto, KeyPair) {
        let metering_keys = KeyPair::random();
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
        let metering_keys = KeyPair::random();
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
            lease_fee_nanos: 1_000_000,
            tariff: vpn_tariff_for_lease(1_000_000, 600),
            flow_label_bits: 24,
            padding_budget_ms: 15,
            relay_tls_spki_sha256_hex: Some("ab".repeat(32)),
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
        let account = account_id_for(&KeyPair::random());
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);

        let response = handle_get_vpn_profile(app.kiso.clone())
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
        assert_eq!(body.relay_tls_spki_sha256_hex, Some("ab".repeat(32)));
    }

    #[tokio::test]
    async fn create_vpn_quote_rejects_operator_owned_escrow() {
        let key_pair = KeyPair::random();
        let account = account_id_for(&key_pair);
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let mut cfg = crate::test_utils::mk_minimal_root_cfg();
        cfg.network.soranet_vpn.enabled = true;
        cfg.network.soranet_vpn.relay_tls_spki_sha256_hex = Some("ab".repeat(32));
        cfg.network.soranet_vpn.escrow_account_id = account.clone();
        cfg.network.soranet_vpn.operator_account_id = account.clone();
        let mut inner = match Arc::try_unwrap(app) {
            Ok(inner) => inner,
            Err(_) => panic!("test app should be uniquely owned"),
        };
        inner.kiso = KisoHandle::mock(&cfg);
        inner.vpn_helper_ticket_secret = Some([0x5A; 32]);
        let app = Arc::new(inner);

        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("quote uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: "standard".to_owned(),
            metering_public_key_hex: metering_public_key_hex(&KeyPair::random()),
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
        let account = account_id_for(&KeyPair::random());
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
        assert_eq!(
            relay_id_from_endpoint(&record.relay_endpoint),
            parsed.relay_id
        );
        assert_eq!(expires_at_ms, parsed.expires_at_ms);
    }

    #[test]
    fn settlement_lease_id_accepts_explicit_prefixed_hex() {
        let account = account_id_for(&KeyPair::random());
        let record = sample_session_record(&account);
        let request = VpnReceiptSubmitRequestDto {
            relay_receipt_hex: String::new(),
            client_voucher_hex: String::new(),
            lease_id_hex: format!("0x{}", "33".repeat(32)),
        };

        let (lease_id, normalized_hex) =
            settlement_lease_id(&request, &record).expect("explicit lease id");

        assert_eq!(lease_id, [0x33; 32]);
        assert_eq!(normalized_hex, "33".repeat(32));
    }

    #[tokio::test]
    async fn create_vpn_session_requires_signed_headers() {
        let account = account_id_for(&KeyPair::random());
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
    async fn create_get_delete_and_list_vpn_session_roundtrip_for_signed_account() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = KeyPair::random();
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
    async fn vpn_quote_create_rejects_replayed_nonce() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = KeyPair::random();
        let account = account_id_for(&key_pair);
        let app = vpn_enabled_app_with_operator(world_with_account(&account), &account);
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/quotes".parse().expect("uri");
        let body = norito::json::to_vec(&VpnQuoteCreateRequestDto {
            exit_class: "standard".to_owned(),
            metering_public_key_hex: metering_public_key_hex(&KeyPair::random()),
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
        let owner_keys = KeyPair::random();
        let intruder_keys = KeyPair::random();
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
        let key_pair = KeyPair::random();
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
    async fn vpn_address_allocator_avoids_collisions_across_active_sessions() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let first_keys = KeyPair::random();
        let second_keys = KeyPair::random();
        let third_keys = KeyPair::random();
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
    async fn submit_vpn_receipt_requires_operator_and_client_voucher() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let user_keys = KeyPair::random();
        let operator_keys = KeyPair::random();
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

        let relay_session_id = relay_session_id_from_session_id(&session.session_id);
        let quote_id = decode_hex_32(&session.quote_id, "quote").expect("quote id");
        let relay_id = relay_id_from_endpoint(&session.relay_endpoint);
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
        let voucher = VpnUsageVoucherV1 {
            signature: Signature::new(metering_keys.private_key(), &voucher_body.encode()),
            client_public_key: metering_keys.public_key().clone(),
            body: voucher_body,
        };
        let earned_fee_nanos = {
            let record = app
                .vpn_sessions
                .get(&session.session_id)
                .expect("active session record");
            legacy_session_earned_fee_nanos(&record, &voucher)
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
            earned_fee_nanos,
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

        let response =
            handle_submit_vpn_receipt(app.clone(), &method, &uri, &headers, body.as_ref())
                .await
                .expect("settled")
                .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
        let settled: VpnReceiptResponseDto = read_json(response).await;
        assert_eq!(settled.status, "settled");
        assert_eq!(settled.receipt_source, "relay");
        assert_eq!(settled.earned_fee_nanos, earned_fee_nanos);
        assert_eq!(
            settled.refunded_fee_nanos,
            session.lease_fee_nanos.saturating_sub(earned_fee_nanos)
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
