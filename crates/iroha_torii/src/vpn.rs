use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use iroha_config::client_api::ConfigGetDTO;
use iroha_core::kiso::KisoHandle;
use iroha_data_model::{
    ValidationFail, account::AccountId, query::error::QueryExecutionFail,
    soranet::vpn::VpnExitClassV1,
};

use crate::{Error, SharedAppState};

const SUPPORTED_EXIT_CLASSES: [&str; 3] = ["standard", "low-latency", "high-security"];

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
    pub dns_servers: Vec<String>,
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
    pub meter_family: String,
    #[norito(default)]
    pub route_pushes: Vec<String>,
    #[norito(default)]
    pub dns_servers: Vec<String>,
    pub helper_ticket_hex: String,
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
pub struct VpnSessionDeleteResponseDto {
    pub session_id: String,
    pub status: String,
    pub disconnected_at_ms: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct VpnSessionRecord {
    pub session_id: String,
    pub account_id: AccountId,
    pub exit_class: String,
    pub relay_endpoint: String,
    pub lease_secs: u64,
    pub expires_at_ms: u64,
    pub meter_family: String,
    pub route_pushes: Vec<String>,
    pub dns_servers: Vec<String>,
    pub helper_ticket_hex: String,
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
        dns_servers: Vec::new(),
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

fn build_helper_ticket_hex(session_id: &str) -> String {
    hex::encode(blake3::hash(format!("{session_id}:helper").as_bytes()).as_bytes())
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
        let _ = app.vpn_sessions.remove(&session_id);
    }
}

fn remove_existing_sessions_for_account(app: &SharedAppState, account_id: &AccountId) {
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
        let _ = app.vpn_sessions.remove(&session_id);
    }
}

fn response_from_record(record: &VpnSessionRecord) -> VpnSessionResponseDto {
    VpnSessionResponseDto {
        session_id: record.session_id.clone(),
        account_id: record.account_id.to_string(),
        exit_class: record.exit_class.clone(),
        relay_endpoint: record.relay_endpoint.clone(),
        lease_secs: record.lease_secs,
        expires_at_ms: record.expires_at_ms,
        meter_family: record.meter_family.clone(),
        route_pushes: record.route_pushes.clone(),
        dns_servers: record.dns_servers.clone(),
        helper_ticket_hex: record.helper_ticket_hex.clone(),
        status: "active".to_owned(),
    }
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
    prune_expired_sessions(&app, current_ms);
    remove_existing_sessions_for_account(&app, &account_id);

    let nonce = headers
        .get(crate::HEADER_NONCE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("vpn");
    let session_id = build_session_id(&account_id, &exit_class, nonce, current_ms);
    let helper_ticket_hex = build_helper_ticket_hex(&session_id);
    let expires_at_ms = current_ms.saturating_add(profile.lease_secs.saturating_mul(1_000));
    let record = VpnSessionRecord {
        session_id: session_id.clone(),
        account_id,
        exit_class,
        relay_endpoint: profile.relay_endpoint,
        lease_secs: profile.lease_secs,
        expires_at_ms,
        meter_family: profile.meter_family,
        route_pushes: profile.route_pushes,
        dns_servers: profile.dns_servers,
        helper_ticket_hex,
    };
    let response = response_from_record(&record);
    app.vpn_sessions.insert(session_id, record);
    Ok((StatusCode::CREATED, crate::utils::JsonBody(response)).into_response())
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
    prune_expired_sessions(&app, current_ms);
    let normalized_session_id = session_id.trim();
    if normalized_session_id.is_empty() {
        return Err(conversion_error("session_id must not be empty"));
    }
    let Some(record) = app.vpn_sessions.get(normalized_session_id) else {
        return Ok((
            StatusCode::NOT_FOUND,
            crate::utils::JsonBody(VpnSessionDeleteResponseDto {
                session_id: normalized_session_id.to_owned(),
                status: "not_found".to_owned(),
                disconnected_at_ms: current_ms,
            }),
        )
            .into_response());
    };
    if record.account_id != account_id {
        return Err(not_permitted_error(
            "vpn session belongs to a different account",
        ));
    }
    drop(record);
    let _ = app.vpn_sessions.remove(normalized_session_id);
    Ok((
        StatusCode::OK,
        crate::utils::JsonBody(VpnSessionDeleteResponseDto {
            session_id: normalized_session_id.to_owned(),
            status: "disconnected".to_owned(),
            disconnected_at_ms: current_ms,
        }),
    )
        .into_response())
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
    async fn create_and_delete_vpn_session_roundtrips_for_signed_account() {
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
        assert_eq!(app.vpn_sessions.len(), 1);

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
        let deleted_body: VpnSessionDeleteResponseDto = read_json(deleted).await;
        assert_eq!(deleted_body.status, "disconnected");
        assert_eq!(app.vpn_sessions.len(), 0);
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
}
