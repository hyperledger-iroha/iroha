use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use iroha_config::client_api::ConfigGetDTO;
use iroha_core::kiso::KisoHandle;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    query::error::QueryExecutionFail,
    soranet::vpn::{
        VPN_DEFAULT_TUNNEL_MTU_BYTES, VpnExitClassV1, VpnHelperTicketV1,
        derive_vpn_session_address_plan_v1,
    },
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
        route_pushes: Vec::new(),
        excluded_routes: Vec::new(),
        dns_servers: Vec::new(),
        tunnel_addresses: default_tunnel_addresses(),
        mtu_bytes: u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES),
        display_billing_label: format!("{default_exit_class} · {}", vpn.meter_family),
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

fn build_helper_ticket_hex(
    session_id: &str,
    expires_at_ms: u64,
    secret: Option<&[u8; 32]>,
) -> String {
    match secret {
        Some(secret) => VpnHelperTicketV1 {
            session_id: relay_session_id_from_session_id(session_id),
            expires_at_ms,
        }
        .to_hex(secret),
        None => hex::encode(blake3::hash(format!("{session_id}:helper").as_bytes()).as_bytes()),
    }
}

fn address_plan_fingerprint(client_tunnel_addresses: &[String]) -> String {
    client_tunnel_addresses.join("|")
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

pub(crate) async fn handle_get_vpn_profile(kiso: KisoHandle) -> Result<Response, Error> {
    let dto = kiso.get_dto().await?;
    Ok(crate::utils::JsonBody(build_profile(&dto)).into_response())
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
    let exit_class = normalize_exit_class(&request.exit_class, &profile.default_exit_class)?;
    let current_ms = now_ms();
    let _vpn_guard = app.vpn_state_lock.lock().await;
    prune_expired_sessions(&app, current_ms);
    remove_existing_sessions_for_account(&app, &account_id, current_ms);

    let nonce = headers
        .get(crate::HEADER_NONCE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("vpn");
    let (session_id, address_plan) =
        allocate_session_id_and_address_plan(&app, &account_id, &exit_class, nonce, current_ms)?;
    let expires_at_ms = current_ms.saturating_add(profile.lease_secs.saturating_mul(1_000));
    let helper_ticket_hex = build_helper_ticket_hex(
        &session_id,
        expires_at_ms,
        app.vpn_helper_ticket_secret.as_ref(),
    );
    let record = VpnSessionRecord {
        session_id: session_id.clone(),
        account_id,
        exit_class,
        relay_endpoint: profile.relay_endpoint,
        lease_secs: profile.lease_secs,
        expires_at_ms,
        connected_at_ms: current_ms,
        meter_family: profile.meter_family,
        route_pushes: profile.route_pushes,
        excluded_routes: profile.excluded_routes,
        dns_servers: profile.dns_servers,
        tunnel_addresses: address_plan.client_tunnel_addresses,
        mtu_bytes: profile.mtu_bytes,
        helper_ticket_hex,
        bytes_in: 0,
        bytes_out: 0,
    };
    let response = response_from_record(&record);
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

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};
    use iroha_core::state::World;
    use iroha_crypto::KeyPair;
    use iroha_data_model::{
        Registrable,
        account::{Account, AccountId},
        domain::{Domain, DomainId},
    };

    use super::*;
    use crate::tests_runtime_handlers::{
        app_auth_test_guard, mk_app_state_for_tests_with_world, signed_app_headers,
        world_with_account,
    };

    fn account_id_for(key_pair: &KeyPair) -> AccountId {
        AccountId::new(key_pair.public_key().clone())
    }

    fn world_with_accounts(accounts: &[AccountId]) -> World {
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(
            accounts
                .first()
                .expect("at least one account is required for test world"),
        );
        let accounts = accounts
            .iter()
            .cloned()
            .map(|account_id| {
                Account::new_in_domain(account_id.clone(), domain_id.clone()).build(&account_id)
            })
            .collect::<Vec<_>>();
        World::with([domain], accounts, [])
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
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));

        let response = handle_get_vpn_profile(app.kiso.clone())
            .await
            .expect("profile")
            .into_response();
        let body: VpnProfileResponseDto = read_json(response).await;

        assert_eq!(body.default_exit_class, "standard");
        assert_eq!(
            body.supported_exit_classes,
            vec!["standard", "low-latency", "high-security"]
        );
        assert!(!body.relay_endpoint.trim().is_empty());
        assert_eq!(body.mtu_bytes, u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES));
        assert_eq!(body.tunnel_addresses, default_tunnel_addresses());
    }

    #[test]
    fn helper_ticket_uses_versioned_ticket_when_secret_is_present() {
        let secret = [0x5A; 32];
        let session_id = "sess_live";
        let expires_at_ms = 50_000;
        let encoded = build_helper_ticket_hex(session_id, expires_at_ms, Some(&secret));
        let parsed =
            VpnHelperTicketV1::parse_hex(&encoded, &secret, 1).expect("ticket should parse");
        assert_eq!(
            relay_session_id_from_session_id(session_id),
            parsed.session_id
        );
        assert_eq!(expires_at_ms, parsed.expires_at_ms);
    }

    #[tokio::test]
    async fn create_vpn_session_requires_signed_headers() {
        let account = account_id_for(&KeyPair::random());
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "standard".to_owned(),
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
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "low-latency".to_owned(),
        })
        .expect("body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let created =
            handle_create_vpn_session(app.clone(), &method, &uri, &headers, body.as_ref())
                .await
                .expect("created")
                .into_response();
        assert_eq!(created.status(), StatusCode::CREATED);
        let session: VpnSessionResponseDto = read_json(created).await;
        assert_eq!(session.account_id, account.to_string());
        assert_eq!(session.exit_class, "low-latency");
        assert_eq!(session.status, "active");
        assert_ne!(session.tunnel_addresses, default_tunnel_addresses());
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
    async fn vpn_session_create_rejects_replayed_nonce() {
        let _guard = app_auth_test_guard(crate::app_auth::CanonicalRequestAuthConfig::default());
        let key_pair = KeyPair::random();
        let account = account_id_for(&key_pair);
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
        let body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "standard".to_owned(),
        })
        .expect("body");
        let headers = signed_app_headers(&account, &key_pair, &method, &uri, body.as_ref());

        let first = handle_create_vpn_session(app.clone(), &method, &uri, &headers, body.as_ref())
            .await
            .expect("first")
            .into_response();
        assert_eq!(first.status(), StatusCode::CREATED);

        let error =
            match handle_create_vpn_session(app, &method, &uri, &headers, body.as_ref()).await {
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
        let app = mk_app_state_for_tests_with_world(world);
        let create_method = Method::POST;
        let create_uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
        let create_body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "high-security".to_owned(),
        })
        .expect("body");
        let create_headers = signed_app_headers(
            &owner,
            &owner_keys,
            &create_method,
            &create_uri,
            create_body.as_ref(),
        );
        let created = handle_create_vpn_session(
            app.clone(),
            &create_method,
            &create_uri,
            &create_headers,
            create_body.as_ref(),
        )
        .await
        .expect("created")
        .into_response();
        let session: VpnSessionResponseDto = read_json(created).await;

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
        let app = mk_app_state_for_tests_with_world(world_with_account(&account));
        let method = Method::POST;
        let uri: Uri = "/v1/vpn/sessions".parse().expect("uri");

        let first_body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "standard".to_owned(),
        })
        .expect("body");
        let first_headers =
            signed_app_headers(&account, &key_pair, &method, &uri, first_body.as_ref());
        handle_create_vpn_session(
            app.clone(),
            &method,
            &uri,
            &first_headers,
            first_body.as_ref(),
        )
        .await
        .expect("first");

        let second_body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "low-latency".to_owned(),
        })
        .expect("body");
        let second_headers =
            signed_app_headers(&account, &key_pair, &method, &uri, second_body.as_ref());
        handle_create_vpn_session(
            app.clone(),
            &method,
            &uri,
            &second_headers,
            second_body.as_ref(),
        )
        .await
        .expect("second");

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
        let app = mk_app_state_for_tests_with_world(world);
        let create_method = Method::POST;
        let create_uri: Uri = "/v1/vpn/sessions".parse().expect("uri");
        let create_body = norito::json::to_vec(&VpnSessionCreateRequestDto {
            exit_class: "standard".to_owned(),
        })
        .expect("body");

        let first_headers = signed_app_headers(
            &first_account,
            &first_keys,
            &create_method,
            &create_uri,
            create_body.as_ref(),
        );
        let first_created = handle_create_vpn_session(
            app.clone(),
            &create_method,
            &create_uri,
            &first_headers,
            create_body.as_ref(),
        )
        .await
        .expect("first created")
        .into_response();
        let first_session: VpnSessionResponseDto = read_json(first_created).await;

        let second_headers = signed_app_headers(
            &second_account,
            &second_keys,
            &create_method,
            &create_uri,
            create_body.as_ref(),
        );
        let second_created = handle_create_vpn_session(
            app.clone(),
            &create_method,
            &create_uri,
            &second_headers,
            create_body.as_ref(),
        )
        .await
        .expect("second created")
        .into_response();
        let second_session: VpnSessionResponseDto = read_json(second_created).await;

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

        let third_headers = signed_app_headers(
            &third_account,
            &third_keys,
            &create_method,
            &create_uri,
            create_body.as_ref(),
        );
        let third_created = handle_create_vpn_session(
            app.clone(),
            &create_method,
            &create_uri,
            &third_headers,
            create_body.as_ref(),
        )
        .await
        .expect("third created")
        .into_response();
        let third_session: VpnSessionResponseDto = read_json(third_created).await;

        assert_ne!(
            third_session.tunnel_addresses,
            second_session.tunnel_addresses
        );
        assert_eq!(app.vpn_sessions.len(), 2);
    }
}
