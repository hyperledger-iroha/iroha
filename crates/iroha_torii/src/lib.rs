//! The web server of Iroha. `Torii` translates to gateway.
#![allow(unexpected_cfgs)]
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
// Temporarily relax a curated set of clippy lints while the Torii refactor is in flight.
// Trim this list further as the remaining call sites are cleaned up.
#![allow(
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::doc_markdown,
    clippy::explicit_iter_loop,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::used_underscore_binding,
    clippy::ptr_arg,
    clippy::match_same_arms,
    clippy::single_match_else,
    clippy::manual_midpoint,
    clippy::manual_map,
    clippy::needless_borrows_for_generic_args,
    clippy::io_other_error,
    clippy::default_trait_access,
    clippy::return_self_not_must_use,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_lossless,
    clippy::items_after_statements,
    clippy::unnecessary_map_or,
    clippy::map_unwrap_or,
    clippy::redundant_locals,
    clippy::clone_on_copy,
    clippy::unreadable_literal,
    clippy::if_not_else,
    clippy::nonminimal_bool,
    clippy::let_unit_value,
    clippy::ignored_unit_patterns,
    clippy::useless_format,
    clippy::to_string_in_format_args,
    clippy::bool_to_int_with_if,
    clippy::redundant_as_str,
    clippy::collapsible_if,
    clippy::drain_collect,
    clippy::iter_with_drain,
    clippy::len_zero,
    clippy::suboptimal_flops,
    clippy::needless_pass_by_value
)]
#![allow(dead_code, clippy::unused_async, unused_imports)]
//!
//! Crate features:
//! - `telemetry` (off by default): Status, Metrics, and API Version endpoints
//! - `schema` (off by default): Data Model Schema endpoint
//! - `app_api` (on by default): app-facing JSON endpoints (filters, webhooks)
//! - `transparent_api` (on by default): forwards data-model transparent API
//! - `p2p_ws` (off by default): exposes a `/p2p` WebSocket for P2P fallback
//! - `connect` (on by default): WalletConnect-style WS and minimal in-node relay
//! - `app_api_https` (off by default): enables HTTPS webhook delivery using rustls
//! - `app_api_wss` (off by default): enables WebSocket/WebSocket Secure webhook delivery
mod api_version;
mod operator_auth;
mod operator_signatures;
#[cfg(feature = "push")]
mod push;
/// Helpers for constructing Norito JSON values within Torii.
pub mod json_utils {
    use norito::json::{self, JsonSerialize, Value};

    fn log_failed(context: &str, err: &json::Error) {
        iroha_logger::error!(%context, ?err, "Torii JSON serialization failed");
    }

    /// Convert any [`JsonSerialize`] value into a [`Value`].
    #[inline]
    #[must_use]
    pub fn json_value<T: JsonSerialize + ?Sized>(value: &T) -> Value {
        match json::to_value(value) {
            Ok(val) => val,
            Err(err) => {
                log_failed("value", &err);
                Value::Null
            }
        }
    }

    /// Build a JSON array from an iterator of serializable values.
    #[inline]
    #[must_use]
    pub fn json_array<T, I>(values: I) -> Value
    where
        T: JsonSerialize,
        I: IntoIterator<Item = T>,
    {
        match json::array(values) {
            Ok(val) => val,
            Err(err) => {
                log_failed("array", &err);
                Value::Null
            }
        }
    }

    /// Key/value helper accepted by [`json_object`].
    pub trait JsonPair {
        /// Convert the pair into an owned `(String, Value)` tuple.
        fn into_pair(self) -> (String, Value);
    }

    impl<K, V> JsonPair for (K, V)
    where
        K: Into<String>,
        V: JsonSerialize,
    {
        fn into_pair(self) -> (String, Value) {
            let (key, value) = self;
            (key.into(), json_value(&value))
        }
    }

    /// Build a JSON object from key/value pairs.
    #[inline]
    #[must_use]
    pub fn json_object<P, I>(pairs: I) -> Value
    where
        P: JsonPair,
        I: IntoIterator<Item = P>,
    {
        let mapped = pairs.into_iter().map(JsonPair::into_pair);
        match json::object(mapped) {
            Ok(val) => val,
            Err(err) => {
                log_failed("object", &err);
                Value::Null
            }
        }
    }

    /// Convert a key and serializable value into a `(String, Value)` pair for [`json_object`].
    #[inline]
    #[must_use]
    /// Convenience helper that turns a key and serializable value into an entry for [`json_object`].
    pub fn json_entry<K, V>(key: K, value: V) -> (String, Value)
    where
        K: Into<String>,
        V: JsonSerialize,
    {
        (key.into(), json_value(&value))
    }
}

pub use json_utils::{json_array, json_entry, json_object, json_value};

pub use crate::app_auth::{
    HEADER_ACCOUNT, HEADER_SIGNATURE, Method, Uri, canonical_request_message,
    signature_header_value,
};

pub mod openapi;

mod content;
mod proof_filters;
use crate::api_version::ApiVersion;
pub mod sorafs;
use std::{
    collections::HashSet,
    convert::{Infallible, TryInto},
    fmt::Debug,
    fs,
    net::IpAddr,
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    body::{Body, Bytes},
    debug_handler,
    extract::{
        DefaultBodyLimit, Extension, State, WebSocketUpgrade,
        connect_info::IntoMakeServiceWithConnectInfo,
    },
    http::{HeaderMap, HeaderValue, Request, StatusCode, header::HeaderName},
    middleware::Next,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
};
#[allow(unused_imports)]
use base64::Engine;
use blake3::hash as blake3_hash;
use dashmap::DashMap;
use error_stack::{Report, ResultExt};
use iroha_config::{
    base::{WithOrigin, util::Bytes as ConfigBytes},
    client_api::ConfigUpdateDTO,
    parameters::actual::{NoritoRpcStage, NoritoRpcTransport, TelemetryProfile, Torii as Config},
};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::Telemetry;
use iroha_core::{
    EventsSender,
    alias::{AliasError, AliasMetricKind, AliasService},
    kiso::{Error as KisoError, KisoHandle},
    kura::Kura,
    prelude::*,
    query::store::LiveQueryStoreHandle,
    queue::{self, Queue},
    state::{
        BlockProofError, State as CoreState, StateReadOnly, StateReadOnlyWithTransactions,
        TransactionsReadOnly, WorldReadOnly,
    },
    sumeragi::rbc_store::SoftwareManifest,
};
use iroha_crypto::{
    ExposedPrivateKey, Hash, HashOf, KeyPair, SignatureOf,
    blake2::{Blake2b512, digest::Digest},
};
#[cfg(feature = "app_api")]
use iroha_data_model::alias::{AliasRecord, AliasTarget};
#[cfg(feature = "app_api")]
use iroha_data_model::events::{
    SharedDataEvent,
    data::{
        DataEvent,
        sorafs::{SorafsDealSettlement, SorafsDealUsage, SorafsGatewayEvent},
    },
};
#[cfg(feature = "app_api")]
use iroha_data_model::sorafs::capacity::ProviderId;
#[cfg(feature = "app_api")]
use iroha_data_model::sorafs::deal::DealUsageReport;
use iroha_data_model::{
    ChainId,
    account::{AccountAddress, AccountAddressFormat, AccountId},
    alias::AliasIndex,
    asset::{AssetDefinitionId, AssetId},
    block::{BlockHeader, proofs::BlockProofs},
    domain::DomainId,
    events::{
        EventBox,
        pipeline::{BlockStatus, PipelineEventBox, TransactionStatus},
    },
    name::Name,
    nft::NftId,
    peer::Peer,
    permission::Permission,
    transaction::{
        SignedTransaction, TransactionSubmissionReceipt, TransactionSubmissionReceiptPayload,
        signed::TransactionEntrypoint,
    },
};
use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
use iroha_futures::supervisor::ShutdownSignal;
use iroha_primitives::addr::SocketAddr;
use iroha_torii_shared::{ErrorEnvelope, QueueErrorEnvelope, QueueErrorSnapshot, uri};
use ivm::iso20022::{MsgError, parse_message};
use mv::storage::StorageReadOnly;
#[cfg(all(feature = "app_api", feature = "telemetry"))]
use norito::json::{self, Value};
use norito::json::{JsonDeserialize, JsonSerialize};
#[cfg(feature = "app_api")]
use sorafs_manifest::provider_advert::CapabilityType;
use sorafs_manifest::repair::{
    REPAIR_WORKER_SIGNATURE_VERSION_V1, RepairReportV1, RepairSlashProposalV1, RepairTicketId,
    RepairWorkerActionV1, RepairWorkerSignaturePayloadV1,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::{
    net::TcpListener,
    sync::{RwLock, watch},
};
use tower::ServiceExt as _;
use tower_http::{
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use utils::extractors::NoritoVersioned;

// Bring connect-info make service into scope for axum 0.8 serve path
use crate::iso20022_bridge::{Iso20022BridgeRuntime, IsoMessageState, Pacs002Status};
use crate::{router::builder::RouterBuilder, routing::conversion_error};

/// Norito JSON derive macros used across Torii modules.
pub mod json_macros {
    pub use norito::derive::{JsonDeserialize, JsonSerialize};
}

#[macro_use]
pub(crate) mod utils;
pub use utils::{
    JsonBody, NoritoBody, ResponseFormat,
    extractors::{JsonOnly, NoritoJson, NoritoJsonWithBytes, NoritoQuery},
};
#[cfg(feature = "app_api")]
mod address_format;
mod app_auth;
mod block;
#[cfg(feature = "connect")]
mod connect;
#[cfg(feature = "app_api")]
mod data_dir;
mod event;
#[cfg(feature = "app_api")]
pub mod explorer;
#[cfg(feature = "app_api")]
pub mod filter;
#[cfg(feature = "app_api")]
mod gov;
mod iso20022_bridge;
mod limits;
mod mcp;
#[cfg(feature = "tx_predicates")]
mod predicates;
mod router;
pub(crate) mod routing;
mod runtime;
#[cfg(feature = "app_api")]
mod sns;
#[cfg(feature = "app_api")]
mod soracloud;
#[cfg(all(feature = "app_api", feature = "telemetry"))]
mod telemetry;
#[cfg(feature = "app_api")]
pub mod test_utils;

// Re-export selected API for integration tests and external consumers
// Governance selection
#[cfg(feature = "app_api")]
pub use gov::{
    AtWindowDto, CouncilDeriveVrfRequest, EnactDto, InstancesByNamespaceResponse, InstancesQuery,
    LocksGetResponse, ProposalGetResponse, ProtectedNamespacesDto, ReferendumGetResponse,
    TallyGetResponse, handle_gov_council_current, handle_gov_enact, handle_gov_get_locks,
    handle_gov_get_proposal, handle_gov_get_referendum, handle_gov_get_tally,
    handle_gov_instances_by_ns, handle_gov_protected_get, handle_gov_protected_set,
    handle_gov_unlock_stats,
};
// Routing helpers used by tests
pub use routing::event::handle_events_stream;
// Additional public re-exports of app endpoints used by tests
pub use routing::event_to_json_value;
#[cfg(feature = "zk-proof-tags")]
pub use routing::handle_get_proof_tags;
#[cfg(feature = "p2p_ws")]
pub use routing::handle_p2p_ws;
pub use routing::{
    ActivateInstanceDto, ActivateInstanceResponseDto, ContractCallDto, ContractCallResponseDto,
    DeployAndActivateInstanceDto, DeployAndActivateInstanceResponseDto, DeployContractDto,
    DeployContractResponseDto, EvidenceListQuery, EvidenceSubmitRequestDto, KaigiRelayDetailDto,
    KaigiRelayDomainMetricsDto, KaigiRelayHealthSnapshotDto, KaigiRelaySummaryDto,
    KaigiRelaySummaryListDto, MaybeTelemetry, PinAliasDto, PinPolicyDto, PinPolicyStorageClassDto,
    ProofApiLimits, ProofFindByIdQueryDto, ProofListQuery, QueryOptions, RegisterPinManifestDto,
    RegisterPinManifestResponseDto, SpaceDirectoryManifestPublishDto,
    SpaceDirectoryManifestRevokeDto, VkListQuery, ZkRootsGetRequestDto, ZkVkRegisterDto,
    ZkVkUpdateDto, ZkVoteGetTallyRequestDto, handle_count_proofs, handle_get_contract_code_bytes,
    handle_get_proof, handle_get_vk, handle_list_proofs, handle_list_vk, handle_post_contract_call,
    handle_post_contract_deploy, handle_post_contract_instance,
    handle_post_contract_instance_activate, handle_post_sorafs_register_manifest,
    handle_post_space_directory_manifest_publish, handle_post_space_directory_manifest_revoke,
    handle_post_sumeragi_evidence_submit, handle_post_vk_register, handle_post_vk_update,
    handle_queries_with_opts as handle_queries, handle_queries_with_opts, handle_v1_events_sse,
    handle_v1_new_view_json, handle_v1_new_view_sse, handle_v1_sumeragi_evidence_count,
    handle_v1_sumeragi_evidence_list, handle_v1_sumeragi_vrf_penalties, handle_v1_zk_roots,
    handle_v1_zk_submit_proof, handle_v1_zk_verify, handle_v1_zk_vote_tally,
    signed_find_proof_by_id,
};
#[cfg(feature = "connect")]
pub use routing::{ConnectSessionRequest, ConnectSessionResponse, ConnectWsQuery};
#[cfg(feature = "telemetry")]
pub use routing::{
    RecordSoranetPrivacyEventDto, RecordSoranetPrivacyShareDto, handle_metrics, handle_status,
};
#[cfg(feature = "telemetry")]
pub use routing::{
    handle_post_soranet_privacy_event, handle_post_soranet_privacy_share,
    handle_v1_kaigi_relay_detail, handle_v1_kaigi_relays, handle_v1_kaigi_relays_health,
    handle_v1_kaigi_relays_sse, handle_v1_sumeragi_collectors, handle_v1_sumeragi_commit_qc,
    handle_v1_sumeragi_leader, handle_v1_sumeragi_pacemaker, handle_v1_sumeragi_params,
    handle_v1_sumeragi_phases, handle_v1_sumeragi_qc, handle_v1_sumeragi_rbc_delivered_height_view,
    handle_v1_sumeragi_rbc_sessions, handle_v1_sumeragi_rbc_status, handle_v1_sumeragi_status,
    handle_v1_sumeragi_status_sse,
};
pub use runtime::{
    ActivateCancelResponse, handle_runtime_activate_upgrade, handle_runtime_cancel_upgrade,
    handle_runtime_upgrades_list,
};

// Shared app state for handlers to avoid large inline closures that break axum Handler bounds
#[derive(Clone)]
struct GatewayFixtureTelemetry {
    version: String,
    profile_version: String,
    fixtures_digest: String,
    released_at_unix: u64,
}

fn sorafs_gateway_fixture_telemetry() -> GatewayFixtureTelemetry {
    let metadata = sorafs_manifest::gateway_fixture_metadata();
    GatewayFixtureTelemetry {
        version: metadata.version.to_string(),
        profile_version: metadata.profile_version.to_string(),
        fixtures_digest: metadata.digest_hex().to_string(),
        released_at_unix: metadata.released_at_unix,
    }
}

fn alias_service_from_iso_config(
    config: &iroha_config::parameters::actual::IsoBridge,
) -> Option<Arc<AliasService>> {
    if !config.enabled {
        return None;
    }

    let service = AliasService::new();
    let mut inserted = 0usize;

    for (index, alias_cfg) in config.account_aliases.iter().enumerate() {
        let canonical = normalise_alias(&alias_cfg.iban);
        if canonical.is_empty() {
            iroha_logger::warn!(
                iban = %alias_cfg.iban,
                "ISO bridge alias produced empty canonical representation"
            );
            continue;
        }

        let alias_name = match Name::from_str(&canonical) {
            Ok(name) => name,
            Err(err) => {
                iroha_logger::warn!(
                    iban = %alias_cfg.iban,
                    %err,
                    "ISO bridge alias is not a valid Name"
                );
                continue;
            }
        };

        let account_id: AccountId = match alias_cfg.account_id.parse() {
            Ok(id) => id,
            Err(err) => {
                iroha_logger::warn!(
                    iban = %alias_cfg.iban,
                    account = %alias_cfg.account_id,
                    %err,
                    "ISO bridge alias refers to an invalid account identifier"
                );
                continue;
            }
        };

        let record = AliasRecord::new(
            alias_name.clone(),
            account_id.clone(),
            AliasTarget::Account(account_id),
            AliasIndex(index as u64),
        );

        match service.storage().put(record) {
            Ok(_) => inserted += 1,
            Err(err) => {
                iroha_logger::warn!(
                    iban = %alias_cfg.iban,
                    ?err,
                    "failed to insert ISO bridge alias into alias service storage"
                );
            }
        }
    }

    if inserted == 0 {
        None
    } else {
        Some(Arc::new(service))
    }
}

fn install_account_resolvers(state: &Arc<CoreState>) {
    use iroha_data_model::{
        account::{
            AccountLabel, clear_account_alias_resolver, clear_account_domain_selector_resolver,
            clear_account_opaque_resolver, clear_account_uaid_resolver, set_account_alias_resolver,
            set_account_domain_selector_resolver, set_account_opaque_resolver,
            set_account_uaid_resolver,
        },
        name::Name,
    };

    clear_account_alias_resolver();
    clear_account_domain_selector_resolver();
    clear_account_opaque_resolver();
    clear_account_uaid_resolver();

    let alias_state = Arc::clone(state);
    set_account_alias_resolver(Arc::new(move |label, domain| {
        let name = Name::from_str(label).ok()?;
        let key = AccountLabel::new(domain.clone(), name);
        let world = alias_state.world_view();
        world.account_aliases().get(&key).cloned()
    }));

    let selector_state = Arc::clone(state);
    set_account_domain_selector_resolver(Arc::new(move |selector| {
        let world = selector_state.world_view();
        world.domain_selectors().get(selector).cloned()
    }));

    let uaid_state = Arc::clone(state);
    set_account_uaid_resolver(Arc::new(move |uaid| {
        let world = uaid_state.world_view();
        world.uaid_accounts().get(uaid).cloned()
    }));

    let opaque_state = Arc::clone(state);
    set_account_opaque_resolver(Arc::new(move |opaque| {
        let world = opaque_state.world_view();
        world.opaque_uaids().get(opaque).copied()
    }));
}

#[cfg(test)]
pub(crate) fn ensure_test_domain_selector_resolver() {
    use std::cell::Cell;

    use iroha_data_model::{
        account::{
            AccountDomainSelector, account_domain_selector_resolver,
            set_account_domain_selector_resolver,
        },
        domain::DomainId,
    };

    thread_local! {
        static RESOLVER_INSTALLED: Cell<bool> = const { Cell::new(false) };
    }

    RESOLVER_INSTALLED.with(|flag| {
        let existing = account_domain_selector_resolver();
        if flag.get() && existing.is_some() {
            return;
        }

        let resolver = std::sync::Arc::new(move |selector: &AccountDomainSelector| {
            if let Some(ref resolver) = existing {
                if let Some(domain) = resolver(selector) {
                    return Some(domain);
                }
            }

            let domains = [
                "wonderland",
                "garden_of_live_flowers",
                "treasury",
                "sora",
                "soranet",
                "default",
                "iroha",
                "alpha",
                "omega",
                "governance",
                "validators",
                "explorer",
                "kitsune",
                "da",
                "council",
                "genesis",
                "test",
                "payload",
                "index_test",
                "dummy",
                "custom",
                "sns",
            ];

            domains.iter().find_map(|label| {
                let domain: DomainId = (*label).parse().ok()?;
                let candidate = AccountDomainSelector::from_domain(&domain).ok()?;
                if &candidate == selector {
                    Some(domain)
                } else {
                    None
                }
            })
        });

        set_account_domain_selector_resolver(resolver);
        flag.set(true);
    });
}

const ALIAS_METRIC_LANE: &str = "torii";

fn alias_json_response<T>(status: StatusCode, payload: T) -> Result<AxResponse, Error>
where
    T: JsonSerialize,
{
    let body = norito::json::to_vec(&payload).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            err.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

fn json_ok<T>(payload: T) -> Result<AxResponse, Error>
where
    T: JsonSerialize,
{
    alias_json_response(StatusCode::OK, payload)
}

fn alias_resolve_ok(
    alias: &str,
    account_id: &str,
    index: Option<u64>,
    source: &'static str,
) -> Result<AxResponse, Error> {
    let payload = routing::AliasResolveResponseDto {
        alias: alias.to_owned(),
        account_id: account_id.to_owned(),
        index,
        source: Some(source.to_owned()),
    };
    alias_json_response(StatusCode::OK, payload)
}

fn alias_resolve_index_ok(
    index: u64,
    alias: &str,
    account_id: &str,
    source: &'static str,
) -> Result<AxResponse, Error> {
    let payload = routing::AliasResolveIndexResponseDto {
        index,
        alias: alias.to_owned(),
        account_id: account_id.to_owned(),
        source: Some(source.to_owned()),
    };
    alias_json_response(StatusCode::OK, payload)
}

fn alias_error_response(status: StatusCode, message: &str) -> Result<AxResponse, Error> {
    let payload = routing::AliasErrorResponseDto {
        error: message.to_owned(),
    };
    alias_json_response(status, payload)
}

fn map_alias_error(err: AliasError) -> Error {
    Error::Query(iroha_data_model::ValidationFail::InternalError(
        err.to_string(),
    ))
}

fn resolve_alias_via_service(
    service: &AliasService,
    alias_input: &str,
) -> Result<AxResponse, Error> {
    let canonical = normalise_alias(alias_input);
    if canonical.is_empty() {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "alias must not be empty".to_string(),
            ),
        )));
    }

    let alias_name = Name::from_str(&canonical).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(err.to_string()),
        ))
    })?;

    let storage = service.storage();
    match storage.resolve(&alias_name) {
        Ok(Some(record)) => {
            storage.emit_metrics(&alias_name, ALIAS_METRIC_LANE, AliasMetricKind::Resolve);
            match &record.target {
                AliasTarget::Account(account_id) => {
                    let account_id_string = account_id.to_string();
                    alias_resolve_ok(
                        record.alias.as_ref(),
                        &account_id_string,
                        Some(record.index.0),
                        "alias_service",
                    )
                }
                _ => alias_error_response(
                    StatusCode::NOT_IMPLEMENTED,
                    "alias targets other than accounts are not supported yet",
                ),
            }
        }
        Ok(None) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(err) => Err(map_alias_error(err)),
    }
}

fn resolve_alias_index_via_service(
    service: &AliasService,
    index: u64,
) -> Result<AxResponse, Error> {
    let storage = service.storage();
    match storage.resolve_index(AliasIndex(index)) {
        Ok(Some(record)) => {
            storage.emit_metrics(&record.alias, ALIAS_METRIC_LANE, AliasMetricKind::Resolve);
            match &record.target {
                AliasTarget::Account(account_id) => {
                    let account_id_string = account_id.to_string();
                    alias_resolve_index_ok(
                        record.index.0,
                        record.alias.as_ref(),
                        &account_id_string,
                        "alias_service",
                    )
                }
                _ => alias_error_response(
                    StatusCode::NOT_IMPLEMENTED,
                    "alias targets other than accounts are not supported yet",
                ),
            }
        }
        Ok(None) => Ok(StatusCode::NOT_FOUND.into_response()),
        Err(err) => Err(map_alias_error(err)),
    }
}

#[allow(clippy::struct_excessive_bools)]
struct AppState {
    events: EventsSender,
    kura: Arc<Kura>,
    chain_id: Arc<ChainId>,
    state: Arc<CoreState>,
    kiso: KisoHandle,
    query_service: LiveQueryStoreHandle,
    rate_limiter: limits::RateLimiter,
    tx_rate_limiter: limits::RateLimiter,
    deploy_rate_limiter: limits::RateLimiter,
    proof_rate_limiter: limits::RateLimiter,
    proof_egress_limiter: limits::RateLimiter,
    content_request_limiter: limits::RateLimiter,
    content_egress_limiter: limits::RateLimiter,
    proof_limits: routing::ProofApiLimits,
    content_config: iroha_config::parameters::actual::Content,
    ws_message_timeout: Duration,
    require_api_token: bool,
    api_tokens_set: Arc<HashSet<String>>,
    operator_auth: Arc<operator_auth::OperatorAuth>,
    operator_signatures: Arc<operator_signatures::OperatorSignatures>,
    soranet_privacy_ingest: iroha_config::parameters::actual::SoranetPrivacyIngest,
    soranet_privacy_tokens: Arc<HashSet<String>>,
    soranet_privacy_allow_nets: Arc<Vec<limits::IpNet>>,
    soranet_privacy_rate_limiter: limits::RateLimiter,
    allow_nets: Arc<Vec<limits::IpNet>>,
    preauth_gate: Arc<limits::PreAuthGate>,
    queue: Arc<Queue>,
    pipeline_status_cache: Arc<PipelineStatusCache>,
    high_load_tx_threshold: usize,
    high_load_stream_tx_threshold: usize,
    high_load_subscription_tx_threshold: usize,
    mcp: iroha_config::parameters::actual::ToriiMcp,
    mcp_rate_limiter: limits::RateLimiter,
    mcp_tools: Arc<Vec<mcp::ToolSpec>>,
    mcp_dispatch_router: std::sync::RwLock<Option<axum::Router<std::sync::Arc<AppState>>>>,
    fee_policy: FeePolicy,
    norito_rpc: iroha_config::parameters::actual::NoritoRpcTransport,
    online_peers: OnlinePeersProvider,
    iso_bridge: Option<Arc<Iso20022BridgeRuntime>>,
    alias_service: Option<Arc<AliasService>>,
    telemetry: routing::MaybeTelemetry,
    telemetry_profile: TelemetryProfile,
    api_versions: api_version::ApiVersionPolicy,
    zk_prover_keys_dir: PathBuf,
    zk_ivm_prove_jobs: Arc<DashMap<String, ZkIvmProveJobState>>,
    zk_ivm_prove_inflight: Arc<tokio::sync::Semaphore>,
    zk_ivm_prove_slots: Arc<tokio::sync::Semaphore>,
    zk_ivm_prove_slots_total: usize,
    zk_ivm_prove_inflight_total: usize,
    zk_ivm_prove_job_ttl_ms: u64,
    zk_ivm_prove_job_max_entries: usize,
    #[cfg(all(feature = "app_api", feature = "telemetry"))]
    peer_telemetry: Arc<telemetry::peers::PeerTelemetryService>,
    rbc_sampling_enabled: bool,
    rbc_sampling_store_dir: Option<PathBuf>,
    rbc_sampling_max_samples: u32,
    rbc_sampling_max_bytes: u64,
    rbc_sampling_daily_budget: u64,
    rbc_sampling_limiter: limits::RateLimiter,
    rbc_sampling_budget: DashMap<String, SamplingBudgetEntry>,
    rbc_sampling_manifest: SoftwareManifest,
    rbc_chain_hash: Hash,
    da_replay_cache: Arc<iroha_core::da::ReplayCache>,
    da_replay_store: Arc<da::ReplayCursorStore>,
    da_receipt_log: Arc<da::DaReceiptLog>,
    da_receipt_signer: KeyPair,
    da_ingest: iroha_config::parameters::actual::DaIngest,
    sumeragi: Option<iroha_core::sumeragi::SumeragiHandle>,
    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    p2p: Option<iroha_core::IrohaNetwork>,
    #[cfg(feature = "connect")]
    connect_bus: connect::Bus,
    #[cfg(feature = "connect")]
    connect_enabled: bool,
    #[cfg(feature = "push")]
    push: Option<push::PushBridge>,
    #[cfg(feature = "push")]
    push_rate_limiter: limits::RateLimiter,
    #[cfg(feature = "app_api")]
    sorafs_cache: Option<Arc<RwLock<sorafs::ProviderAdvertCache>>>,
    #[cfg(feature = "app_api")]
    sorafs_node: sorafs_node::NodeHandle,
    #[cfg(feature = "app_api")]
    sorafs_limits: Arc<sorafs::SorafsQuotaEnforcer>,
    #[cfg(feature = "app_api")]
    por_coordinator: Arc<sorafs::PorCoordinator>,
    #[cfg(feature = "app_api")]
    sorafs_alias_cache_policy: sorafs::AliasCachePolicy,
    #[cfg(feature = "app_api")]
    sorafs_alias_enforcement: sorafs::AliasCacheEnforcement,
    #[cfg(feature = "app_api")]
    sorafs_admission: Option<Arc<sorafs::AdmissionRegistry>>,
    #[cfg(feature = "app_api")]
    sorafs_gateway_config: iroha_config::parameters::actual::SorafsGateway,
    #[cfg(feature = "app_api")]
    sorafs_gateway_policy: Option<Arc<sorafs::gateway::GatewayPolicy>>,
    #[cfg(feature = "app_api")]
    sorafs_gateway_denylist: Option<Arc<sorafs::gateway::GatewayDenylist>>,
    #[cfg(feature = "app_api")]
    sorafs_gateway_tls_state: Option<Arc<RwLock<sorafs::gateway::TlsStateSnapshot>>>,
    #[cfg(feature = "app_api")]
    sorafs_pin_policy: sorafs::PinSubmissionPolicy,
    #[cfg(feature = "app_api")]
    sorafs_blinded_resolver: Option<Arc<sorafs::BlindedCidResolver>>,
    #[cfg(feature = "app_api")]
    stream_token_issuer: Option<Arc<sorafs::StreamTokenIssuer>>,
    #[cfg(feature = "app_api")]
    stream_token_concurrency: sorafs::StreamTokenConcurrencyTracker,
    #[cfg(feature = "app_api")]
    stream_token_quota: sorafs::StreamTokenQuotaTracker,
    #[cfg(feature = "app_api")]
    sorafs_chunk_range_overrides: DashMap<[u8; 32], bool>,
    #[cfg(feature = "app_api")]
    offline_issuer: Option<OfflineIssuerSigner>,
    #[cfg(feature = "app_api")]
    uaid_onboarding: Option<AccountOnboardingSigner>,
    #[cfg(feature = "app_api")]
    sns_registry: Arc<sns::Registry>,
    #[cfg(feature = "app_api")]
    soracloud_registry: Arc<soracloud::Registry>,
}

pub(crate) type SharedAppState = std::sync::Arc<AppState>;

#[derive(Clone)]
struct SamplingBudgetEntry {
    bytes_served: u64,
    window_start: Instant,
}

fn json_string_or_null(opt: Option<String>) -> norito::json::native::Value {
    opt.map(norito::json::native::Value::from)
        .unwrap_or(norito::json::native::Value::Null)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PipelineStatusKind {
    Queued,
    Approved,
    Committed,
    Applied,
    Rejected,
    Expired,
}

const PIPELINE_STATUS_CACHE_CAP: usize = 100_000;
const PIPELINE_STATUS_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
const PIPELINE_STATUS_CACHE_PRUNE_INTERVAL_SECS: u64 = 30;

impl PipelineStatusKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Approved => "Approved",
            Self::Committed => "Committed",
            Self::Applied => "Applied",
            Self::Rejected => "Rejected",
            Self::Expired => "Expired",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Queued => 0,
            Self::Approved => 1,
            Self::Expired => 2,
            Self::Committed => 3,
            Self::Applied => 4,
            Self::Rejected => 5,
        }
    }
}

#[derive(Clone, Debug)]
struct PipelineStatusEntry {
    kind: PipelineStatusKind,
    block_height: Option<NonZeroU64>,
    rejection: Option<iroha_data_model::transaction::error::TransactionRejectionReason>,
    observed_at: Instant,
}

impl PipelineStatusEntry {
    fn fresh(
        kind: PipelineStatusKind,
        block_height: Option<NonZeroU64>,
        rejection: Option<iroha_data_model::transaction::error::TransactionRejectionReason>,
    ) -> Self {
        Self::at_time(kind, block_height, rejection, Instant::now())
    }

    fn at_time(
        kind: PipelineStatusKind,
        block_height: Option<NonZeroU64>,
        rejection: Option<iroha_data_model::transaction::error::TransactionRejectionReason>,
        observed_at: Instant,
    ) -> Self {
        Self {
            kind,
            block_height,
            rejection,
            observed_at,
        }
    }

    fn merge_from_event(&mut self, incoming: PipelineStatusEntry) {
        let incoming_observed_at = incoming.observed_at;
        match incoming.kind.rank().cmp(&self.kind.rank()) {
            std::cmp::Ordering::Greater => {
                *self = incoming;
                return;
            }
            std::cmp::Ordering::Equal => {
                if self.block_height.is_none() {
                    self.block_height = incoming.block_height;
                }
                if self.rejection.is_none() && incoming.rejection.is_some() {
                    self.rejection = incoming.rejection;
                }
            }
            std::cmp::Ordering::Less => {}
        }
        if incoming_observed_at > self.observed_at {
            self.observed_at = incoming_observed_at;
        }
    }
}

#[derive(Clone, Debug)]
struct PendingBlockStatus {
    kind: PipelineStatusKind,
    block_hash: HashOf<BlockHeader>,
    observed_at: Instant,
}

#[derive(Debug)]
struct PipelineStatusCache {
    entries: DashMap<HashOf<SignedTransaction>, PipelineStatusEntry>,
    pending_blocks: DashMap<NonZeroU64, PendingBlockStatus>,
    capacity: usize,
    ttl: Duration,
    start: Instant,
    last_prune_secs: std::sync::atomic::AtomicU64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockRecordOutcome {
    Recorded,
    MissingBlock,
    HashMismatch,
}

impl PipelineStatusCache {
    fn new() -> Self {
        Self::with_limits(PIPELINE_STATUS_CACHE_CAP, PIPELINE_STATUS_CACHE_TTL)
    }

    fn with_limits(capacity: usize, ttl: Duration) -> Self {
        let cap = capacity.max(1);
        Self {
            entries: DashMap::new(),
            pending_blocks: DashMap::new(),
            capacity: cap,
            ttl,
            start: Instant::now(),
            last_prune_secs: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn record_transaction_event(
        &self,
        event: &iroha_data_model::events::pipeline::TransactionEvent,
    ) {
        let (kind, rejection) = match event.status() {
            TransactionStatus::Queued => (PipelineStatusKind::Queued, None),
            TransactionStatus::Expired => (PipelineStatusKind::Expired, None),
            TransactionStatus::Approved => (PipelineStatusKind::Approved, None),
            TransactionStatus::Rejected(reason) => {
                (PipelineStatusKind::Rejected, Some((**reason).clone()))
            }
        };
        let incoming = PipelineStatusEntry::fresh(kind, event.block_height(), rejection);
        self.entries
            .entry(*event.hash())
            .and_modify(|entry| entry.merge_from_event(incoming.clone()))
            .or_insert(incoming);
        self.prune_if_needed(Instant::now());
    }

    fn record_block_event(
        &self,
        event: &iroha_data_model::events::pipeline::BlockEvent,
        kura: &Kura,
    ) {
        let kind = match event.status {
            BlockStatus::Committed => PipelineStatusKind::Committed,
            BlockStatus::Applied => PipelineStatusKind::Applied,
            _ => return,
        };
        let height = event.header.height();
        let block_hash = event.header.hash();
        let now = Instant::now();
        match self.record_block_results(height, block_hash, kind, kura, now) {
            BlockRecordOutcome::Recorded => {
                self.pending_blocks.remove(&height);
                self.prune_if_needed(now);
            }
            BlockRecordOutcome::MissingBlock => {
                self.pending_blocks.insert(
                    height,
                    PendingBlockStatus {
                        kind,
                        block_hash,
                        observed_at: now,
                    },
                );
                self.prune_if_needed(now);
            }
            BlockRecordOutcome::HashMismatch => {}
        }
    }

    fn lookup(&self, hash: &HashOf<SignedTransaction>) -> Option<PipelineStatusEntry> {
        self.entries.get(hash).map(|entry| entry.clone())
    }

    fn record_entry(&self, hash: HashOf<SignedTransaction>, entry: PipelineStatusEntry) {
        self.entries
            .entry(hash)
            .and_modify(|current| current.merge_from_event(entry.clone()))
            .or_insert(entry);
        self.prune_if_needed(Instant::now());
    }

    fn refresh_pending_blocks(&self, kura: &Kura) {
        if self.pending_blocks.is_empty() {
            return;
        }
        let now = Instant::now();
        let pending: Vec<_> = self
            .pending_blocks
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect();
        for (height, pending) in pending {
            match self.record_block_results(height, pending.block_hash, pending.kind, kura, now) {
                BlockRecordOutcome::Recorded | BlockRecordOutcome::HashMismatch => {
                    self.pending_blocks.remove(&height);
                }
                BlockRecordOutcome::MissingBlock => {}
            }
        }
        self.prune_if_needed(now);
    }

    fn prune_if_needed(&self, now: Instant) {
        let entries_len = self.entries.len();
        let pending_len = self.pending_blocks.len();
        let elapsed_secs = now.saturating_duration_since(self.start).as_secs().max(1);
        let last_prune = self
            .last_prune_secs
            .load(std::sync::atomic::Ordering::Relaxed);
        let prune_due =
            elapsed_secs.saturating_sub(last_prune) >= PIPELINE_STATUS_CACHE_PRUNE_INTERVAL_SECS;
        let over_cap = entries_len > self.capacity || pending_len > self.capacity;
        if !over_cap && !prune_due {
            return;
        }
        self.last_prune_secs
            .store(elapsed_secs, std::sync::atomic::Ordering::Relaxed);
        self.prune(now);
    }

    fn prune(&self, now: Instant) {
        if !self.ttl.is_zero() {
            let ttl = self.ttl;
            let stale_keys: Vec<_> = self
                .entries
                .iter()
                .filter_map(|entry| {
                    let age = now.saturating_duration_since(entry.observed_at);
                    (age > ttl).then_some(*entry.key())
                })
                .collect();
            for key in stale_keys {
                self.entries.remove(&key);
            }

            let stale_pending: Vec<_> = self
                .pending_blocks
                .iter()
                .filter_map(|entry| {
                    let age = now.saturating_duration_since(entry.observed_at);
                    (age > ttl).then_some(*entry.key())
                })
                .collect();
            for key in stale_pending {
                self.pending_blocks.remove(&key);
            }
        }

        self.evict_over_capacity();
    }

    fn evict_over_capacity(&self) {
        let len = self.entries.len();
        if len <= self.capacity {
            return;
        }
        let mut ordered: Vec<_> = self
            .entries
            .iter()
            .map(|entry| (*entry.key(), entry.observed_at))
            .collect();
        ordered.sort_by_key(|(_, observed_at)| *observed_at);
        let excess = len - self.capacity;
        for (hash, _) in ordered.into_iter().take(excess) {
            self.entries.remove(&hash);
        }

        let pending_len = self.pending_blocks.len();
        if pending_len <= self.capacity {
            return;
        }
        let mut pending_ordered: Vec<_> = self
            .pending_blocks
            .iter()
            .map(|entry| (*entry.key(), entry.observed_at))
            .collect();
        pending_ordered.sort_by_key(|(_, observed_at)| *observed_at);
        let excess = pending_len - self.capacity;
        for (height, _) in pending_ordered.into_iter().take(excess) {
            self.pending_blocks.remove(&height);
        }
    }

    fn record_block_results(
        &self,
        height: NonZeroU64,
        expected_hash: HashOf<BlockHeader>,
        kind: PipelineStatusKind,
        kura: &Kura,
        now: Instant,
    ) -> BlockRecordOutcome {
        let height_usize = match usize::try_from(height.get()) {
            Ok(value) => value,
            Err(_) => {
                iroha_logger::debug!(
                    height = height.get(),
                    "pipeline status cache skipped block: height exceeds usize"
                );
                return BlockRecordOutcome::MissingBlock;
            }
        };
        let Some(height_nz) = NonZeroUsize::new(height_usize) else {
            return BlockRecordOutcome::MissingBlock;
        };
        let Some(block) = kura.get_block(height_nz) else {
            iroha_logger::debug!(
                height = height.get(),
                "pipeline status cache skipped block: block not in kura"
            );
            return BlockRecordOutcome::MissingBlock;
        };
        let block_ref = block.as_ref();
        if block_ref.hash() != expected_hash {
            iroha_logger::debug!(
                height = height.get(),
                "pipeline status cache skipped block: hash mismatch"
            );
            return BlockRecordOutcome::HashMismatch;
        }
        let external_total = block_ref.external_transactions().len();
        for (tx, result) in block_ref
            .external_transactions()
            .zip(block_ref.results().take(external_total))
        {
            let (entry_kind, rejection) = match &result.0 {
                Ok(_) => (kind, None),
                Err(reason) => (PipelineStatusKind::Rejected, Some(reason.clone())),
            };
            let incoming = PipelineStatusEntry::at_time(entry_kind, Some(height), rejection, now);
            self.entries
                .entry(tx.hash())
                .and_modify(|entry| entry.merge_from_event(incoming.clone()))
                .or_insert(incoming);
        }
        BlockRecordOutcome::Recorded
    }
}

fn telemetry_unavailable_response(
    endpoint: &'static str,
    telemetry: &routing::MaybeTelemetry,
) -> axum::response::Response {
    #[cfg(feature = "telemetry")]
    {
        Error::telemetry_profile_forbidden(endpoint, telemetry.profile()).into_response()
    }

    #[cfg(not(feature = "telemetry"))]
    {
        let _ = endpoint;
        let _ = telemetry;
        axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
    }
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
fn collect_peer_urls(
    _online_peers: &OnlinePeersProvider,
    configured: &[telemetry::peers::ToriiUrl],
) -> Vec<telemetry::peers::ToriiUrl> {
    if configured.is_empty() {
        // Avoid probing P2P ports; peer telemetry requires explicit Torii URLs.
        iroha_logger::debug!("peer telemetry disabled: no peer_telemetry_urls configured");
        return Vec::new();
    }

    let mut urls = configured.to_vec();
    urls.sort();
    urls.dedup();
    urls
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConnScheme {
    Http,
    Ws,
    NoritoRpc,
}

impl ConnScheme {
    fn from_request<B>(req: &axum::http::Request<B>) -> Self {
        if req
            .headers()
            .get(axum::http::header::UPGRADE)
            .and_then(|value| value.to_str().ok())
            .map_or(false, |v| v.eq_ignore_ascii_case("websocket"))
        {
            Self::Ws
        } else if matches!(
            req.uri().path(),
            iroha_torii_shared::uri::TRANSACTION | iroha_torii_shared::uri::QUERY
        ) || req
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map_or(false, |ct| ct.contains(crate::utils::NORITO_MIME_TYPE))
        {
            Self::NoritoRpc
        } else {
            Self::Http
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Ws => "ws",
            Self::NoritoRpc => "norito_rpc",
        }
    }
}

struct PreAuthRequestGuard {
    permit: limits::PreAuthPermit,
    telemetry: routing::MaybeTelemetry,
    scheme: ConnScheme,
}

impl Drop for PreAuthRequestGuard {
    fn drop(&mut self) {
        let scheme = self.scheme;
        self.telemetry
            .with_metrics(|telemetry| telemetry.dec_torii_active_conn(scheme.label()));
    }
}

impl AppState {
    async fn acquire_preauth(
        &self,
        ip: Option<IpAddr>,
        scheme: ConnScheme,
    ) -> Result<PreAuthRequestGuard, limits::RejectReason> {
        let permit = self.preauth_gate.acquire(ip, Some(scheme.label())).await?;
        let telemetry = self.telemetry.clone();
        telemetry.with_metrics(|telemetry| telemetry.inc_torii_active_conn(scheme.label()));
        Ok(PreAuthRequestGuard {
            permit,
            telemetry,
            scheme,
        })
    }

    fn record_preauth_reject(&self, reason: limits::RejectReason) {
        self.telemetry
            .with_metrics(|telemetry| telemetry.inc_torii_pre_auth_reject(reason.metric_label()));
    }

    fn record_norito_rpc_gate(&self, outcome: &'static str) {
        let stage = self.norito_rpc.stage.label();
        self.telemetry
            .with_metrics(|telemetry| telemetry.inc_torii_norito_rpc_gate(stage, outcome));
    }

    fn norito_rpc_config(&self) -> &NoritoRpcTransport {
        &self.norito_rpc
    }

    /// Returns true when API tokens are required and at least one token is configured.
    fn api_token_enforced(&self) -> bool {
        self.require_api_token && !self.api_tokens_set.is_empty()
    }

    fn check_norito_rpc_allowed(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<(), Box<axum::response::Response>> {
        let provided_token = norito_rpc_token_from_headers(headers);
        match evaluate_norito_rpc_gate(self.norito_rpc_config(), headers) {
            Ok(()) => {
                self.record_norito_rpc_gate("allowed");
                Ok(())
            }
            Err(failure) => {
                let outcome = match failure {
                    NoritoRpcGateFailure::Disabled => "disabled",
                    NoritoRpcGateFailure::CanaryDenied => {
                        if provided_token.is_some() {
                            "canary_denied"
                        } else {
                            "canary_missing_token"
                        }
                    }
                    NoritoRpcGateFailure::MtlsRequired => "mtls_required",
                };
                self.record_norito_rpc_gate(outcome);
                Err(Box::new(norito_rpc_error_response(failure)))
            }
        }
    }

    fn rpc_capabilities(&self) -> RpcCapabilitiesResponse {
        RpcCapabilitiesResponse {
            norito_rpc: RpcNoritoRpcCapability::from(self.norito_rpc_config()),
        }
    }

    fn rpc_ping(&self) -> RpcPingResponse {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        RpcPingResponse {
            ok: true,
            unix_time_ms: timestamp_ms,
            norito_rpc: RpcNoritoRpcCapability::from(self.norito_rpc_config()),
        }
    }

    /// Clone the telemetry handle for downstream helpers.
    pub(crate) fn telemetry_handle(&self) -> routing::MaybeTelemetry {
        self.telemetry.clone()
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn stream_token_issuer(&self) -> Option<Arc<sorafs::StreamTokenIssuer>> {
        self.stream_token_issuer.clone()
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn stream_token_concurrency(&self) -> &sorafs::StreamTokenConcurrencyTracker {
        &self.stream_token_concurrency
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn stream_token_quota(&self) -> &sorafs::StreamTokenQuotaTracker {
        &self.stream_token_quota
    }

    #[cfg(feature = "app_api")]
    fn blinded_resolver(&self) -> Option<&Arc<sorafs::BlindedCidResolver>> {
        self.sorafs_blinded_resolver.as_ref()
    }

    #[cfg(feature = "app_api")]
    #[allow(clippy::result_large_err)]
    fn evaluate_gateway_policy(
        &self,
        ctx: sorafs::gateway::RequestContext<'_>,
    ) -> Result<(), sorafs::gateway::PolicyViolation> {
        let Some(policy) = &self.sorafs_gateway_policy else {
            return Ok(());
        };
        match policy.evaluate(&ctx) {
            sorafs::gateway::PolicyDecision::Allow => Ok(()),
            sorafs::gateway::PolicyDecision::Deny(violation) => {
                #[cfg(feature = "telemetry")]
                {
                    let (reason, detail) = violation.telemetry_labels();
                    self.telemetry.with_metrics(|metrics| {
                        metrics.record_sorafs_gar_violation(reason, detail);
                    });
                }
                self.publish_gar_violation_event(&ctx, &violation);
                Err(violation)
            }
        }
    }

    #[cfg(feature = "app_api")]
    fn publish_gar_violation_event(
        &self,
        ctx: &sorafs::gateway::RequestContext<'_>,
        violation: &sorafs::gateway::PolicyViolation,
    ) {
        let payload = sorafs::gateway::build_gar_violation_event(ctx, violation);
        let data_event = DataEvent::Sorafs(SorafsGatewayEvent::GarViolation(payload));
        let event = SharedDataEvent::from(data_event);
        if let Err(err) = self.events.send(EventBox::Data(event)) {
            iroha_logger::warn!(?err, "failed to broadcast SoraFS GAR violation event");
        }
    }

    #[cfg(feature = "app_api")]
    fn publish_deal_usage_event(
        &self,
        report: &DealUsageReport,
        outcome: &sorafs_node::UsageOutcome,
    ) {
        use std::convert::TryFrom;

        let payload = SorafsDealUsage {
            deal_id: report.deal_id,
            provider_id: outcome.provider_id,
            client_id: outcome.client_id,
            epoch: report.epoch,
            storage_gib_hours: report.storage_gib_hours,
            egress_bytes: report.egress_bytes,
            deterministic_charge_nano: outcome.deterministic_charge_nano,
            micropayment_credit_generated_nano: outcome.micropayment_credit_generated_nano,
            micropayment_credit_applied_nano: outcome.micropayment_credit_applied_nano,
            micropayment_credit_carry_nano: outcome.micropayment_credit_carry_nano,
            outstanding_nano: outcome.outstanding_nano,
            tickets_processed: u64::try_from(outcome.tickets_processed).unwrap_or(u64::MAX),
            tickets_won: u64::try_from(outcome.tickets_won).unwrap_or(u64::MAX),
            tickets_duplicate: u64::try_from(outcome.tickets_duplicate).unwrap_or(u64::MAX),
        };
        let event =
            SharedDataEvent::from(DataEvent::Sorafs(SorafsGatewayEvent::DealUsage(payload)));
        if let Err(err) = self.events.send(EventBox::Data(event)) {
            iroha_logger::warn!(?err, "failed to broadcast SoraFS deal usage event");
        }
    }

    #[cfg(feature = "app_api")]
    fn publish_deal_settlement_event(
        &self,
        outcome: &sorafs_node::DealSettlementOutcome,
        encoded: &[u8],
        encoded_b64: &str,
    ) {
        use std::convert::TryFrom;

        let mut digest = [0u8; 32];
        digest.copy_from_slice(blake3_hash(encoded).as_bytes());
        let payload = SorafsDealSettlement {
            record: outcome.record.clone(),
            governance_encoded_blake3: digest,
            governance_encoded_len: u64::try_from(encoded.len()).unwrap_or(u64::MAX),
            governance_encoded_b64: encoded_b64.to_owned(),
        };
        let event = SharedDataEvent::from(DataEvent::Sorafs(SorafsGatewayEvent::DealSettlement(
            payload,
        )));
        if let Err(err) = self.events.send(EventBox::Data(event)) {
            iroha_logger::warn!(?err, "failed to broadcast SoraFS deal settlement event");
        }
    }

    #[cfg(feature = "app_api")]
    fn current_tls_snapshot(&self) -> Option<sorafs::gateway::TlsStateSnapshot> {
        let state = self.sorafs_gateway_tls_state.as_ref()?;
        let snapshot = state.try_read().ok()?.clone();
        #[cfg(feature = "telemetry")]
        self.telemetry
            .with_metrics(|metrics| snapshot.apply_metrics(metrics, SystemTime::now()));
        Some(snapshot)
    }

    #[cfg(feature = "app_api")]
    fn provider_supports_chunk_range(&self, provider_id: &[u8; 32]) -> Option<bool> {
        if let Some(override_entry) = self.sorafs_chunk_range_overrides.get(provider_id) {
            return Some(*override_entry);
        }
        let cache = self.sorafs_cache.as_ref()?;
        let guard = cache.try_read().ok()?;
        let record = guard.record_by_provider(provider_id)?;
        let supported = record
            .known_capabilities()
            .contains(&CapabilityType::ChunkRangeFetch);
        Some(supported)
    }

    fn consume_rbc_sampling_budget(&self, key: &str, bytes: u64) -> Result<(), Error> {
        if self.rbc_sampling_daily_budget == 0 {
            return Ok(());
        }
        let now = Instant::now();
        let mut entry =
            self.rbc_sampling_budget
                .entry(key.to_string())
                .or_insert(SamplingBudgetEntry {
                    bytes_served: 0,
                    window_start: now,
                });
        if now.duration_since(entry.window_start) >= Duration::from_hours(24) {
            entry.bytes_served = 0;
            entry.window_start = now;
        }
        if entry.bytes_served.saturating_add(bytes) > self.rbc_sampling_daily_budget {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
        entry.bytes_served = entry.bytes_served.saturating_add(bytes);
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum NoritoRpcGateFailure {
    Disabled,
    CanaryDenied,
    MtlsRequired,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
/// Response body for `/rpc/capabilities`.
struct RpcCapabilitiesResponse {
    /// Norito-RPC capability advert.
    norito_rpc: RpcNoritoRpcCapability,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
/// Response body for `/rpc/ping`.
struct RpcPingResponse {
    /// Whether the node is healthy.
    ok: bool,
    /// Current UNIX timestamp reported by the node (milliseconds).
    unix_time_ms: u64,
    /// Norito-RPC capability advert.
    norito_rpc: RpcNoritoRpcCapability,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
/// Norito-RPC rollout summary shared with RPC clients.
struct RpcNoritoRpcCapability {
    /// Whether Norito-RPC decoding is enabled.
    enabled: bool,
    /// Whether ingress requires mTLS before allowing Norito-RPC.
    require_mtls: bool,
    /// Rollout stage label (`disabled`, `canary`, `ga`).
    stage: String,
    /// Size of the canary allowlist.
    canary_allowlist_size: usize,
}

impl From<&NoritoRpcTransport> for RpcNoritoRpcCapability {
    fn from(value: &NoritoRpcTransport) -> Self {
        Self {
            enabled: value.enabled,
            require_mtls: value.require_mtls,
            stage: value.stage.label().to_string(),
            canary_allowlist_size: value.allowed_clients.len(),
        }
    }
}

fn evaluate_norito_rpc_gate(
    cfg: &NoritoRpcTransport,
    headers: &HeaderMap,
) -> Result<(), NoritoRpcGateFailure> {
    if !cfg.enabled {
        return Err(NoritoRpcGateFailure::Disabled);
    }
    if cfg.require_mtls && !norito_rpc_mtls_present(headers) {
        return Err(NoritoRpcGateFailure::MtlsRequired);
    }
    match cfg.stage {
        NoritoRpcStage::Disabled => Err(NoritoRpcGateFailure::Disabled),
        NoritoRpcStage::Ga => Ok(()),
        NoritoRpcStage::Canary => {
            let token =
                norito_rpc_token_from_headers(headers).ok_or(NoritoRpcGateFailure::CanaryDenied)?;
            if cfg.allowed_clients.iter().any(|allowed| allowed == token) {
                Ok(())
            } else {
                Err(NoritoRpcGateFailure::CanaryDenied)
            }
        }
    }
}

fn norito_rpc_token_from_headers(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(HEADER_API_TOKEN)
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty())
}

fn norito_rpc_mtls_present(headers: &HeaderMap) -> bool {
    headers
        .get(HEADER_MTLS_FORWARD)
        .and_then(|value| value.to_str().ok())
        .map_or(false, |value| !value.is_empty())
}

fn norito_rpc_error_response(failure: NoritoRpcGateFailure) -> axum::response::Response {
    match failure {
        NoritoRpcGateFailure::Disabled => norito_rpc_error(
            StatusCode::FORBIDDEN,
            NORITO_RPC_DISABLED_CODE,
            "Norito-RPC is disabled on this Torii node",
        ),
        NoritoRpcGateFailure::CanaryDenied => norito_rpc_error(
            StatusCode::FORBIDDEN,
            NORITO_RPC_CANARY_DENIED_CODE,
            "Norito-RPC canary allowlist rejected this token",
        ),
        NoritoRpcGateFailure::MtlsRequired => norito_rpc_error(
            StatusCode::FORBIDDEN,
            NORITO_RPC_MTLS_REQUIRED_CODE,
            "Norito-RPC requires mTLS at ingress",
        ),
    }
}

fn norito_rpc_error(
    status: StatusCode,
    code: &str,
    message: &'static str,
) -> axum::response::Response {
    match axum::response::Response::builder()
        .status(status)
        .header(HEADER_NORITO_RPC_ERROR, code)
        .header(
            axum::http::header::RETRY_AFTER,
            NORITO_RPC_RETRY_AFTER_SECONDS,
        )
        .body(Body::from(message))
    {
        Ok(response) => response,
        Err(err) => {
            iroha_logger::error!(?err, "failed to build Norito-RPC gate error response");
            axum::response::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("failed to evaluate Norito-RPC gate"))
                .expect("static response build succeeds")
        }
    }
}

async fn inject_remote_addr_header(
    mut req: axum::http::Request<Body>,
    next: Next,
) -> Result<axum::response::Response, Infallible> {
    use axum::{extract::ConnectInfo, http::header::HeaderName};

    let header = HeaderName::from_static(limits::REMOTE_ADDR_HEADER);
    req.headers_mut().remove(&header);
    if let Some(connect) = req.extensions().get::<ConnectInfo<std::net::SocketAddr>>() {
        if let Ok(value) = connect.0.ip().to_string().parse() {
            req.headers_mut().insert(header, value);
        }
    }

    Ok(next.run(req).await)
}

async fn enforce_preauth(
    State(app): State<SharedAppState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Result<axum::response::Response, Infallible> {
    use axum::{extract::ConnectInfo, response::IntoResponse};

    let remote_ip = req
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip());
    let scheme = ConnScheme::from_request(&req);
    match app.acquire_preauth(remote_ip, scheme).await {
        Ok(guard) => {
            if matches!(scheme, ConnScheme::NoritoRpc) {
                if let Err(resp) = app.check_norito_rpc_allowed(req.headers()) {
                    drop(guard);
                    return Ok(*resp);
                }
            }

            let response = next.run(req).await;
            drop(guard);
            Ok(response)
        }
        Err(reason) => {
            app.record_preauth_reject(reason);
            let (status, body) = match reason {
                limits::RejectReason::GlobalCap => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "torii pre-auth global connection limit reached",
                ),
                limits::RejectReason::IpCap => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "torii pre-auth per-ip connection limit reached",
                ),
                limits::RejectReason::RateLimited => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "torii pre-auth rate limit exceeded",
                ),
                limits::RejectReason::Banned => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "torii pre-auth temporary ban in effect",
                ),
                limits::RejectReason::SchemeCap => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "torii pre-auth scheme connection limit reached",
                ),
            };
            Ok((status, body).into_response())
        }
    }
}

async fn enforce_api_token(
    State(app): State<SharedAppState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Result<axum::response::Response, Infallible> {
    if let Err(err) = validate_api_token(&app, req.headers()) {
        return Ok(err.into_response());
    }

    Ok(next.run(req).await)
}

async fn enforce_api_version(
    State(app): State<SharedAppState>,
    mut req: axum::http::Request<Body>,
    next: Next,
) -> Result<axum::response::Response, Infallible> {
    let negotiated = match api_version::negotiate(req.headers(), &app.api_versions) {
        Ok(version) => version,
        Err(err) => {
            let result_label = err.result_label();
            let version_label = err.version_label();
            app.telemetry.with_metrics(|metrics| {
                metrics.observe_torii_api_version(result_label, version_label.as_str())
            });
            iroha_logger::warn!(
                code = err.code(),
                requested = version_label.as_str(),
                supported = app.api_versions.supported_labels().as_str(),
                "Torii API version negotiation failed"
            );
            return Ok(err.into_error().into_response());
        }
    };

    req.extensions_mut().insert(negotiated);
    let negotiated_label = negotiated.version.to_label();
    let result_label = if negotiated.inferred { "default" } else { "ok" };
    app.telemetry.with_metrics(|metrics| {
        metrics.observe_torii_api_version(result_label, negotiated_label.as_str())
    });

    let mut response = next.run(req).await;
    if let Ok(val) = HeaderValue::from_str(&negotiated_label) {
        response
            .headers_mut()
            .insert(iroha_torii_shared::HEADER_API_VERSION, val);
    }
    let supported = app
        .api_versions
        .supported
        .iter()
        .copied()
        .map(ApiVersion::to_label)
        .collect::<Vec<_>>()
        .join(", ");
    if let Ok(val) = HeaderValue::from_str(&supported) {
        response.headers_mut().insert("x-iroha-api-supported", val);
    }
    if let Ok(val) = HeaderValue::from_str(&app.api_versions.min_proof.to_label()) {
        response
            .headers_mut()
            .insert("x-iroha-api-min-proof-version", val);
    }
    if let Some(sunset) = app.api_versions.sunset_unix {
        if let Ok(val) = HeaderValue::from_str(&sunset.to_string()) {
            response
                .headers_mut()
                .insert("x-iroha-api-sunset-unix", val);
        }
    }

    Ok(response)
}

async fn record_http_metrics(
    State(app): State<SharedAppState>,
    req: axum::http::Request<Body>,
    next: Next,
) -> Result<axum::response::Response, Infallible> {
    fn normalise_content_type(raw: &str) -> String {
        raw.split(';')
            .next()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "unknown".to_string())
    }

    let method = req.method().clone();
    let scheme_label = ConnScheme::from_request(&req).label();
    let telemetry = app.telemetry_handle();
    let start = Instant::now();
    let response = next.run(req).await;
    let duration = start.elapsed();
    let status = response.status();
    let content_type_header = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let content_type_label = content_type_header
        .map(normalise_content_type)
        .unwrap_or_else(|| "unknown".to_string());
    let response_bytes = response
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    telemetry.with_metrics(|tel| {
        tel.observe_torii_http_request(
            method.as_str(),
            status,
            content_type_label.as_str(),
            duration,
            response_bytes,
        );
        tel.observe_torii_request_by_scheme(scheme_label, status, duration);
    });

    Ok(response)
}

// Small helper to enforce token + rate limit
async fn check_access(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &str,
) -> Result<(), Error> {
    check_access_enforced(app, headers, remote, hint, true).await
}

fn validate_api_token(app: &AppState, headers: &axum::http::HeaderMap) -> Result<(), Error> {
    let token_hdr = headers
        .get(HEADER_API_TOKEN)
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token {
        if app.api_tokens_set.is_empty() {
            return Err(Error::Query(
                iroha_data_model::ValidationFail::NotPermitted(
                    "Torii requires an API token but none are configured".to_string(),
                ),
            ));
        }
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(
                iroha_data_model::ValidationFail::NotPermitted(
                    "missing or invalid API token".to_string(),
                ),
            ));
        }
    }
    Ok(())
}

async fn check_access_enforced(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &str,
    enforce_rate: bool,
) -> Result<(), Error> {
    check_access_enforced_with_cost(app, headers, remote, hint, enforce_rate, 1).await
}

async fn check_access_enforced_with_cost(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &str,
    enforce_rate: bool,
    cost: u64,
) -> Result<(), Error> {
    validate_api_token(app, headers)?;
    let key = rate_limit_key(headers, remote, hint, app.api_token_enforced());
    let cost = cost.max(1);
    if !limits::allow_cost_conditionally(&app.rate_limiter, &key, cost, enforce_rate).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(())
}

fn rate_limit_key(
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &str,
    use_api_token: bool,
) -> String {
    limits::key_from_headers(headers, remote, Some(hint), use_api_token)
}

async fn rate_limit_requests(app: &SharedAppState, key: &str) -> Result<(), Error> {
    if !limits::allow_conditionally(&app.rate_limiter, key, true).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(())
}

const SORANET_PRIVACY_TOKEN_HEADER: &str = "x-soranet-privacy-token";

fn privacy_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(SORANET_PRIVACY_TOKEN_HEADER)
        .or_else(|| headers.get("x-api-token"))
        .and_then(|value| value.to_str().ok())
}

fn privacy_reject(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> AxResponse {
    let payload = ErrorEnvelope::new(code, message.into());
    (status, utils::NoritoBody(payload)).into_response()
}

async fn enforce_soranet_privacy_ingest(
    app: &SharedAppState,
    headers: &HeaderMap,
    remote: Option<IpAddr>,
    endpoint: &'static str,
) -> Result<(), AxResponse> {
    let ingest_cfg = &app.soranet_privacy_ingest;
    #[cfg(not(feature = "telemetry"))]
    let _ = endpoint;
    if !ingest_cfg.enabled {
        #[cfg(feature = "telemetry")]
        app.telemetry
            .record_soranet_privacy_ingest_reject(endpoint, "disabled")
            .await;
        return Err(privacy_reject(
            StatusCode::SERVICE_UNAVAILABLE,
            "soranet_privacy_disabled",
            "soranet privacy ingestion is disabled",
        ));
    }

    if app.soranet_privacy_allow_nets.is_empty()
        || !limits::is_allowed_by_cidr(headers, remote, &app.soranet_privacy_allow_nets)
    {
        #[cfg(feature = "telemetry")]
        app.telemetry
            .record_soranet_privacy_ingest_reject(endpoint, "namespace_blocked")
            .await;
        return Err(privacy_reject(
            StatusCode::FORBIDDEN,
            "soranet_privacy_namespace_blocked",
            "submitter not in allowed namespace",
        ));
    }

    let token = privacy_token(headers);
    if ingest_cfg.require_token {
        let Some(token_val) = token else {
            #[cfg(feature = "telemetry")]
            app.telemetry
                .record_soranet_privacy_ingest_reject(endpoint, "missing_token")
                .await;
            return Err(privacy_reject(
                StatusCode::UNAUTHORIZED,
                "soranet_privacy_token_required",
                "missing SoraNet privacy token",
            ));
        };
        if !app.soranet_privacy_tokens.contains(token_val) {
            #[cfg(feature = "telemetry")]
            app.telemetry
                .record_soranet_privacy_ingest_reject(endpoint, "invalid_token")
                .await;
            return Err(privacy_reject(
                StatusCode::UNAUTHORIZED,
                "soranet_privacy_token_invalid",
                "token not authorised for SoraNet privacy ingest",
            ));
        }
    }

    let rate_key = token
        .map(ToOwned::to_owned)
        .or_else(|| remote.map(|ip| ip.to_string()))
        .unwrap_or_else(|| "anonymous".to_string());
    let enforce_rate = ingest_cfg.rate_per_sec.is_some();
    if !limits::allow_conditionally(&app.soranet_privacy_rate_limiter, &rate_key, enforce_rate)
        .await
    {
        #[cfg(feature = "telemetry")]
        app.telemetry
            .record_soranet_privacy_ingest_reject(endpoint, "rate_limited")
            .await;
        return Err(privacy_reject(
            StatusCode::TOO_MANY_REQUESTS,
            "soranet_privacy_rate_limited",
            "soranet privacy ingest is rate limited",
        ));
    }

    Ok(())
}

#[cfg(all(feature = "app_api", feature = "push"))]
fn push_error_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> AxResponse {
    let payload = ErrorEnvelope::new(code, message.into());
    (status, utils::NoritoBody(payload)).into_response()
}

#[cfg(all(feature = "app_api", feature = "push"))]
async fn handler_push_register_device(
    State(app): State<SharedAppState>,
    NoritoJson(req): NoritoJson<push::RegisterDeviceRequest>,
) -> AxResponse {
    if app.push.is_none() {
        return push_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "push_disabled",
            "push bridge disabled",
        );
    }
    let rate_key = format!("push:{}", req.account_id);
    if !app.push_rate_limiter.allow(&rate_key).await {
        return push_error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "push_rate_limited",
            "push registrations are rate limited",
        );
    }
    let bridge = app.push.as_ref().expect("checked push presence");
    match bridge.register_device(req) {
        Ok(()) => StatusCode::ACCEPTED.into_response(),
        Err(push::PushError::Disabled) => push_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "push_disabled",
            "push bridge disabled",
        ),
        Err(push::PushError::MissingCredentials { platform }) => push_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "push_missing_credentials",
            format!("missing credentials for {}", platform.label()),
        ),
        Err(push::PushError::InvalidAccount(value)) => push_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_account",
            format!("invalid account id `{value}`"),
        ),
        Err(push::PushError::InvalidPlatform(value)) => push_error_response(
            StatusCode::BAD_REQUEST,
            "invalid_platform",
            format!("unsupported platform `{value}`"),
        ),
        Err(push::PushError::TooManyTopics { max }) => push_error_response(
            StatusCode::BAD_REQUEST,
            "too_many_topics",
            format!("topics exceed configured maximum ({max})"),
        ),
        Err(push::PushError::EmptyToken) => push_error_response(
            StatusCode::BAD_REQUEST,
            "empty_token",
            "token must not be empty",
        ),
    }
}

fn ensure_proof_api_version(
    app: &AppState,
    negotiated: api_version::NegotiatedVersion,
    endpoint: &'static str,
) -> Result<(), Error> {
    if let Err(err) = api_version::enforce_minimum(
        negotiated.version,
        app.api_versions.min_proof,
        &app.api_versions,
    ) {
        let label = err.version_label();
        app.telemetry.with_metrics(|metrics| {
            metrics.observe_torii_api_version(err.result_label(), label.as_str())
        });
        iroha_logger::warn!(
            endpoint,
            requested = label.as_str(),
            minimum = app.api_versions.min_proof.to_label().as_str(),
            inferred = negotiated.inferred,
            "rejecting request below proof API minimum version"
        );
        return Err(err.into_error());
    }
    Ok(())
}

async fn check_proof_access(
    app: &AppState,
    negotiated: api_version::NegotiatedVersion,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &'static str,
    cost: u64,
    enforce_rate: bool,
) -> Result<(), Error> {
    ensure_proof_api_version(app, negotiated, hint)?;
    check_access_enforced(app, headers, remote, hint, enforce_rate).await?;
    let key = rate_limit_key(headers, remote, hint, app.api_token_enforced());
    if limits::allow_cost_conditionally(&app.proof_rate_limiter, &key, cost, enforce_rate).await {
        return Ok(());
    }
    app.telemetry
        .with_metrics(|tel| tel.inc_torii_proof_throttle(hint));
    app.telemetry.with_metrics(|tel| {
        tel.observe_torii_proof_request(hint, "rate_limited", 0, std::time::Duration::from_secs(0))
    });
    let retry_after_secs = app.proof_limits.retry_after.as_secs().max(1);
    iroha_logger::warn!(
        %hint,
        %retry_after_secs,
        %key,
        "proof endpoint throttled request"
    );
    Err(Error::ProofRateLimited {
        endpoint: hint,
        retry_after_secs,
    })
}

fn enforce_proof_body_limit(app: &AppState, len: usize, hint: &'static str) -> Result<(), Error> {
    let max = app.proof_limits.max_body_bytes;
    if (len as u64) <= max {
        return Ok(());
    }
    app.telemetry
        .with_metrics(|tel| tel.inc_torii_proof_throttle(hint));
    Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
            "payload too large for {hint}: {len} > {max}"
        )),
    )))
}

async fn enforce_proof_egress(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &'static str,
    bytes: u64,
    enforce_rate: bool,
) -> Result<(), Error> {
    if bytes == 0 {
        return Ok(());
    }
    let key = rate_limit_key(headers, remote, hint, app.api_token_enforced());
    if limits::allow_cost_conditionally(&app.proof_egress_limiter, &key, bytes, enforce_rate).await
    {
        return Ok(());
    }
    app.telemetry
        .with_metrics(|tel| tel.inc_torii_proof_throttle(hint));
    let retry_after_secs = app.proof_limits.retry_after.as_secs().max(1);
    iroha_logger::warn!(
        %hint,
        %retry_after_secs,
        %key,
        bytes,
        "proof endpoint egress throttled request"
    );
    Err(Error::ProofRateLimited {
        endpoint: hint,
        retry_after_secs,
    })
}

// -------------- Governance handlers (AppState-based) --------------
use axum::{extract::Path as AxPath, response::Response as AxResponse};

use crate::NoritoQuery as AxQuery;

async fn handler_contracts_instances_by_ns(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(ns): AxPath<String>,
    AxQuery(q): AxQuery<crate::gov::InstancesQuery>,
) -> Result<JsonBody<crate::gov::InstancesByNamespaceResponse>, Error> {
    check_access(&app, &headers, None, "v1/contracts/instances/{ns}").await?;
    crate::gov::handle_gov_instances_by_ns(app.state.clone(), AxPath(ns), AxQuery(q)).await
}

async fn handler_gov_instances_ns(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(ns): AxPath<String>,
    AxQuery(q): AxQuery<crate::gov::InstancesQuery>,
) -> Result<JsonBody<crate::gov::InstancesByNamespaceResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/instances/{ns}").await?;
    crate::gov::handle_gov_instances_by_ns(app.state.clone(), AxPath(ns), AxQuery(q)).await
}

async fn handler_gov_enact(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::NoritoJson<crate::gov::EnactDto>,
) -> Result<JsonBody<crate::gov::EnactResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/enact").await?;
    crate::gov::handle_gov_enact(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        body,
    )
    .await
}

async fn handler_gov_council_current(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<JsonBody<crate::gov::CouncilCurrentResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/council/current").await?;
    crate::gov::handle_gov_council_current(app.state.clone()).await
}

async fn handler_gov_proposal_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: AxPath<String>,
) -> Result<JsonBody<crate::gov::ProposalGetResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/proposals/{id}").await?;
    crate::gov::handle_gov_get_proposal(app.state.clone(), id).await
}

async fn handler_gov_locks_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    rid: AxPath<String>,
) -> Result<JsonBody<crate::gov::LocksGetResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/locks/{rid}").await?;
    crate::gov::handle_gov_get_locks(app.state.clone(), rid).await
}

async fn handler_gov_referendum_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: AxPath<String>,
) -> Result<JsonBody<crate::gov::ReferendumGetResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/referenda/{id}").await?;
    crate::gov::handle_gov_get_referendum(app.state.clone(), id).await
}

// Missing wrappers for governance endpoints that require AppState access/rate limiting
async fn handler_gov_propose_deploy(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::NoritoJson<crate::gov::ProposeDeployContractDto>,
) -> Result<JsonBody<crate::gov::ProposeDeployContractResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/proposals/deploy-contract").await?;
    crate::gov::handle_gov_propose_deploy(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        body,
    )
    .await
}

async fn handler_gov_protected_set(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(body): crate::utils::extractors::NoritoJson<
        crate::gov::ProtectedNamespacesDto,
    >,
) -> Result<JsonBody<crate::gov::ProtectedNamespacesApplyResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/protected").await?;
    crate::gov::handle_gov_protected_set(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(body),
    )
    .await
}

async fn handler_gov_protected_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<JsonBody<crate::gov::ProtectedNamespacesGetResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/protected").await?;
    crate::gov::handle_gov_protected_get(app.state.clone()).await
}

async fn handler_gov_tally_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: AxPath<String>,
) -> Result<JsonBody<crate::gov::TallyGetResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/tally/{id}").await?;
    crate::gov::handle_gov_get_tally(app.state.clone(), id).await
}

async fn handler_gov_unlock_stats(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<JsonBody<crate::gov::UnlockStatsResponse>, Error> {
    check_access(&app, &headers, None, "v1/gov/unlocks/stats").await?;
    crate::gov::handle_gov_unlock_stats(app.state.clone()).await
}

#[cfg(feature = "app_api")]
async fn handler_account_transactions_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_id): AxPath<String>,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    let tel = app.telemetry.clone();
    let key_hint = account_id.clone();
    #[allow(unused_variables)]
    let canonical_account = account_id.clone();
    let limits = crate::routing::app_query_limits();
    let mut env = env;
    let page_limit = limits.clamp_page_limit(env.pagination.limit)?;
    env.pagination.limit = Some(page_limit);
    env.fetch_size = limits.clamp_fetch_size(env.fetch_size)?;
    let payload = crate::utils::extractors::NoritoJson(env);

    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_account_transactions_with_policy(
            app.state.clone(),
            AxPath(account_id),
            payload,
            tel.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    let cost = limits.rate_limit_cost(page_limit);
    check_access_enforced_with_cost(&app, &headers, None, &key_hint, enforce, cost).await?;

    routing::handle_v1_account_transactions_with_policy(
        app.state.clone(),
        AxPath(key_hint),
        payload,
        tel,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_account_assets(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_id): AxPath<String>,
    AxQuery(p): AxQuery<crate::routing::AccountAssetsGetParams>,
) -> Result<impl IntoResponse, Error> {
    let tel = app.telemetry_handle();
    let key_hint = account_id.clone();
    let limits = crate::routing::app_query_limits();
    let page_limit = limits.clamp_page_limit(p.limit)?;
    let mut p = p;
    p.limit = Some(page_limit);
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_account_assets_with_policy(
            app.state.clone(),
            AxPath(account_id),
            AxQuery(p),
            tel.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    let cost = limits.rate_limit_cost(page_limit);
    check_access_enforced_with_cost(&app, &headers, None, &key_hint, enforce, cost).await?;

    routing::handle_v1_account_assets_with_policy(
        app.state.clone(),
        AxPath(key_hint),
        AxQuery(p),
        tel,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_account_permissions(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_id): AxPath<String>,
    AxQuery(p): AxQuery<crate::filter::Pagination>,
) -> Result<impl IntoResponse, Error> {
    let key_hint = account_id.clone();
    let tel = app.telemetry_handle();
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_account_permissions_with_policy(
            app.state.clone(),
            AxPath(account_id),
            AxQuery(p),
            tel.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, &key_hint, enforce).await?;

    routing::handle_v1_account_permissions_with_policy(
        app.state.clone(),
        AxPath(key_hint),
        AxQuery(p),
        tel,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_account_assets_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_id): AxPath<String>,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    let tel = app.telemetry_handle();
    let key_hint = account_id.clone();
    let limits = crate::routing::app_query_limits();
    let mut env = env;
    let page_limit = limits.clamp_page_limit(env.pagination.limit)?;
    env.pagination.limit = Some(page_limit);
    env.fetch_size = limits.clamp_fetch_size(env.fetch_size)?;
    let payload = crate::utils::extractors::NoritoJson(env);
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_account_assets_query_with_policy(
            app.state.clone(),
            AxPath(account_id),
            payload,
            tel.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    let cost = limits.rate_limit_cost(page_limit);
    check_access_enforced_with_cost(&app, &headers, None, &key_hint, enforce, cost).await?;

    routing::handle_v1_account_assets_query_with_policy(
        app.state.clone(),
        AxPath(key_hint),
        payload,
        tel,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_account_transactions_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_id): AxPath<String>,
    AxQuery(params): AxQuery<crate::routing::AccountTransactionsGetParams>,
) -> Result<impl IntoResponse, Error> {
    let tel = app.telemetry.clone();

    let key_hint = account_id.clone();
    #[allow(unused_variables)]
    let canonical_account = account_id.clone();
    let limits = crate::routing::app_query_limits();
    let mut params = params;
    let page_limit = limits.clamp_page_limit(params.limit)?;
    params.limit = Some(page_limit);

    let norito_params: AxQuery<crate::routing::AccountTransactionsGetParams> = AxQuery(params);

    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_account_transactions_get_with_policy(
            app.state.clone(),
            AxPath(account_id),
            norito_params.clone(),
            tel.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    let cost = limits.rate_limit_cost(page_limit);
    check_access_enforced_with_cost(&app, &headers, None, &key_hint, enforce, cost).await?;

    routing::handle_v1_account_transactions_get_with_policy(
        app.state.clone(),
        AxPath(key_hint),
        norito_params,
        tel,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_proofs_query(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    NoritoJson(dto): NoritoJson<crate::routing::ProofFindByIdQueryDto>,
) -> Result<Response, Error> {
    let tel = app.telemetry.clone();
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    ensure_proof_api_version(&app, negotiated, "v1/proofs/query")?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        let signed = crate::routing::signed_find_proof_by_id(&dto)?;
        return routing::handle_queries_with_opts(
            app.query_service.clone(),
            app.state.clone(),
            signed,
            tel,
            crate::NoritoQuery(QueryOptions::default()),
            format,
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/proofs/query", enforce).await?;

    let signed = crate::routing::signed_find_proof_by_id(&dto)?;
    routing::handle_queries_with_opts(
        app.query_service.clone(),
        app.state.clone(),
        signed,
        tel,
        crate::NoritoQuery(QueryOptions::default()),
        format,
    )
    .await
}

#[cfg(all(feature = "app_api", feature = "zk-proof-tags"))]
async fn handler_proof_tags(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    AxPath((backend, hash)): AxPath<(String, String)>,
) -> Result<impl IntoResponse, Error> {
    ensure_proof_api_version(&app, negotiated, "v1/zk/proof-tags/{backend}/{hash}")?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_get_proof_tags(app.state.clone(), AxPath((backend, hash))).await;
    }

    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/zk/proof-tags/{backend}/{hash}",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    routing::handle_get_proof_tags(app.state.clone(), AxPath((backend, hash))).await
}

#[cfg(all(feature = "app_api", not(feature = "zk-proof-tags")))]
async fn handler_proof_tags(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath((backend, hash)): AxPath<(String, String)>,
) -> Result<AxResponse, Error> {
    let _ = (app, headers, backend, hash);
    Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::NotFound,
    )))
}

/// Forward `/v1/zk/verify-batch` requests to the routing handler.
///
/// # Errors
/// Propagates routing errors when the batch verification fails.
#[cfg(all(feature = "app_api", feature = "zk-verify-batch"))]
pub async fn handle_v1_zk_verify_batch(
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    crate::routing::handle_v1_zk_verify_batch(headers, body).await
}

/// Fallback `/v1/zk/verify-batch` handler when `zk-verify-batch` feature is disabled.
///
/// # Errors
/// Never errors; always returns a `501 Not Implemented` response.
#[cfg(all(feature = "app_api", not(feature = "zk-verify-batch")))]
pub async fn handle_v1_zk_verify_batch(
    _headers: axum::http::HeaderMap,
    _body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    Ok((
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "zk-verify-batch feature disabled",
    ))
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_accounts_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::ListFilterParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_accounts(app.state.clone(), AxQuery(p), app.telemetry.clone())
            .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/accounts", enforce).await?;

    routing::handle_v1_accounts(app.state.clone(), AxQuery(p), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_accounts_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_accounts_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/accounts/query", enforce).await?;

    routing::handle_v1_accounts_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_accounts_resolve(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(request): crate::utils::extractors::NoritoJson<
        crate::routing::AccountResolveRequestDto,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_accounts_resolve(
            crate::utils::extractors::NoritoJson(request),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/accounts/resolve", enforce).await?;

    routing::handle_v1_accounts_resolve(
        crate::utils::extractors::NoritoJson(request),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_accounts_onboard(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: crate::utils::extractors::NoritoJson<crate::routing::AccountOnboardingRequestDto>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_accounts_onboard(app.clone(), request, app.telemetry.clone())
            .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_ACCOUNTS_ONBOARD.trim_start_matches('/'),
        enforce,
    )
    .await?;

    routing::handle_v1_accounts_onboard(app.clone(), request, app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct AccountsPortfolioQuery {
    #[norito(default)]
    asset_id: Option<String>,
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_accounts_portfolio(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(uaid_literal): AxPath<String>,
    AxQuery(query): AxQuery<AccountsPortfolioQuery>,
) -> Result<impl IntoResponse, Error> {
    let asset_id = match query.asset_id {
        Some(raw) => Some(parse_asset_id(&raw)?),
        None => None,
    };
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_accounts_portfolio(
            app.state.clone(),
            AxPath(uaid_literal),
            asset_id,
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/accounts/{uaid}/portfolio",
        enforce,
    )
    .await?;

    routing::handle_v1_accounts_portfolio(
        app.state.clone(),
        AxPath(uaid_literal),
        asset_id,
        app.telemetry.clone(),
    )
    .await
}

pub(crate) fn ensure_nexus_lanes_enabled(
    nexus_enabled: bool,
    endpoint: &'static str,
) -> Result<(), Error> {
    if nexus_enabled {
        Ok(())
    } else {
        Err(Error::AppQueryValidation {
            code: "nexus_disabled",
            message: format!(
                "{endpoint} requires nexus.enabled=true; lanes are unavailable in Iroha 2 mode"
            ),
        })
    }
}

#[cfg(feature = "app_api")]
#[cfg(test)]
mod nexus_lane_boundary_tests {
    use super::*;

    #[test]
    fn gating_rejects_when_nexus_disabled() {
        let err = ensure_nexus_lanes_enabled(false, routing::ENDPOINT_NEXUS_PUBLIC_LANE_STAKE)
            .expect_err("lane endpoints must reject when Nexus is disabled");
        match err {
            Error::AppQueryValidation { code, message } => {
                assert_eq!(code, "nexus_disabled");
                assert!(
                    message.contains("nexus.enabled=true"),
                    "message should explain the required flag: {message}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn gating_allows_when_nexus_enabled() {
        ensure_nexus_lanes_enabled(true, routing::ENDPOINT_NEXUS_PUBLIC_LANE_STAKE)
            .expect("lane endpoints should be available when Nexus is enabled");
    }
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_nexus_public_lane_validators(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(lane_literal): AxPath<String>,
    crate::NoritoQuery(params): crate::NoritoQuery<routing::PublicLaneValidatorsQueryParams>,
) -> Result<impl IntoResponse, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    ensure_nexus_lanes_enabled(
        nexus_enabled,
        routing::ENDPOINT_NEXUS_PUBLIC_LANE_VALIDATORS,
    )?;
    let lane_id = routing::parse_lane_id_literal(&lane_literal)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_nexus_public_lane_validators(
            app.state.clone(),
            lane_id,
            params,
            app.telemetry.clone(),
        )
        .await;
    }
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_NEXUS_PUBLIC_LANE_VALIDATORS.trim_start_matches('/'),
        enforce,
    )
    .await?;
    routing::handle_v1_nexus_public_lane_validators(
        app.state.clone(),
        lane_id,
        params,
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_nexus_public_lane_stake(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(lane_literal): AxPath<String>,
    crate::NoritoQuery(params): crate::NoritoQuery<routing::PublicLaneStakeQueryParams>,
) -> Result<impl IntoResponse, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    ensure_nexus_lanes_enabled(nexus_enabled, routing::ENDPOINT_NEXUS_PUBLIC_LANE_STAKE)?;
    let lane_id = routing::parse_lane_id_literal(&lane_literal)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_nexus_public_lane_stake(
            app.state.clone(),
            lane_id,
            params,
            app.telemetry.clone(),
        )
        .await;
    }
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_NEXUS_PUBLIC_LANE_STAKE.trim_start_matches('/'),
        enforce,
    )
    .await?;
    routing::handle_v1_nexus_public_lane_stake(
        app.state.clone(),
        lane_id,
        params,
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_nexus_public_lane_rewards(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(lane_literal): AxPath<String>,
    crate::NoritoQuery(params): crate::NoritoQuery<routing::PublicLaneRewardsQueryParams>,
) -> Result<impl IntoResponse, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    ensure_nexus_lanes_enabled(nexus_enabled, routing::ENDPOINT_NEXUS_PUBLIC_LANE_REWARDS)?;
    let lane_id = routing::parse_lane_id_literal(&lane_literal)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_nexus_public_lane_rewards(
            app.state.clone(),
            lane_id,
            params,
            app.telemetry.clone(),
        )
        .await;
    }
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_NEXUS_PUBLIC_LANE_REWARDS.trim_start_matches('/'),
        enforce,
    )
    .await?;
    routing::handle_v1_nexus_public_lane_rewards(
        app.state.clone(),
        lane_id,
        params,
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_nexus_dataspaces_account_summary(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(literal): AxPath<String>,
    crate::NoritoQuery(params): crate::NoritoQuery<
        routing::NexusDataspacesAccountSummaryQueryParams,
    >,
) -> Result<impl IntoResponse, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    ensure_nexus_lanes_enabled(
        nexus_enabled,
        routing::ENDPOINT_NEXUS_DATASPACES_ACCOUNT_SUMMARY,
    )?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_nexus_dataspaces_account_summary(
            app.state.clone(),
            AxPath(literal),
            crate::NoritoQuery(params.clone()),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_NEXUS_DATASPACES_ACCOUNT_SUMMARY.trim_start_matches('/'),
        enforce,
    )
    .await?;
    routing::handle_v1_nexus_dataspaces_account_summary(
        app.state.clone(),
        AxPath(literal),
        crate::NoritoQuery(params),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_space_directory_bindings(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(uaid_literal): AxPath<String>,
    AxQuery(query): AxQuery<crate::routing::SpaceDirectoryBindingsQuery>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_space_directory_bindings(
            app.state.clone(),
            AxPath(uaid_literal),
            AxQuery(query.clone()),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_SPACE_DIRECTORY_BINDINGS.trim_start_matches('/'),
        enforce,
    )
    .await?;

    routing::handle_v1_space_directory_bindings(
        app.state.clone(),
        AxPath(uaid_literal),
        AxQuery(query),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_space_directory_manifests(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(uaid_literal): AxPath<String>,
    AxQuery(query): AxQuery<crate::routing::SpaceDirectoryManifestQuery>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_space_directory_manifests(
            app.state.clone(),
            AxPath(uaid_literal),
            AxQuery(query),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_SPACE_DIRECTORY_MANIFESTS.trim_start_matches('/'),
        enforce,
    )
    .await?;

    routing::handle_v1_space_directory_manifests(
        app.state.clone(),
        AxPath(uaid_literal),
        AxQuery(query),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_space_directory_manifest_publish(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: crate::utils::extractors::NoritoJson<crate::routing::SpaceDirectoryManifestPublishDto>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_post_space_directory_manifest_publish(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            request,
        )
        .await
        .map(axum::response::IntoResponse::into_response);
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_SPACE_DIRECTORY_MANIFEST_PUBLISH.trim_start_matches('/'),
        enforce,
    )
    .await?;

    crate::routing::handle_post_space_directory_manifest_publish(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    .map(axum::response::IntoResponse::into_response)
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_space_directory_manifest_revoke(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: crate::utils::extractors::NoritoJson<crate::routing::SpaceDirectoryManifestRevokeDto>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_post_space_directory_manifest_revoke(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            request,
        )
        .await
        .map(axum::response::IntoResponse::into_response);
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        routing::ENDPOINT_SPACE_DIRECTORY_MANIFEST_REVOKE.trim_start_matches('/'),
        enforce,
    )
    .await?;

    crate::routing::handle_post_space_directory_manifest_revoke(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    .map(axum::response::IntoResponse::into_response)
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_repo_agreements(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::ListFilterParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_repo_agreements(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/repo/agreements", enforce).await?;

    routing::handle_v1_repo_agreements(app.state.clone(), AxQuery(p), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_repo_agreements_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_repo_agreements_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/repo/agreements/query", enforce).await?;

    routing::handle_v1_repo_agreements_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_allowances_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::OfflineAllowanceListParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_allowances(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/allowances", enforce).await?;

    routing::handle_v1_offline_allowances(app.state.clone(), AxQuery(p), app.telemetry.clone())
        .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_allowances_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_allowances_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/allowances/query", enforce).await?;

    routing::handle_v1_offline_allowances_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_certificates_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_allowances_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/certificates/query",
        enforce,
    )
    .await?;

    routing::handle_v1_offline_allowances_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_allowances_issue(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineAllowanceIssueRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_allowances_issue(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/allowances", enforce).await?;

    routing::handle_post_v1_offline_allowances_issue(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_certificates_issue(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineCertificateIssueRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_certificates_issue(
            app.clone(),
            crate::utils::extractors::NoritoJson(req),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/certificates/issue",
        enforce,
    )
    .await?;

    routing::handle_post_v1_offline_certificates_issue(
        app.clone(),
        crate::utils::extractors::NoritoJson(req),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_allowance_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(certificate_id_hex): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_allowance_get(
            app.state.clone(),
            certificate_id_hex,
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/allowances/{certificate_id_hex}",
        enforce,
    )
    .await?;

    routing::handle_v1_offline_allowance_get(
        app.state.clone(),
        certificate_id_hex,
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_certificates_renew_issue(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(certificate_id_hex): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineCertificateIssueRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_certificates_renew_issue(
            app.clone(),
            certificate_id_hex,
            crate::utils::extractors::NoritoJson(req),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/certificates/{certificate_id_hex}/renew/issue",
        enforce,
    )
    .await?;

    routing::handle_post_v1_offline_certificates_renew_issue(
        app.clone(),
        certificate_id_hex,
        crate::utils::extractors::NoritoJson(req),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_allowances_renew(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(certificate_id_hex): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineCertificateRenewRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_allowances_renew(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            certificate_id_hex,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/allowances/{certificate_id_hex}/renew",
        enforce,
    )
    .await?;

    routing::handle_post_v1_offline_allowances_renew(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        certificate_id_hex,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_certificates_revoke(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineCertificateRevokeRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_certificates_revoke(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/certificates/revoke",
        enforce,
    )
    .await?;

    routing::handle_post_v1_offline_certificates_revoke(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_settlements_submit(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineSettlementSubmitRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_settlements_submit(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/settlements", enforce).await?;

    routing::handle_post_v1_offline_settlements_submit(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_spend_receipts_submit(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineSpendReceiptsSubmitRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_offline_spend_receipts(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(req),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/spend-receipts", enforce).await?;

    routing::handle_post_v1_offline_spend_receipts(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(req),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_state(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_state(app.state.clone(), app.telemetry.clone()).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/state", enforce).await?;

    routing::handle_v1_offline_state(app.state.clone(), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_revocations_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::ListFilterParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_revocations(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/revocations", enforce).await?;

    routing::handle_v1_offline_revocations(app.state.clone(), AxQuery(p), app.telemetry.clone())
        .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_revocations_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_revocations_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/revocations/query",
        enforce,
    )
    .await?;

    routing::handle_v1_offline_revocations_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_summaries_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::ListFilterParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_summaries(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/summaries", enforce).await?;

    routing::handle_v1_offline_summaries(app.state.clone(), AxQuery(p), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_summaries_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_summaries_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/summaries/query", enforce).await?;

    routing::handle_v1_offline_summaries_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_receipts_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::OfflineReceiptListParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_receipts(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/receipts", enforce).await?;

    routing::handle_v1_offline_receipts(app.state.clone(), AxQuery(p), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_receipts_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_receipts_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/receipts/query", enforce).await?;

    routing::handle_v1_offline_receipts_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_transfers_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::OfflineTransferListParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_transfers(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/transfers", enforce).await?;

    routing::handle_v1_offline_transfers(app.state.clone(), AxQuery(p), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_transfer_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(bundle_id_hex): AxPath<String>,
    AxQuery(p): AxQuery<crate::routing::OfflineTransferGetParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_transfer_get(
            app.state.clone(),
            bundle_id_hex,
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/transfers/{bundle_id_hex}",
        enforce,
    )
    .await?;

    routing::handle_v1_offline_transfer_get(
        app.state.clone(),
        bundle_id_hex,
        AxQuery(p),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_transfers_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_transfers_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/transfers/query", enforce).await?;

    routing::handle_v1_offline_transfers_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_transfer_proof(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::OfflineTransferProofRequest,
    >,
) -> Result<impl IntoResponse, Error> {
    ensure_proof_api_version(&app, negotiated, "v1/offline/transfers/proof")?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_transfer_proof(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(req),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/offline/transfers/proof",
        1,
        enforce,
    )
    .await?;

    routing::handle_v1_offline_transfer_proof(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(req),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_offline_bundle_proof_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::OfflineBundleProofStatusParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_bundle_proof_status(
            app.state.clone(),
            AxQuery(p),
            app.telemetry.clone(),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/offline/bundle/proof_status",
        enforce,
    )
    .await?;

    routing::handle_v1_offline_bundle_proof_status(
        app.state.clone(),
        AxQuery(p),
        app.telemetry.clone(),
    )
    .await
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
#[axum::debug_handler]
async fn handler_offline_rejections(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_offline_rejections(app.state.clone(), app.telemetry.clone())
            .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/offline/rejections", enforce).await?;

    routing::handle_v1_offline_rejections(app.state.clone(), app.telemetry.clone()).await
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerAccountsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    domain: Option<String>,
    #[norito(default)]
    with_asset: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerDomainsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    owned_by: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerPaginationOnly {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerAssetDefinitionsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    domain: Option<String>,
    #[norito(default)]
    owned_by: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerAssetsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    owned_by: Option<String>,
    #[norito(default)]
    definition: Option<String>,
    #[norito(default)]
    asset_id: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerNftsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    owned_by: Option<String>,
    #[norito(default)]
    domain: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerTransactionsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    authority: Option<String>,
    #[norito(default)]
    block: Option<u64>,
    #[norito(default)]
    status: Option<String>,
    #[norito(default)]
    asset_id: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(JsonDeserialize)]
struct ExplorerInstructionsQuery {
    #[norito(flatten)]
    pagination: explorer::ExplorerPaginationQuery,
    #[norito(default)]
    account: Option<String>,
    #[norito(default)]
    authority: Option<String>,
    #[norito(default)]
    transaction_hash: Option<String>,
    #[norito(default)]
    transaction_status: Option<String>,
    #[norito(default)]
    block: Option<u64>,
    #[norito(default)]
    kind: Option<String>,
    #[norito(default)]
    asset_id: Option<String>,
}

#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_DOMAINS_OWNED_BY: &str = "/v1/explorer/domains?owned_by";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_ASSET_DEFINITIONS_OWNED_BY: &str = "/v1/explorer/asset-definitions?owned_by";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_ASSETS_OWNED_BY: &str = "/v1/explorer/assets?owned_by";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_NFTS_OWNED_BY: &str = "/v1/explorer/nfts?owned_by";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_TRANSACTIONS_AUTHORITY: &str = "/v1/explorer/transactions?authority";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_INSTRUCTIONS_AUTHORITY: &str = "/v1/explorer/instructions?authority";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_INSTRUCTIONS_ACCOUNT: &str = "/v1/explorer/instructions?account";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_ACCOUNT_DETAIL: &str = "/v1/explorer/accounts/{account_id}";
#[cfg(feature = "app_api")]
const CONTEXT_EXPLORER_ACCOUNT_QR: &str = "/v1/explorer/accounts/{account_id}/qr";
#[cfg(feature = "app_api")]
const CONTEXT_KAIGI_RELAY_EVENTS_QUERY: &str = "/v1/kaigi/relays/events?relay";

#[cfg(feature = "app_api")]
fn parse_account_id_for_endpoint(
    app: &AppState,
    literal: &str,
    endpoint: &'static str,
) -> Result<AccountId, Error> {
    routing::parse_account_path_segment(literal, &app.telemetry, endpoint)
        .map(|(account_id, _)| account_id)
}

#[cfg(feature = "app_api")]
fn parse_domain_id(raw: &str) -> Result<DomainId, Error> {
    raw.parse::<DomainId>()
        .map_err(|_| Error::Query(iroha_data_model::ValidationFail::TooComplex))
}

#[cfg(feature = "app_api")]
fn parse_asset_definition_id(raw: &str) -> Result<AssetDefinitionId, Error> {
    raw.parse::<AssetDefinitionId>()
        .map_err(|_| Error::Query(iroha_data_model::ValidationFail::TooComplex))
}

#[cfg(feature = "app_api")]
fn parse_asset_id(raw: &str) -> Result<AssetId, Error> {
    raw.parse::<AssetId>()
        .map_err(|_| Error::Query(iroha_data_model::ValidationFail::TooComplex))
}

#[cfg(feature = "app_api")]
fn parse_transaction_hash(raw: &str) -> Result<HashOf<TransactionEntrypoint>, Error> {
    raw.trim()
        .parse::<HashOf<TransactionEntrypoint>>()
        .map_err(|_| conversion_error("invalid transaction hash".to_owned()))
}

#[cfg(feature = "app_api")]
fn parse_nft_id(raw: &str) -> Result<NftId, Error> {
    raw.parse::<NftId>()
        .map_err(|_| Error::Query(iroha_data_model::ValidationFail::TooComplex))
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_accounts_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerAccountsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerAccountsQuery {
        pagination,
        domain,
        with_asset,
    } = query;
    let domain = match domain {
        Some(raw) => Some(parse_domain_id(&raw)?),
        None => None,
    };
    let asset_filter = match with_asset {
        Some(raw) => Some(parse_asset_definition_id(&raw)?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/accounts").await?;
    }
    routing::handle_v1_explorer_accounts(app.state.clone(), pagination, domain, asset_filter).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_domains_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerDomainsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerDomainsQuery {
        pagination,
        owned_by,
    } = query;
    let owned_by = match owned_by {
        Some(raw) => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_DOMAINS_OWNED_BY,
        )?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/domains").await?;
    }
    routing::handle_v1_explorer_domains(app.state.clone(), pagination, owned_by).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_asset_definitions_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerAssetDefinitionsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerAssetDefinitionsQuery {
        pagination,
        domain,
        owned_by,
    } = query;
    let domain = match domain {
        Some(raw) => Some(parse_domain_id(&raw)?),
        None => None,
    };
    let owned_by = match owned_by {
        Some(raw) => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_ASSET_DEFINITIONS_OWNED_BY,
        )?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/asset-definitions").await?;
    }
    routing::handle_v1_explorer_asset_definitions(app.state.clone(), pagination, domain, owned_by)
        .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_assets_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerAssetsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerAssetsQuery {
        pagination,
        owned_by,
        definition,
        asset_id,
    } = query;
    let owned_by = match owned_by {
        Some(raw) => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_ASSETS_OWNED_BY,
        )?),
        None => None,
    };
    let definition = match definition {
        Some(raw) => Some(parse_asset_definition_id(&raw)?),
        None => None,
    };
    let asset_id = match asset_id {
        Some(raw) => Some(parse_asset_id(&raw)?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/assets").await?;
    }
    routing::handle_v1_explorer_assets(
        app.state.clone(),
        pagination,
        owned_by,
        definition,
        asset_id,
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_nfts_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerNftsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerNftsQuery {
        pagination,
        owned_by,
        domain,
    } = query;
    let owned_by = match owned_by {
        Some(raw) => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_NFTS_OWNED_BY,
        )?),
        None => None,
    };
    let domain = match domain {
        Some(raw) => Some(parse_domain_id(&raw)?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/nfts").await?;
    }
    routing::handle_v1_explorer_nfts(app.state.clone(), pagination, owned_by, domain).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_blocks_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerPaginationOnly>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/blocks").await?;
    }
    routing::handle_v1_explorer_blocks(app.state.clone(), query.pagination).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_transactions_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerTransactionsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerTransactionsQuery {
        pagination,
        authority,
        block,
        status,
        asset_id,
    } = query;
    if let Some(block_height) = block {
        if block_height == 0 {
            return Err(conversion_error("block must be at least 1".to_owned()));
        }
    }
    let authority = match authority {
        Some(raw) => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_TRANSACTIONS_AUTHORITY,
        )?),
        None => None,
    };
    let status = match status {
        Some(raw) => Some(crate::routing::parse_transaction_status_filter(&raw)?),
        None => None,
    };
    let asset_id = match asset_id {
        Some(raw) if raw.trim().is_empty() => None,
        Some(raw) => Some(parse_asset_id(&raw)?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/transactions").await?;
    }
    crate::routing::handle_v1_explorer_transactions(
        app.state.clone(),
        app.telemetry.clone(),
        pagination,
        authority,
        block,
        status,
        asset_id,
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_instructions_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<ExplorerInstructionsQuery>,
) -> Result<AxResponse, Error> {
    let ExplorerInstructionsQuery {
        pagination,
        account,
        authority,
        transaction_hash,
        transaction_status,
        block,
        kind,
        asset_id,
    } = query;
    if let Some(block_height) = block {
        if block_height == 0 {
            return Err(conversion_error("block must be at least 1".to_owned()));
        }
    }
    let authority = match authority {
        Some(raw) if !raw.trim().is_empty() => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_INSTRUCTIONS_AUTHORITY,
        )?),
        _ => None,
    };
    let account = match account {
        Some(raw) if !raw.trim().is_empty() => Some(parse_account_id_for_endpoint(
            &app,
            &raw,
            CONTEXT_EXPLORER_INSTRUCTIONS_ACCOUNT,
        )?),
        _ => None,
    };
    let transaction_hash = match transaction_hash {
        Some(raw) if !raw.trim().is_empty() => Some(parse_transaction_hash(&raw)?),
        _ => None,
    };
    let status = match transaction_status {
        Some(raw) => Some(crate::routing::parse_transaction_status_filter(&raw)?),
        None => None,
    };
    let kind = match kind {
        Some(raw) if raw.trim().is_empty() => None,
        Some(raw) if raw.trim().eq_ignore_ascii_case("all") => None,
        Some(raw) => Some(
            raw.parse::<explorer::ExplorerInstructionKind>()
                .map_err(|_| Error::Query(iroha_data_model::ValidationFail::TooComplex))?,
        ),
        None => None,
    };
    let asset_id = match asset_id {
        Some(raw) if raw.trim().is_empty() => None,
        Some(raw) => Some(parse_asset_id(&raw)?),
        None => None,
    };
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/instructions").await?;
    }
    crate::routing::handle_v1_explorer_instructions(
        app.state.clone(),
        app.telemetry.clone(),
        pagination,
        crate::routing::ExplorerInstructionQuery {
            account,
            authority,
            transaction_hash,
            status,
            block,
            kind,
            asset_id,
        },
    )
    .await
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
#[axum::debug_handler]
async fn handler_explorer_metrics(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/metrics").await?;
    }
    routing::handle_v1_explorer_metrics(app.state.clone(), app.kura.clone(), app.telemetry.clone())
        .await
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
#[axum::debug_handler]
async fn handler_telemetry_peers_info(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/telemetry/peers-info").await?;
    }
    let peers = app.peer_telemetry.peers_info().await;
    Ok(JsonBody(peers).into_response())
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_account_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/accounts/{id}").await?;
    }
    let account_id =
        parse_account_id_for_endpoint(&app, &account_raw, CONTEXT_EXPLORER_ACCOUNT_DETAIL)?;
    routing::handle_v1_explorer_account_detail(app.state.clone(), account_id).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_account_qr(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(account_raw): AxPath<String>,
    AxQuery(query): AxQuery<explorer::ExplorerAddressFormatQuery>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/accounts/{account_id}/qr").await?;
    }
    let account_id =
        parse_account_id_for_endpoint(&app, &account_raw, CONTEXT_EXPLORER_ACCOUNT_QR)?;
    let address_format = query.address_format_pref()?;
    routing::handle_v1_explorer_account_qr(
        app.state.clone(),
        account_id,
        app.telemetry.clone(),
        address_format,
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_domain_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(domain_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/domains/{id}").await?;
    }
    let domain_id = parse_domain_id(&domain_raw)?;
    routing::handle_v1_explorer_domain_detail(app.state.clone(), domain_id).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_asset_definition_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(def_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/asset-definitions/{id}").await?;
    }
    let definition_id = parse_asset_definition_id(&def_raw)?;
    routing::handle_v1_explorer_asset_definition_detail(app.state.clone(), definition_id).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_asset_definition_econometrics(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(def_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(
            &app,
            &headers,
            None,
            "v1/explorer/asset-definitions/{id}/econometrics",
        )
        .await?;
    }
    let definition_id = parse_asset_definition_id(&def_raw)?;
    routing::handle_v1_explorer_asset_definition_econometrics(app.state.clone(), definition_id)
        .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_asset_definition_snapshot(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(def_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(
            &app,
            &headers,
            None,
            "v1/explorer/asset-definitions/{id}/snapshot",
        )
        .await?;
    }
    let definition_id = parse_asset_definition_id(&def_raw)?;
    routing::handle_v1_explorer_asset_definition_snapshot(app.state.clone(), definition_id).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_asset_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(asset_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/assets/{id}").await?;
    }
    let asset_id = parse_asset_id(&asset_raw)?;
    routing::handle_v1_explorer_asset_detail(app.state.clone(), asset_id).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_nft_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(nft_raw): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/nfts/{id}").await?;
    }
    let nft_id = parse_nft_id(&nft_raw)?;
    routing::handle_v1_explorer_nft_detail(app.state.clone(), nft_id).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_block_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(identifier): AxPath<String>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/blocks/{id}").await?;
    }
    routing::handle_v1_explorer_block_detail(app.state.clone(), identifier).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_transaction_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(hash): AxPath<String>,
    AxQuery(query): AxQuery<explorer::ExplorerAddressFormatQuery>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(&app, &headers, None, "v1/explorer/transactions/{hash}").await?;
    }
    let address_format = query.address_format_pref()?;
    crate::routing::handle_v1_explorer_transaction_detail(
        app.state.clone(),
        app.telemetry.clone(),
        hash,
        address_format,
    )
    .await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_explorer_instruction_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath((hash, index)): AxPath<(String, u64)>,
    AxQuery(query): AxQuery<explorer::ExplorerAddressFormatQuery>,
) -> Result<AxResponse, Error> {
    let allowed = limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    if !allowed {
        check_access(
            &app,
            &headers,
            None,
            "v1/explorer/instructions/{hash}/{index}",
        )
        .await?;
    }
    let address_format = query.address_format_pref()?;
    crate::routing::handle_v1_explorer_instruction_detail(
        app.state.clone(),
        app.telemetry.clone(),
        hash,
        index,
        address_format,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_assets_definitions_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::ListFilterParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_assets_definitions(app.state.clone(), AxQuery(p)).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/assets/definitions", enforce).await?;

    routing::handle_v1_assets_definitions(app.state.clone(), AxQuery(p)).await
}

#[cfg(feature = "app_api")]
async fn handler_assets_definitions_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_assets_definitions_query(
            app.state.clone(),
            crate::utils::extractors::NoritoJson(env),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/assets/definitions/query", enforce).await?;

    routing::handle_v1_assets_definitions_query(
        app.state.clone(),
        crate::utils::extractors::NoritoJson(env),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_asset_holders(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(def_id): AxPath<String>,
    AxQuery(p): AxQuery<routing::AssetHolderGetParams>,
) -> Result<impl IntoResponse, Error> {
    let limits = crate::routing::app_query_limits();
    let page_limit = limits.clamp_page_limit(p.limit)?;
    let mut p = p;
    p.limit = Some(page_limit);
    let query: AxQuery<routing::AssetHolderGetParams> = AxQuery(p.clone());

    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_asset_holders(
            app.state.clone(),
            AxPath(def_id),
            query.clone(),
            app.telemetry.clone(),
        )
        .await;
    }
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    let cost = limits.rate_limit_cost(page_limit);
    check_access_enforced_with_cost(&app, &headers, None, &def_id, enforce, cost).await?;
    routing::handle_v1_asset_holders(
        app.state.clone(),
        AxPath(def_id),
        query,
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_asset_holders_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(def_id): AxPath<String>,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    let limits = crate::routing::app_query_limits();
    let mut env = env;
    let page_limit = limits.clamp_page_limit(env.pagination.limit)?;
    env.pagination.limit = Some(page_limit);
    env.fetch_size = limits.clamp_fetch_size(env.fetch_size)?;
    let payload = crate::utils::extractors::NoritoJson(env);
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_asset_holders_query(
            app.state.clone(),
            AxPath(def_id),
            payload.clone(),
            app.telemetry.clone(),
        )
        .await;
    }
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    let cost = limits.rate_limit_cost(page_limit);
    check_access_enforced_with_cost(&app, &headers, None, &def_id, enforce, cost).await?;
    routing::handle_v1_asset_holders_query(
        app.state.clone(),
        AxPath(def_id),
        payload,
        app.telemetry.clone(),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_confidential_asset_transitions(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(def_id): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_confidential_asset_transitions(
            app.state.clone(),
            AxPath(def_id),
        )
        .await;
    }
    check_access(&app, &headers, None, &def_id).await?;
    routing::handle_v1_confidential_asset_transitions(app.state.clone(), AxPath(def_id)).await
}

#[cfg(feature = "app_api")]
async fn handler_domains_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::filter::Pagination>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_domains(app.state.clone(), AxQuery(p)).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/domains", enforce).await?;

    routing::handle_v1_domains(app.state.clone(), AxQuery(p)).await
}

#[cfg(feature = "app_api")]
async fn handler_domains_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<impl IntoResponse, Error> {
    let payload = crate::utils::extractors::NoritoJson(env);
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_domains_query(app.state.clone(), payload).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/domains/query", enforce).await?;

    routing::handle_v1_domains_query(app.state.clone(), payload).await
}

#[cfg(feature = "app_api")]
async fn handler_nfts_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::ListFilterParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_nfts(app.state.clone(), AxQuery(p)).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/nfts", enforce).await?;

    routing::handle_v1_nfts(app.state.clone(), AxQuery(p)).await
}

#[cfg(feature = "app_api")]
#[axum::debug_handler]
async fn handler_nfts_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(env): crate::utils::extractors::NoritoJson<
        crate::filter::QueryEnvelope,
    >,
) -> Result<axum::response::Response, Error> {
    let payload = crate::utils::extractors::NoritoJson(env);
    let response = if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        routing::handle_v1_nfts_query(app.state.clone(), payload).await?
    } else {
        let enforce =
            app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
        check_access_enforced(&app, &headers, None, "v1/nfts/query", enforce).await?;
        routing::handle_v1_nfts_query(app.state.clone(), payload).await?
    };

    Ok(response.into_response())
}

#[cfg(feature = "app_api")]
async fn handler_subscription_plans_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::SubscriptionPlanListParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_subscription_plans(app.state.clone(), AxQuery(p)).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/subscriptions/plans", enforce).await?;

    routing::handle_v1_subscription_plans(app.state.clone(), AxQuery(p)).await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_plans_create(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionPlanCreateDto,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_plan(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/subscriptions/plans", enforce).await?;

    routing::handle_post_v1_subscription_plan(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscriptions_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(p): AxQuery<crate::routing::SubscriptionListParams>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_subscriptions(app.state.clone(), AxQuery(p)).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/subscriptions", enforce).await?;

    routing::handle_v1_subscriptions(app.state.clone(), AxQuery(p)).await
}

#[cfg(feature = "app_api")]
async fn handler_subscriptions_create(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionCreateDto,
    >,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_create(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, None, "v1/subscriptions", enforce).await?;

    routing::handle_post_v1_subscription_create(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_v1_subscription_get(app.state.clone(), subscription_id).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}",
        enforce,
    )
    .await?;

    routing::handle_v1_subscription_get(app.state.clone(), subscription_id).await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_pause(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionActionDto,
    >,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_pause(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            subscription_id,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}/pause",
        enforce,
    )
    .await?;

    routing::handle_post_v1_subscription_pause(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        subscription_id,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_resume(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionActionDto,
    >,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_resume(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            subscription_id,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}/resume",
        enforce,
    )
    .await?;

    routing::handle_post_v1_subscription_resume(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        subscription_id,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_cancel(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionActionDto,
    >,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_cancel(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            subscription_id,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}/cancel",
        enforce,
    )
    .await?;

    routing::handle_post_v1_subscription_cancel(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        subscription_id,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_keep(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionActionDto,
    >,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_keep(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            subscription_id,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}/keep",
        enforce,
    )
    .await?;

    routing::handle_post_v1_subscription_keep(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        subscription_id,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_usage(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionUsageRequestDto,
    >,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_usage(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            subscription_id,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}/usage",
        enforce,
    )
    .await?;

    routing::handle_post_v1_subscription_usage(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        subscription_id,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_charge_now(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(subscription_raw): AxPath<String>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::SubscriptionActionDto,
    >,
) -> Result<impl IntoResponse, Error> {
    let subscription_id = parse_nft_id(&subscription_raw)?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return routing::handle_post_v1_subscription_charge_now(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            subscription_id,
            crate::utils::extractors::NoritoJson(req),
        )
        .await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        None,
        "v1/subscriptions/{subscription_id}/charge-now",
        enforce,
    )
    .await?;

    routing::handle_post_v1_subscription_charge_now(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        subscription_id,
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_parameters(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    if limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.allow_nets) {
        return routing::handle_v1_parameters(app.state.clone()).await;
    }

    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(&app, &headers, Some(remote_ip), "v1/parameters", enforce).await?;

    routing::handle_v1_parameters(app.state.clone()).await
}

#[cfg(feature = "app_api")]
async fn handler_webhooks_create(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::JsonOnly<crate::webhook::WebhookCreate>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(webhook::handle_create_webhook(body).await);
    }

    check_access_enforced(&app, &headers, None, "v1/webhooks", true).await?;

    Ok(webhook::handle_create_webhook(body).await)
}

#[cfg(feature = "app_api")]
async fn handler_webhooks_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(webhook::handle_list_webhooks().await);
    }

    check_access_enforced(&app, &headers, None, "v1/webhooks", true).await?;

    Ok(webhook::handle_list_webhooks().await)
}

#[cfg(feature = "app_api")]
async fn handler_webhooks_delete(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(id): AxPath<u64>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(webhook::handle_delete_webhook(axum::extract::Path(id)).await);
    }

    check_access_enforced(&app, &headers, None, "v1/webhooks", true).await?;

    Ok(webhook::handle_delete_webhook(axum::extract::Path(id)).await)
}

#[cfg(feature = "app_api")]
async fn handler_gov_ballot_zk(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::NoritoJson<crate::gov::ZkBallotDto>,
) -> Result<JsonBody<crate::gov::BallotSubmitResponse>, Error> {
    check_access_enforced(&app, &headers, None, "v1/gov/ballots/zk", true).await?;
    crate::gov::handle_gov_ballot_zk(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        body,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_gov_ballot_zk_v1(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::JsonOnly(value): crate::utils::extractors::JsonOnly<
        norito::json::Value,
    >,
) -> Result<JsonBody<crate::gov::BallotSubmitResponse>, Error> {
    #[cfg(feature = "zk-ballot")]
    {
        check_access_enforced(&app, &headers, None, "v1/gov/ballots/zk-v1", true).await?;
        let raw = norito::json::to_vec(&value).map_err(|e| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "bad json: {e}"
                )),
            ))
        })?;
        let dto: crate::gov::ZkBallotV1Dto = norito::json::from_value(value).map_err(|e| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "bad json: {e}"
                )),
            ))
        })?;
        crate::gov::handle_gov_ballot_zk_v1(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJsonWithBytes {
                value: dto,
                raw: Bytes::from(raw),
            },
        )
        .await
    }
    #[cfg(not(feature = "zk-ballot"))]
    {
        let _ = (app, headers, value);
        Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )))
    }
}

#[cfg(feature = "app_api")]
async fn handler_gov_ballot_zk_v1_ballot_proof(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::JsonOnly(value): crate::utils::extractors::JsonOnly<
        norito::json::Value,
    >,
) -> Result<JsonBody<crate::gov::BallotSubmitResponse>, Error> {
    #[cfg(feature = "zk-ballot")]
    {
        check_access_enforced(
            &app,
            &headers,
            None,
            "v1/gov/ballots/zk-v1/ballot-proof",
            true,
        )
        .await?;
        let raw = norito::json::to_vec(&value).map_err(|e| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "bad json: {e}"
                )),
            ))
        })?;
        let dto: crate::gov::ZkBallotV1BallotProofDto =
            norito::json::from_value(value).map_err(|e| {
                Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                        "bad json: {e}"
                    )),
                ))
            })?;
        crate::gov::handle_gov_ballot_zk_v1_ballotproof(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            crate::utils::extractors::NoritoJsonWithBytes {
                value: dto,
                raw: Bytes::from(raw),
            },
        )
        .await
    }
    #[cfg(not(feature = "zk-ballot"))]
    {
        let _ = (app, headers, value);
        Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )))
    }
}

#[cfg(feature = "app_api")]
async fn handler_gov_ballot_plain(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::NoritoJson<crate::gov::PlainBallotDto>,
) -> Result<JsonBody<crate::gov::BallotSubmitResponse>, Error> {
    check_access_enforced(&app, &headers, None, "v1/gov/ballots/plain", true).await?;
    crate::gov::handle_gov_ballot_plain_with_policy(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        body,
        app.telemetry.clone(),
    )
    .await
}

async fn handler_gov_finalize(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::NoritoJson<crate::gov::FinalizeDto>,
) -> Result<JsonBody<crate::gov::FinalizeResponse>, Error> {
    check_access_enforced(&app, &headers, None, "v1/gov/finalize", true).await?;
    crate::gov::handle_gov_finalize(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        body,
    )
    .await
}

#[allow(dead_code)]
fn _assert_finalize_send_bounds() {
    fn assert_send<T: Send>() {}
    assert_send::<JsonBody<crate::gov::FinalizeResponse>>();
    assert_send::<crate::gov::FinalizeResponse>();
    assert_send::<crate::gov::FinalizeDto>();
}

#[allow(dead_code)]
fn _assert_app_state_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<AppState>();
    assert_sync::<SharedAppState>();
}

#[cfg(feature = "app_api")]
async fn handler_gov_council_audit(
    State(app): State<SharedAppState>,
    AxQuery(q): AxQuery<crate::gov::CouncilAuditQuery>,
) -> Result<JsonBody<crate::gov::CouncilAuditResponse>, Error> {
    crate::gov::handle_gov_council_audit(app.state.clone(), AxQuery(q)).await
}

#[cfg(all(feature = "app_api", feature = "gov_vrf"))]
async fn handler_gov_council_persist(
    State(app): State<SharedAppState>,
    body: crate::utils::extractors::NoritoJson<crate::gov::CouncilPersistRequest>,
) -> Result<JsonBody<crate::gov::CouncilPersistResponse>, Error> {
    crate::gov::handle_gov_council_persist(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        body,
    )
    .await
}

#[cfg(all(feature = "app_api", feature = "gov_vrf"))]
async fn handler_gov_council_replace(
    State(app): State<SharedAppState>,
    body: crate::utils::extractors::NoritoJson<crate::gov::CouncilReplaceRequest>,
) -> Result<JsonBody<crate::gov::CouncilReplaceResponse>, Error> {
    crate::gov::handle_gov_council_replace(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        body,
    )
    .await
}

#[cfg(feature = "gov_vrf")]
async fn handler_gov_council_derive_vrf(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: crate::utils::extractors::NoritoJson<crate::gov::CouncilDeriveVrfRequest>,
) -> Result<crate::utils::JsonBody<crate::gov::CouncilDeriveVrfResponse>, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/gov/council/derive-vrf",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    check_access(&app, &headers, None, "v1/gov/council/derive-vrf").await?;
    crate::gov::handle_gov_council_derive_vrf(app.state.clone(), body).await
}

#[cfg(not(feature = "gov_vrf"))]
async fn handler_gov_council_derive_vrf(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    _body: crate::utils::extractors::NoritoJson<crate::gov::CouncilDeriveVrfRequest>,
) -> Result<AxResponse, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/gov/council/derive-vrf",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    check_access(&app, &headers, None, "v1/gov/council/derive-vrf").await?;
    Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(
            "not implemented".to_string(),
        ),
    )))
}

// -------------- Runtime (AppState-based) --------------

/// GET /v1/runtime/abi/active — wrapper that enforces Torii access policy, then delegates.
async fn handler_runtime_abi_active(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access(&app, &headers, None, "v1/runtime/abi/active").await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_abi_active(app.state.clone()).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/runtime/abi/hash — wrapper that enforces Torii access policy, then delegates.
async fn handler_runtime_abi_hash(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access(&app, &headers, None, "v1/runtime/abi/hash").await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_abi_hash(app.state.clone()).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

// -------------- Core info (AppState-based) --------------

/// GET /v1/configuration — wrapper that enforces Torii access policy, then delegates.
async fn handler_get_configuration(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    check_access(&app, &headers, None, "v1/configuration").await?;
    routing::handle_get_configuration(app.kiso.clone()).await
}

/// POST /v1/configuration — wrapper that enforces Torii access policy, then delegates.
async fn handler_post_configuration(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::JsonOnly(dto): crate::utils::extractors::JsonOnly<ConfigUpdateDTO>,
) -> Result<impl IntoResponse, Error> {
    check_access(&app, &headers, None, "v1/configuration").await?;
    routing::handle_post_configuration(app.kiso.clone(), dto).await
}

/// POST /v1/nexus/lifecycle — apply a lane lifecycle plan and reconfigure routing.
async fn handler_post_nexus_lane_lifecycle(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::NoritoJson(plan): crate::NoritoJson<routing::LaneLifecyclePlanDto>,
) -> Result<impl IntoResponse, Error> {
    check_access(&app, &headers, None, "v1/nexus/lifecycle").await?;
    routing::handle_post_nexus_lane_lifecycle(app.state.clone(), app.queue.clone(), plan).await
}

/// GET /v1/peers — wrapper that enforces Torii access policy, then delegates.
async fn handler_peers(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    check_access(&app, &headers, None, "v1/peers").await?;
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(routing::handle_peers(&app.online_peers, format))
}

/// GET /v1/health — wrapper that enforces Torii access policy, then delegates.
async fn handler_health(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    check_access(&app, &headers, Some(remote.ip()), "v1/health").await?;
    Ok(routing::handle_health().await.into_response())
}

async fn handler_version(State(app): State<SharedAppState>) -> impl IntoResponse {
    routing::handle_version(app.state.clone()).await
}

async fn handler_api_versions(State(app): State<SharedAppState>) -> impl IntoResponse {
    routing::handle_api_versions(&app.api_versions)
}

/// GET /v1/time/now — wrapper that enforces Torii access policy, then delegates.
async fn handler_time_now(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    check_access(&app, &headers, None, "v1/time/now").await?;
    Ok(routing::handle_time_now().await.into_response())
}

/// GET /v1/time/status — wrapper that enforces Torii access policy, then delegates.
async fn handler_time_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    check_access(&app, &headers, None, "v1/time/status").await?;
    Ok(routing::handle_time_status().await.into_response())
}

// -------------- Schema & Profiling (AppState-based) --------------

#[cfg(feature = "schema")]
async fn handler_schema(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.allow_nets) {
        return Ok(routing::handle_schema().await);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        "schema",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(routing::handle_schema().await)
}

#[cfg(feature = "profiling")]
static PROFILING_LOCK: std::sync::OnceLock<std::sync::Arc<tokio::sync::Mutex<()>>> =
    std::sync::OnceLock::new();

#[cfg(feature = "profiling")]
async fn handler_profile(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxQuery(params): AxQuery<routing::profiling::ProfileParams>,
) -> Result<Vec<u8>, Error> {
    let remote_ip = remote.ip();
    if !limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.allow_nets) {
        check_access(&app, &headers, Some(remote_ip), "debug/pprof/profile").await?;
    }
    let lock = PROFILING_LOCK
        .get_or_init(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone();
    routing::profiling::handle_profile(params, lock).await
}

/// GET /v1/runtime/metrics — wrapper enforcing access policy.
async fn handler_runtime_metrics(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access(&app, &headers, None, "v1/runtime/metrics").await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_metrics(app.state.clone()).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/node/capabilities — wrapper enforcing access policy.
async fn handler_node_capabilities(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access(&app, &headers, None, "v1/node/capabilities").await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_node_capabilities(app.state.clone()).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

async fn handler_runtime_upgrades_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access_enforced(&app, &headers, None, "v1/runtime/upgrades", true).await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_upgrades_list(app.state.clone()).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

async fn handler_runtime_propose_upgrade(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    body: crate::utils::extractors::NoritoJson<crate::runtime::ProposeUpgradeDto>,
) -> Result<Response, Error> {
    check_access_enforced(&app, &headers, None, "v1/runtime/upgrades/propose", true).await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_propose_upgrade(body).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

async fn handler_runtime_activate_upgrade(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: AxPath<String>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access_enforced(&app, &headers, None, "v1/runtime/upgrades/activate", true).await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_activate_upgrade(id).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

async fn handler_runtime_cancel_upgrade(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: AxPath<String>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    check_access_enforced(&app, &headers, None, "v1/runtime/upgrades/cancel", true).await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let payload = crate::runtime::handle_runtime_cancel_upgrade(id).await?;
    Ok(crate::utils::respond_with_format(payload, format))
}

async fn handler_zk_roots(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        routing::ZkRootsGetRequestDto,
    >,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/roots", true).await?;
    routing::handle_v1_zk_roots(
        app.state.clone(),
        accept.map(|value| value.0),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

async fn handler_zk_verify(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    let cost = (body.len() as u64)
        .saturating_div(4 * 1024)
        .saturating_add(1);
    ensure_proof_api_version(&app, negotiated, "v1/zk/verify")?;
    enforce_proof_body_limit(&app, body.len(), "v1/zk/verify")?;
    check_proof_access(&app, negotiated, &headers, None, "v1/zk/verify", cost, true).await?;
    routing::handle_v1_zk_verify(headers, body).await
}

async fn handler_zk_submit_proof(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    let cost = (body.len() as u64)
        .saturating_div(4 * 1024)
        .saturating_add(1);
    ensure_proof_api_version(&app, negotiated, "v1/zk/submit-proof")?;
    enforce_proof_body_limit(&app, body.len(), "v1/zk/submit-proof")?;
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/submit-proof",
        cost,
        true,
    )
    .await?;
    routing::handle_v1_zk_submit_proof(headers, body).await
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
/// Request body for `POST /v1/zk/ivm/derive`.
pub struct ZkIvmDeriveRequestDto {
    /// Verifying key reference used to select circuit parameters (gas schedule id, version).
    pub vk_ref: iroha_data_model::proof::VerifyingKeyId,
    /// Transaction authority used for admission-style binding and host context.
    pub authority: iroha_data_model::account::AccountId,
    /// Transaction metadata (must include `gas_limit`).
    #[norito(default)]
    pub metadata: iroha_data_model::metadata::Metadata,
    /// IVM bytecode to execute.
    pub bytecode: iroha_data_model::transaction::IvmBytecode,
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
/// Response body returned by `POST /v1/zk/ivm/derive`.
///
/// Note: this API does not expose plaintext gas usage; it returns commitments only.
pub struct ZkIvmDeriveResponseDto {
    /// Proved executable payload derived from local IVM execution.
    pub proved: iroha_data_model::transaction::IvmProved,
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
/// Request body for `POST /v1/zk/ivm/prove`.
pub struct ZkIvmProveRequestDto {
    /// Verifying key reference to use when producing the proof attachment.
    pub vk_ref: iroha_data_model::proof::VerifyingKeyId,
    /// Transaction authority used for admission-style binding and host context.
    pub authority: iroha_data_model::account::AccountId,
    /// Transaction metadata (must include `gas_limit`).
    #[norito(default)]
    pub metadata: iroha_data_model::metadata::Metadata,
    /// IVM bytecode to execute and prove.
    pub bytecode: iroha_data_model::transaction::IvmBytecode,
    /// Optional client-provided proved payload.
    ///
    /// When provided, Torii derives the authoritative payload from bytecode
    /// execution and rejects the request if this value does not match.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub proved: Option<iroha_data_model::transaction::IvmProved>,
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
/// Response body returned by `POST /v1/zk/ivm/prove` and `DELETE /v1/zk/ivm/prove/{job_id}`.
pub struct ZkIvmProveJobCreatedDto {
    /// Stable job identifier.
    pub job_id: String,
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
/// Response body returned by `GET /v1/zk/ivm/prove/{job_id}`.
///
/// Note: this API does not expose plaintext gas usage; it returns commitments only.
pub struct ZkIvmProveJobDto {
    /// Stable job identifier.
    pub job_id: String,
    /// Job status label (`pending`, `running`, `done`, `error`).
    pub status: String,
    /// Optional error message when status is `error`.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Echoed proved payload for convenience.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub proved: Option<iroha_data_model::transaction::IvmProved>,
    /// Proof attachment produced for the proved payload.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<iroha_data_model::proof::ProofAttachment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZkIvmProveJobStatus {
    Pending,
    Running,
    Done,
    Error,
}

#[derive(Clone)]
struct ZkIvmProveJobState {
    created_ms: u64,
    status: ZkIvmProveJobStatus,
    proved: Option<iroha_data_model::transaction::IvmProved>,
    vk_ref: iroha_data_model::proof::VerifyingKeyId,
    attachment: Option<iroha_data_model::proof::ProofAttachment>,
    error: Option<String>,
    cancel: tokio::sync::watch::Sender<bool>,
}

fn zk_ivm_prove_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn zk_ivm_prove_gc_jobs_at(
    jobs: &DashMap<String, ZkIvmProveJobState>,
    now_ms: u64,
    ttl_ms: u64,
    max_entries: usize,
) {
    if ttl_ms > 0 {
        let expire_before = now_ms.saturating_sub(ttl_ms);
        let expired = jobs
            .iter()
            .filter_map(|entry| {
                let state = entry.value();
                (state.created_ms < expire_before).then(|| entry.key().clone())
            })
            .collect::<Vec<_>>();
        for key in expired {
            if let Some((_key, state)) = jobs.remove(&key) {
                // Best-effort cancellation. Proving (spawn_blocking) is not preemptible, but
                // cancellation frees capacity permits once the async wrapper exits.
                let _ = state.cancel.send(true);
            }
        }
    }

    if max_entries > 0 {
        let len = jobs.len();
        if len > max_entries {
            let mut by_age = jobs
                .iter()
                .filter_map(|entry| {
                    let state = entry.value();
                    matches!(
                        state.status,
                        ZkIvmProveJobStatus::Done | ZkIvmProveJobStatus::Error
                    )
                    .then(|| (entry.key().clone(), state.created_ms))
                })
                .collect::<Vec<_>>();
            by_age.sort_by_key(|(_, created_ms)| *created_ms);
            for (key, _) in by_age.into_iter().take(len.saturating_sub(max_entries)) {
                jobs.remove(&key);
            }
        }
    }
}

fn zk_ivm_prove_gc_jobs(app: &AppState) {
    zk_ivm_prove_gc_jobs_at(
        app.zk_ivm_prove_jobs.as_ref(),
        zk_ivm_prove_now_ms(),
        app.zk_ivm_prove_job_ttl_ms,
        app.zk_ivm_prove_job_max_entries,
    );
}

fn zk_ivm_prove_observe_queue_metrics(
    telemetry: &crate::routing::MaybeTelemetry,
    slots: &tokio::sync::Semaphore,
    slots_total: usize,
    inflight: &tokio::sync::Semaphore,
    inflight_total: usize,
) {
    let used_slots = slots_total.saturating_sub(slots.available_permits());
    let used_inflight = inflight_total.saturating_sub(inflight.available_permits());
    let queued = used_slots.saturating_sub(used_inflight);
    telemetry.with_metrics(|tel| {
        tel.set_torii_zk_ivm_prove_inflight(used_inflight as u64);
        tel.set_torii_zk_ivm_prove_queued(queued as u64);
    });
}

fn zk_ivm_prove_status_label(status: ZkIvmProveJobStatus) -> &'static str {
    match status {
        ZkIvmProveJobStatus::Pending => "pending",
        ZkIvmProveJobStatus::Running => "running",
        ZkIvmProveJobStatus::Done => "done",
        ZkIvmProveJobStatus::Error => "error",
    }
}

fn normalize_halo2_ipa_circuit_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/ipa-v1/") {
        return (!rest.is_empty()).then(|| trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/") {
        return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa-v1/{rest}"));
    }
    if let Some(rest) = trimmed.strip_prefix(iroha_core::zk::ZK_BACKEND_HALO2_IPA) {
        if let Some(rest) = rest.strip_prefix("::") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa-v1/{rest}"));
        }
        if let Some(rest) = rest.strip_prefix(':') {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa-v1/{rest}"));
        }
        if let Some(rest) = rest.strip_prefix('/') {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa-v1/{rest}"));
        }
    }
    Some(format!("halo2/pasta/ipa-v1/{trimmed}"))
}

fn halo2_ipa_circuit_id_matches(record_id: &str, env_id: &str) -> bool {
    match (
        normalize_halo2_ipa_circuit_id(record_id),
        normalize_halo2_ipa_circuit_id(env_id),
    ) {
        (Some(rec), Some(env)) => rec == env,
        _ => record_id == env_id,
    }
}

fn normalize_stark_fri_circuit_id(backend: &str, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == backend {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix(backend) {
        if let Some(rest) = rest.strip_prefix(':') {
            return (!rest.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(rest) = rest.strip_prefix('/') {
            return (!rest.is_empty()).then(|| format!("{backend}:{rest}"));
        }
    }
    Some(format!("{backend}:{trimmed}"))
}

fn is_stark_fri_v1_backend(backend: &str) -> bool {
    backend == iroha_core::zk::ZK_BACKEND_STARK_FRI_V1 || backend.starts_with("stark/fri-v1/")
}

fn circuit_id_matches(backend: &str, record_id: &str, env_id: &str) -> bool {
    if backend == iroha_core::zk::ZK_BACKEND_HALO2_IPA {
        halo2_ipa_circuit_id_matches(record_id, env_id)
    } else if is_stark_fri_v1_backend(backend) {
        match (
            normalize_stark_fri_circuit_id(backend, record_id),
            normalize_stark_fri_circuit_id(backend, env_id),
        ) {
            (Some(rec), Some(env)) => rec == env,
            _ => record_id == env_id,
        }
    } else {
        record_id == env_id
    }
}

fn sanitize_zk_key_component(component: &str) -> String {
    let mut out = String::with_capacity(component.len());
    for ch in component.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}

fn zk_vk_store_path(keys_dir: &Path, id: &iroha_data_model::proof::VerifyingKeyId) -> PathBuf {
    let backend = sanitize_zk_key_component(id.backend.as_ref());
    let name = sanitize_zk_key_component(&id.name);
    keys_dir.join(format!("{backend}__{name}.vk"))
}

fn zk_pk_store_path(keys_dir: &Path, id: &iroha_data_model::proof::VerifyingKeyId) -> PathBuf {
    let backend = sanitize_zk_key_component(id.backend.as_ref());
    let name = sanitize_zk_key_component(&id.name);
    keys_dir.join(format!("{backend}__{name}.pk"))
}

async fn handler_zk_ivm_derive(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    let cost = (body.len() as u64)
        .saturating_div(4 * 1024)
        .saturating_add(1);
    ensure_proof_api_version(&app, negotiated, "v1/zk/ivm/derive")?;
    enforce_proof_body_limit(&app, body.len(), "v1/zk/ivm/derive")?;
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/ivm/derive",
        cost,
        true,
    )
    .await?;

    let req: ZkIvmDeriveRequestDto = norito::decode_from_bytes(body.as_ref()).or_else(|_| {
        norito::json::from_slice(body.as_ref()).map_err(|err| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "invalid derive request body: {err}"
                )),
            ))
        })
    })?;

    let backend = req.vk_ref.backend.as_str();
    if !iroha_core::zk::is_ivm_execution_backend(backend) {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "ivm derive requires vk_ref.backend == `halo2/ipa` or `stark/fri-v1`".to_owned(),
            ),
        )));
    }

    let parsed = ivm::ProgramMetadata::parse(req.bytecode.as_ref()).map_err(|_| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "invalid IVM header".to_owned(),
            ),
        ))
    })?;
    if parsed.metadata.mode & ivm::ivm_mode::ZK == 0 {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "ivm derive requires bytecode ZK mode bit (mode & ZK != 0)".to_owned(),
            ),
        )));
    }

    let vk_record = {
        // Ensure the view guard does not live across `.await` points below.
        let world = app.state.world_view();
        world
            .verifying_keys()
            .get(&req.vk_ref)
            .cloned()
            .ok_or_else(|| {
                Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                        "verifying key not found: {}::{}",
                        req.vk_ref.backend, req.vk_ref.name
                    )),
                ))
            })?
    };

    if vk_record.status != iroha_data_model::confidential::ConfidentialStatus::Active {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "verifying key is not Active".to_owned(),
            ),
        )));
    }
    if !circuit_id_matches(
        backend,
        &vk_record.circuit_id,
        iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
    ) {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "verifying key circuit_id is not compatible with `ivm-execution-v1` for backend `{backend}` (got `{}`)",
                vk_record.circuit_id,
            )),
        )));
    }
    let expected_schema_hash = iroha_core::zk::ivm_execution_public_inputs_schema_hash();
    if vk_record.public_inputs_schema_hash != expected_schema_hash {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "verifying key schema hash is not compatible with `ivm-execution-v1`".to_owned(),
            ),
        )));
    }
    if vk_record.gas_schedule_id.as_deref().is_none() {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "verifying key missing gas_schedule_id".to_owned(),
            ),
        )));
    }

    let chain_id = app.chain_id.as_ref().clone();
    let state = Arc::clone(&app.state);
    let authority = req.authority;
    let metadata = req.metadata;
    let bytecode = req.bytecode;
    let proved = tokio::task::spawn_blocking(
        move || -> Result<iroha_data_model::transaction::IvmProved, String> {
            let kp = iroha_crypto::KeyPair::random();
            let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
                chain_id,
                authority.clone(),
            )
            .with_metadata(metadata)
            .with_executable(iroha_data_model::transaction::Executable::Ivm(bytecode))
            .sign(kp.private_key())
            // Proof derivation needs a stable authority, but signature validity is not required here.
            .with_authority(authority);

            let view = state.query_view();
            iroha_core::pipeline::overlay::derive_ivm_proved_payload_from_ivm_execution(
                &view, &tx, &vk_record,
            )
            .map_err(|err| err.to_string())
        },
    )
    .await
    .map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "derive task panicked: {err}"
            )),
        ))
    })?
    .map_err(|msg| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(msg),
        ))
    })?;

    Ok(crate::utils::respond_with_format(
        ZkIvmDeriveResponseDto { proved },
        crate::utils::ResponseFormat::Json,
    ))
}

async fn handler_zk_ivm_prove(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    zk_ivm_prove_gc_jobs(&app);
    let cost = (body.len() as u64)
        .saturating_div(4 * 1024)
        .saturating_add(1);
    ensure_proof_api_version(&app, negotiated, "v1/zk/ivm/prove")?;
    enforce_proof_body_limit(&app, body.len(), "v1/zk/ivm/prove")?;
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/ivm/prove",
        cost,
        true,
    )
    .await?;

    let req: ZkIvmProveRequestDto = norito::decode_from_bytes(body.as_ref()).or_else(|_| {
        norito::json::from_slice(body.as_ref()).map_err(|err| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "invalid prove request body: {err}"
                )),
            ))
        })
    })?;

    let backend = req.vk_ref.backend.as_str();
    if !iroha_core::zk::is_ivm_execution_backend(backend) {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "ivm prove requires vk_ref.backend == `halo2/ipa` or `stark/fri-v1`".to_owned(),
            ),
        )));
    }

    let world = app.state.world_view();
    let vk_record = world
        .verifying_keys()
        .get(&req.vk_ref)
        .cloned()
        .ok_or_else(|| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "verifying key not found: {}::{}",
                    req.vk_ref.backend, req.vk_ref.name
                )),
            ))
        })?;

    if vk_record.status != iroha_data_model::confidential::ConfidentialStatus::Active {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "verifying key is not Active".to_owned(),
            ),
        )));
    }
    let keys_dir = app.zk_prover_keys_dir.clone();
    if !circuit_id_matches(
        backend,
        &vk_record.circuit_id,
        iroha_core::zk::IVM_EXECUTION_V1_CIRCUIT_ID,
    ) {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "verifying key circuit_id is not compatible with `ivm-execution-v1` for backend `{backend}` (got `{}`)",
                vk_record.circuit_id,
            )),
        )));
    }
    let expected_schema_hash = iroha_core::zk::ivm_execution_public_inputs_schema_hash();
    if vk_record.public_inputs_schema_hash != expected_schema_hash {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "verifying key schema hash is not compatible with `ivm-execution-v1`".to_owned(),
            ),
        )));
    }
    let vk_box = if let Some(vk_box) = vk_record.key.as_ref() {
        if vk_box.backend.as_str() != req.vk_ref.backend.as_str() {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "verifying key backend does not match vk_ref backend".to_owned(),
                ),
            )));
        }
        vk_box.clone()
    } else {
        let path = zk_vk_store_path(&keys_dir, &req.vk_ref);
        let bytes = std::fs::read(&path).map_err(|err| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "failed to read verifying key bytes at {}: {err}",
                    path.display()
                )),
            ))
        })?;
        iroha_data_model::proof::VerifyingKeyBox::new(req.vk_ref.backend.as_str().to_owned(), bytes)
    };
    if vk_record.vk_len > 0 && vk_box.bytes.len() != vk_record.vk_len as usize {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "verifying key length {} does not match registry vk_len {}",
                vk_box.bytes.len(),
                vk_record.vk_len
            )),
        )));
    }
    let computed_commitment = iroha_core::zk::hash_vk(&vk_box);
    if computed_commitment != vk_record.commitment {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "verifying key commitment mismatch".to_owned(),
            ),
        )));
    }

    let retry_after_secs = app.proof_limits.retry_after.as_secs().max(1);
    let slot_permit = app
        .zk_ivm_prove_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| Error::ProofRateLimited {
            endpoint: "v1/zk/ivm/prove",
            retry_after_secs,
        })?;

    let job_id_bytes: [u8; 16] = rand::random();
    let job_id = hex::encode(job_id_bytes);
    let created_ms = zk_ivm_prove_now_ms();

    let vk_ref = req.vk_ref.clone();
    let authority = req.authority.clone();
    let metadata = req.metadata.clone();
    let bytecode = req.bytecode.clone();
    let maybe_client_proved = req.proved.clone();
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    app.zk_ivm_prove_jobs.insert(
        job_id.clone(),
        ZkIvmProveJobState {
            created_ms,
            status: ZkIvmProveJobStatus::Pending,
            proved: None,
            vk_ref,
            attachment: None,
            error: None,
            cancel: cancel_tx,
        },
    );
    zk_ivm_prove_observe_queue_metrics(
        &app.telemetry,
        app.zk_ivm_prove_slots.as_ref(),
        app.zk_ivm_prove_slots_total,
        app.zk_ivm_prove_inflight.as_ref(),
        app.zk_ivm_prove_inflight_total,
    );

    let circuit_id = vk_record.circuit_id.clone();
    let max_proof_bytes = vk_record.max_proof_bytes;
    let backend = req.vk_ref.backend.clone();
    let vk_ref = req.vk_ref;
    let chain_id = app.chain_id.as_ref().clone();
    let state = Arc::clone(&app.state);
    let job_id_for_task = job_id.clone();
    let telemetry = app.telemetry.clone();
    let slots = app.zk_ivm_prove_slots.clone();
    let slots_total = app.zk_ivm_prove_slots_total;
    let inflight = app.zk_ivm_prove_inflight.clone();
    let inflight_total = app.zk_ivm_prove_inflight_total;
    let jobs = app.zk_ivm_prove_jobs.clone();
    tokio::spawn(async move {
        let mut cancel_rx = cancel_rx;
        let slot_permit = slot_permit;
        if *cancel_rx.borrow() {
            drop(slot_permit);
            zk_ivm_prove_observe_queue_metrics(
                &telemetry,
                slots.as_ref(),
                slots_total,
                inflight.as_ref(),
                inflight_total,
            );
            return;
        }

        let Some(inflight_permit) = (tokio::select! {
            permit = inflight.clone().acquire_owned() => match permit {
                Ok(permit) => Some(permit),
                Err(_) => None,
            },
            _ = cancel_rx.changed() => None,
        }) else {
            drop(slot_permit);
            zk_ivm_prove_observe_queue_metrics(
                &telemetry,
                slots.as_ref(),
                slots_total,
                inflight.as_ref(),
                inflight_total,
            );
            return;
        };

        if let Some(mut entry) = jobs.get_mut(&job_id_for_task) {
            entry.status = ZkIvmProveJobStatus::Running;
        }
        zk_ivm_prove_observe_queue_metrics(
            &telemetry,
            slots.as_ref(),
            slots_total,
            inflight.as_ref(),
            inflight_total,
        );

        let prove_job = tokio::task::spawn_blocking(move || {
            (|| -> Result<(iroha_data_model::transaction::IvmProved, iroha_data_model::proof::ProofAttachment), String> {
                let parsed = ivm::ProgramMetadata::parse(bytecode.as_ref())
                    .map_err(|_| "invalid IVM header".to_owned())?;
                if parsed.metadata.mode & ivm::ivm_mode::ZK == 0 {
                    return Err(
                        "ivm prove requires bytecode ZK mode bit (mode & ZK != 0)".to_owned()
                    );
                }
                let body = bytecode
                    .as_ref()
                    .get(parsed.header_len..)
                    .ok_or_else(|| "invalid IVM header (missing code body)".to_owned())?;
                let code_hash = iroha_crypto::Hash::new(body);
                let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
                    chain_id,
                    authority.clone(),
                )
                .with_metadata(metadata.clone())
                .with_executable(iroha_data_model::transaction::Executable::Ivm(bytecode.clone()))
                .sign(iroha_crypto::KeyPair::random().private_key())
                // Proof derivation needs stable authority; signature validity is not required.
                .with_authority(authority.clone());

                let view = state.query_view();
                let derived_proved =
                    iroha_core::pipeline::overlay::derive_ivm_proved_payload_from_ivm_execution(
                        &view, &tx, &vk_record,
                    )
                    .map_err(|err| err.to_string())?;

                if let Some(client_proved) = maybe_client_proved.as_ref()
                    && client_proved != &derived_proved
                {
                    return Err(
                        "provided `proved` payload does not match node-derived execution payload"
                            .to_owned(),
                    );
                }

                let overlay_bytes = norito::to_bytes(&derived_proved.overlay)
                    .map_err(|_| "failed to encode derived proved overlay".to_owned())?;
                let overlay_hash = iroha_crypto::Hash::new(&overlay_bytes);

                let proof_box = if backend.as_str() == iroha_core::zk::ZK_BACKEND_HALO2_IPA {
                    let pk_path = zk_pk_store_path(&keys_dir, &vk_ref);
                    let pk_bytes = std::fs::read(&pk_path).map_err(|err| {
                        format!(
                            "failed to read proving key bytes at {}: {err}",
                            pk_path.display()
                        )
                    })?;
                    iroha_core::zk::prove_halo2_ipa_ivm_execution_envelope(
                        circuit_id.as_str(),
                        &vk_box,
                        code_hash,
                        overlay_hash,
                        derived_proved.events_commitment,
                        derived_proved.gas_policy_commitment,
                        Some(pk_bytes.as_slice()),
                    )?
                } else if is_stark_fri_v1_backend(backend.as_str()) {
                    #[cfg(feature = "zk-stark")]
                    {
                        iroha_core::zk::prove_stark_fri_ivm_execution_envelope(
                            backend.as_str(),
                            circuit_id.as_str(),
                            &vk_box,
                            code_hash,
                            overlay_hash,
                            derived_proved.events_commitment,
                            derived_proved.gas_policy_commitment,
                        )?
                    }
                    #[cfg(not(feature = "zk-stark"))]
                    {
                        return Err(
                            "stark/fri-v1 prove requested but binary lacks `zk-stark`".to_owned()
                        );
                    }
                } else {
                    return Err("unsupported backend for ivm prove".to_owned());
                };
                if max_proof_bytes > 0 && proof_box.bytes.len() > max_proof_bytes as usize {
                    return Err("generated proof exceeds verifying key max_proof_bytes".to_owned());
                }
                let attachment =
                    iroha_data_model::proof::ProofAttachment::new_ref(backend.clone(), proof_box, vk_ref);
                Ok((derived_proved, attachment))
            })()
        });

        let outcome = tokio::select! {
            res = prove_job => match res {
                Ok(outcome) => outcome,
                Err(err) => Err(format!("prove job panicked: {err}")),
            },
            _ = cancel_rx.changed() => {
                drop(inflight_permit);
                drop(slot_permit);
                zk_ivm_prove_observe_queue_metrics(
                    &telemetry,
                    slots.as_ref(),
                    slots_total,
                    inflight.as_ref(),
                    inflight_total,
                );
                return;
            }
        };

        if let Some(mut entry) = jobs.get_mut(&job_id_for_task) {
            match outcome {
                Ok((proved, attachment)) => {
                    entry.status = ZkIvmProveJobStatus::Done;
                    entry.proved = Some(proved);
                    entry.attachment = Some(attachment);
                    entry.error = None;
                }
                Err(err) => {
                    entry.status = ZkIvmProveJobStatus::Error;
                    entry.proved = None;
                    entry.attachment = None;
                    entry.error = Some(err);
                }
            }
        }

        drop(inflight_permit);
        drop(slot_permit);
        zk_ivm_prove_observe_queue_metrics(
            &telemetry,
            slots.as_ref(),
            slots_total,
            inflight.as_ref(),
            inflight_total,
        );
    });

    Ok(crate::utils::respond_with_format(
        ZkIvmProveJobCreatedDto { job_id },
        crate::utils::ResponseFormat::Json,
    ))
}

async fn handler_zk_ivm_prove_get(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    job_id: axum::extract::Path<String>,
) -> Result<impl IntoResponse, Error> {
    zk_ivm_prove_gc_jobs(&app);
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/ivm/prove/{job_id}",
        1,
        true,
    )
    .await?;

    let job_id = job_id.0;
    let Some(entry) = app.zk_ivm_prove_jobs.get(&job_id) else {
        return Ok((
            StatusCode::NOT_FOUND,
            "prove job not found; submit a new job via POST /v1/zk/ivm/prove",
        )
            .into_response());
    };

    let status = zk_ivm_prove_status_label(entry.status).to_owned();
    let proved = entry.proved.clone();
    let attachment = entry.attachment.clone();
    let error = entry.error.clone();

    Ok(crate::utils::respond_with_format(
        ZkIvmProveJobDto {
            job_id,
            status,
            error,
            proved,
            attachment,
        },
        crate::utils::ResponseFormat::Json,
    ))
}

async fn handler_zk_ivm_prove_delete(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    job_id: axum::extract::Path<String>,
) -> Result<impl IntoResponse, Error> {
    zk_ivm_prove_gc_jobs(&app);
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/ivm/prove/{job_id}",
        1,
        true,
    )
    .await?;

    let job_id = job_id.0;
    if let Some((_key, state)) = app.zk_ivm_prove_jobs.remove(&job_id) {
        let _ = state.cancel.send(true);
    }
    Ok(crate::utils::respond_with_format(
        ZkIvmProveJobCreatedDto { job_id },
        crate::utils::ResponseFormat::Json,
    ))
}

fn zk_attachments_tenant(
    app: &SharedAppState,
    headers: &axum::http::HeaderMap,
) -> crate::zk_attachments::AttachmentTenant {
    if let Some(token) = headers.get("x-api-token").and_then(|v| v.to_str().ok()) {
        if app.api_tokens_set.contains(token) {
            return crate::zk_attachments::AttachmentTenant::from_api_token(token);
        }
    }
    if let Some(ip) = headers
        .get(limits::REMOTE_ADDR_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
    {
        return crate::zk_attachments::AttachmentTenant::from_remote_ip(ip);
    }
    crate::zk_attachments::AttachmentTenant::anonymous()
}

async fn handler_zk_attachments_create(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/attachments", true).await?;
    let tenant = zk_attachments_tenant(&app, &headers);
    Ok(crate::zk_attachments::handle_post_attachment(tenant, headers, body).await)
}

async fn handler_zk_attachments_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/attachments", true).await?;
    let tenant = zk_attachments_tenant(&app, &headers);
    Ok(crate::zk_attachments::handle_list_attachments(tenant).await)
}

async fn handler_zk_attachments_filtered(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(q): AxQuery<crate::zk_attachments::AttachmentListQuery>,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/attachments", true).await?;
    let tenant = zk_attachments_tenant(&app, &headers);
    Ok(crate::zk_attachments::handle_list_attachments_filtered(tenant, AxQuery(q)).await)
}

async fn handler_zk_attachments_count(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(q): AxQuery<crate::zk_attachments::AttachmentListQuery>,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/attachments/count", true).await?;
    let tenant = zk_attachments_tenant(&app, &headers);
    Ok(crate::zk_attachments::handle_count_attachments(tenant, AxQuery(q)).await)
}

async fn handler_zk_attachment_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: axum::extract::Path<String>,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/attachments/{id}", true).await?;
    let tenant = zk_attachments_tenant(&app, &headers);
    Ok(crate::zk_attachments::handle_get_attachment(tenant, id).await)
}

async fn handler_zk_attachment_delete(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    id: axum::extract::Path<String>,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/attachments/{id}", true).await?;
    let tenant = zk_attachments_tenant(&app, &headers);
    Ok(crate::zk_attachments::handle_delete_attachment(tenant, id).await)
}

async fn handler_zk_vote_tally(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    crate::utils::extractors::NoritoJson(req): crate::utils::extractors::NoritoJson<
        crate::routing::ZkVoteGetTallyRequestDto,
    >,
) -> Result<impl IntoResponse, Error> {
    check_access_enforced(&app, &headers, None, "v1/zk/vote/tally", true).await?;
    routing::handle_v1_zk_vote_tally(
        State(app.state.clone()),
        accept.map(|value| value.0),
        crate::utils::extractors::NoritoJson(req),
    )
    .await
}

// -------------- Telemetry (AppState-based) --------------
#[cfg(feature = "telemetry")]
async fn handler_status_tail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
    AxPath(tail): AxPath<String>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    // Allowlist bypass
    if limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.allow_nets) {
        return routing::handle_status(
            &app.telemetry,
            accept.map(|e| e.0),
            Some(&tail),
            nexus_enabled,
        )
        .await;
    }
    // Token gate
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    // Conditional rate limiting based on load/fees
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        "status",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if enforce && !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    routing::handle_status(
        &app.telemetry,
        accept.map(|e| e.0),
        Some(&tail),
        nexus_enabled,
    )
    .await
}

#[cfg(feature = "telemetry")]
async fn handler_status_root(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    if limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.allow_nets) {
        return routing::handle_status(&app.telemetry, accept.map(|e| e.0), None, nexus_enabled)
            .await;
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        "status",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if enforce && !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    routing::handle_status(&app.telemetry, accept.map(|e| e.0), None, nexus_enabled).await
}

#[cfg(feature = "telemetry")]
async fn handler_metrics(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<String, Error> {
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    if limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.allow_nets) {
        return routing::handle_metrics(&app.telemetry, nexus_enabled).await;
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        "metrics",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    routing::handle_metrics(&app.telemetry, nexus_enabled).await
}

#[cfg(feature = "telemetry")]
async fn handler_soracloud_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }

    let key = rate_limit_key(
        &headers,
        None,
        "v1/soracloud/status",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    if !app.telemetry.allows_metrics() {
        return Ok(telemetry_unavailable_response(
            "/v1/soracloud/status",
            &app.telemetry,
        ));
    }

    let format =
        match crate::utils::negotiate_response_format(accept.as_ref().map(|value| &value.0)) {
            Ok(format) => format,
            Err(response) => return Ok(response),
        };

    let metrics = app.telemetry.metrics().await;
    let governance_manifest_rejected: u64 = [
        "missing_manifest",
        "non_validator_authority",
        "quorum_rejected",
        "protected_namespace_rejected",
        "runtime_hook_rejected",
        "uaid_not_bound",
    ]
    .into_iter()
    .map(|reason| {
        metrics
            .governance_manifest_admission_total
            .with_label_values(&[reason])
            .get()
    })
    .sum();

    let sorafs_provider_rejected: u64 = [
        "decode",
        "stale",
        "policy_violation",
        "unsupported_signature",
        "signature",
        "unknown_capabilities",
        "admission_missing",
        "digest_error",
        "body_mismatch",
        "body_digest_mismatch",
        "advert_key_mismatch",
        "retention_expired",
    ]
    .into_iter()
    .map(|reason| {
        metrics
            .torii_sorafs_admission_total
            .with_label_values(&["rejected", reason])
            .get()
    })
    .sum();
    let failed_admission_total =
        governance_manifest_rejected.saturating_add(sorafs_provider_rejected);

    let backpressure = app.queue.current_backpressure();
    let queue_active = u64::try_from(app.queue.active_len()).unwrap_or(u64::MAX);
    let queue_queued = u64::try_from(backpressure.queued()).unwrap_or(u64::MAX);
    let queue_capacity = u64::try_from(backpressure.capacity().get()).unwrap_or(u64::MAX);
    let high_load_threshold = u64::try_from(app.high_load_tx_threshold).unwrap_or(u64::MAX);
    let high_load = backpressure.is_saturated() || queue_active >= high_load_threshold;

    let nexus = app.state.nexus_snapshot();
    let routing = json_object(vec![
        json_entry("nexus_enabled", nexus.enabled),
        json_entry(
            "lane_count",
            u64::try_from(nexus.lane_catalog.lanes().len()).unwrap_or(u64::MAX),
        ),
        json_entry(
            "dataspace_count",
            u64::try_from(nexus.dataspace_catalog.entries().len()).unwrap_or(u64::MAX),
        ),
        json_entry(
            "routing_rules",
            u64::try_from(nexus.routing_policy.rules.len()).unwrap_or(u64::MAX),
        ),
        json_entry(
            "default_lane_id",
            nexus.routing_policy.default_lane.as_u32(),
        ),
        json_entry(
            "default_dataspace_id",
            nexus.routing_policy.default_dataspace.as_u64(),
        ),
    ]);

    #[cfg(feature = "app_api")]
    let (service_health, storage_pressure) = {
        if app.sorafs_node.is_enabled() {
            let schedulers = app.sorafs_node.schedulers();
            let telemetry = schedulers.telemetry_snapshot();
            let utilisation = schedulers.utilisation_snapshot();
            let degraded = utilisation.fetch_utilisation_bps >= 9_000
                || utilisation.pin_queue_utilisation_bps >= 9_000
                || utilisation.por_utilisation_bps >= 9_000
                || telemetry.por_samples_failed > 0;
            let status = if degraded { "degraded" } else { "healthy" };
            (
                json_object(vec![
                    json_entry("mode", "sorafs_runtime"),
                    json_entry("status", status),
                    json_entry(
                        "message",
                        "runtime host integration pending; exposing SoraFS health as baseline",
                    ),
                ]),
                json_object(vec![
                    json_entry("enabled", true),
                    json_entry("bytes_used", telemetry.bytes_used),
                    json_entry("bytes_capacity", telemetry.bytes_capacity),
                    json_entry(
                        "pin_queue_depth",
                        u64::try_from(telemetry.pin_queue_depth).unwrap_or(u64::MAX),
                    ),
                    json_entry(
                        "fetch_inflight",
                        u64::try_from(telemetry.fetch_inflight).unwrap_or(u64::MAX),
                    ),
                    json_entry(
                        "por_inflight",
                        u64::try_from(telemetry.por_inflight).unwrap_or(u64::MAX),
                    ),
                    json_entry("por_samples_failed_total", telemetry.por_samples_failed),
                    json_entry("fetch_utilisation_bps", utilisation.fetch_utilisation_bps),
                    json_entry(
                        "pin_queue_utilisation_bps",
                        utilisation.pin_queue_utilisation_bps,
                    ),
                    json_entry("por_utilisation_bps", utilisation.por_utilisation_bps),
                ]),
            )
        } else {
            (
                json_object(vec![
                    json_entry("mode", "local_only"),
                    json_entry("status", "not_configured"),
                    json_entry(
                        "message",
                        "SoraFS node runtime is disabled; no hosted service health is available",
                    ),
                ]),
                json_object(vec![json_entry("enabled", false)]),
            )
        }
    };

    #[cfg(not(feature = "app_api"))]
    let (service_health, storage_pressure) = (
        json_object(vec![
            json_entry("mode", "app_api_disabled"),
            json_entry("status", "unavailable"),
            json_entry(
                "message",
                "binary compiled without app_api; Soracloud runtime health unavailable",
            ),
        ]),
        json_object(vec![json_entry("enabled", false)]),
    );

    let resource_pressure = json_object(vec![
        json_entry("queue_active", queue_active),
        json_entry("queue_queued", queue_queued),
        json_entry("queue_capacity", queue_capacity),
        json_entry("queue_saturated", backpressure.is_saturated()),
        json_entry("high_load_threshold", high_load_threshold),
        json_entry("high_load", high_load),
        json_entry("storage", storage_pressure),
    ]);
    let failed_admissions = json_object(vec![
        json_entry("total", failed_admission_total),
        json_entry("governance_manifest_rejected", governance_manifest_rejected),
        json_entry("sorafs_provider_rejected", sorafs_provider_rejected),
    ]);

    #[cfg(feature = "app_api")]
    let control_plane = app.soracloud_registry.snapshot(None, 10).await;

    #[cfg(feature = "app_api")]
    let payload = json_object(vec![
        json_entry("schema_version", 1_u16),
        json_entry("service_health", service_health),
        json_entry("routing", routing),
        json_entry("resource_pressure", resource_pressure),
        json_entry("failed_admissions", failed_admissions),
        json_entry("control_plane", json_value(&control_plane)),
    ]);

    #[cfg(not(feature = "app_api"))]
    let payload = json_object(vec![
        json_entry("schema_version", 1_u16),
        json_entry("service_health", service_health),
        json_entry("routing", routing),
        json_entry("resource_pressure", resource_pressure),
        json_entry("failed_admissions", failed_admissions),
    ]);

    Ok(crate::utils::respond_value_with_format(payload, format))
}

#[cfg(feature = "telemetry")]
async fn handler_post_soranet_privacy_event(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: crate::NoritoJson<RecordSoranetPrivacyEventDto>,
) -> Result<AxResponse, Error> {
    if let Err(resp) =
        enforce_soranet_privacy_ingest(&app, &headers, Some(remote.ip()), "event").await
    {
        return Ok(resp);
    }
    routing::handle_post_soranet_privacy_event(app.telemetry.clone(), request)
        .await
        .map(IntoResponse::into_response)
}

#[cfg(feature = "telemetry")]
async fn handler_post_soranet_privacy_share(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: crate::NoritoJson<RecordSoranetPrivacyShareDto>,
) -> Result<AxResponse, Error> {
    if let Err(resp) =
        enforce_soranet_privacy_ingest(&app, &headers, Some(remote.ip()), "share").await
    {
        return Ok(resp);
    }
    routing::handle_post_soranet_privacy_share(app.telemetry.clone(), request)
        .await
        .map(IntoResponse::into_response)
}

// -------------- Telemetry: collectors + new_view SSE/JSON --------------
#[cfg(feature = "telemetry")]
async fn handler_sumeragi_collectors(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/collectors",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_collectors(State(app.state.clone()), accept).await
}

#[cfg(feature = "telemetry")]
async fn handler_new_view_sse(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/new_view/sse",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/new_view/sse",
            &app.telemetry,
        ));
    }
    Ok(handle_v1_new_view_sse(1_000).into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_new_view_json(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/new_view/json",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/new_view/json",
            &app.telemetry,
        ));
    }
    routing::handle_v1_new_view_json()
        .await
        .map(axum::response::IntoResponse::into_response)
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
async fn handler_kaigi_relays(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    AxQuery(params): AxQuery<routing::KaigiRelayFormatParams>,
) -> Result<Response, Error> {
    let key = rate_limit_key(&headers, None, "v1/kaigi/relays", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    routing::handle_v1_kaigi_relays(
        app.state.clone(),
        app.telemetry.clone(),
        AxQuery(params),
        format,
    )
    .await
    .map(axum::response::IntoResponse::into_response)
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
async fn handler_kaigi_relay_detail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    AxPath(relay_id): AxPath<String>,
    AxQuery(params): AxQuery<routing::KaigiRelayFormatParams>,
) -> Result<Response, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/kaigi/relays/{relay_id}",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    routing::handle_v1_kaigi_relay_detail_with_policy(
        app.state.clone(),
        app.telemetry.clone(),
        AxPath(relay_id),
        AxQuery(params),
        format,
    )
    .await
    .map(axum::response::IntoResponse::into_response)
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
async fn handler_kaigi_relays_health(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/kaigi/relays/health",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    routing::handle_v1_kaigi_relays_health(app.state.clone(), app.telemetry.clone(), format)
        .await
        .map(axum::response::IntoResponse::into_response)
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
async fn handler_kaigi_relays_sse(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(mut params): AxQuery<routing::KaigiRelayEventsParams>,
) -> Result<Response, Error> {
    if let Some(ref relay_literal) = params.relay {
        let parsed = routing::parse_account_literal(
            relay_literal,
            &app.telemetry,
            CONTEXT_KAIGI_RELAY_EVENTS_QUERY,
        )
        .map_err(|err| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "invalid relay literal `{relay_literal}`: {}",
                    err.reason()
                )),
            ))
        })?;
        params.relay = Some(parsed.canonical().to_string());
    }

    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(
            routing::handle_v1_kaigi_relays_sse(app.events.clone(), AxQuery(params))
                .into_response(),
        );
    }

    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }

    let key = rate_limit_key(
        &headers,
        None,
        "v1/kaigi/relays/events",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    Ok(routing::handle_v1_kaigi_relays_sse(app.events.clone(), AxQuery(params)).into_response())
}

#[cfg(feature = "app_api")]
async fn handler_soradns_directory_latest(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/soradns/directory/latest",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    routing::handle_v1_soradns_directory_latest(app.state.clone())
        .map(axum::response::IntoResponse::into_response)
}

#[cfg(feature = "app_api")]
async fn handler_soradns_directory_events(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(
            routing::handle_v1_soradns_directory_events_sse(app.events.clone()).into_response(),
        );
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/soradns/directory/events",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(routing::handle_v1_soradns_directory_events_sse(app.events.clone()).into_response())
}

// -------------- Streaming: SSE / WS / P2P --------------

#[cfg(feature = "app_api")]
async fn handler_events_sse(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(params): AxQuery<routing::EventsSseParams>,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(
            routing::handle_v1_events_sse(app.events.clone(), AxQuery(params))?.into_response(),
        );
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/events/sse", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(routing::handle_v1_events_sse(app.events.clone(), AxQuery(params))?.into_response())
}

#[cfg(feature = "app_api")]
async fn handler_gov_stream(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(routing::handle_v1_gov_stream(app.events.clone()).into_response());
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/gov/stream", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(routing::handle_v1_gov_stream(app.events.clone()).into_response())
}

#[cfg(all(feature = "app_api", feature = "telemetry"))]
async fn handler_telemetry_live(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(routing::handle_v1_telemetry_live(
            app.state.clone(),
            app.kura.clone(),
            app.telemetry.clone(),
            app.peer_telemetry.clone(),
            app.events.clone(),
        )?
        .into_response());
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/telemetry/live",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(routing::handle_v1_telemetry_live(
        app.state.clone(),
        app.kura.clone(),
        app.telemetry.clone(),
        app.peer_telemetry.clone(),
        app.events.clone(),
    )?
    .into_response())
}

#[cfg(feature = "app_api")]
async fn handler_explorer_transactions_stream(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(routing::handle_v1_explorer_transactions_stream(
            app.kura.clone(),
            app.events.clone(),
        )
        .into_response());
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/explorer/transactions/stream",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(
        routing::handle_v1_explorer_transactions_stream(app.kura.clone(), app.events.clone())
            .into_response(),
    )
}

#[cfg(feature = "app_api")]
async fn handler_explorer_blocks_stream(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(
            routing::handle_v1_explorer_blocks_stream(app.kura.clone(), app.events.clone())
                .into_response(),
        );
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/explorer/blocks/stream",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(
        routing::handle_v1_explorer_blocks_stream(app.kura.clone(), app.events.clone())
            .into_response(),
    )
}

#[cfg(feature = "app_api")]
async fn handler_explorer_instructions_stream(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return Ok(routing::handle_v1_explorer_instructions_stream(
            app.kura.clone(),
            app.events.clone(),
        )
        .into_response());
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/explorer/instructions/stream",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(
        routing::handle_v1_explorer_instructions_stream(app.kura.clone(), app.events.clone())
            .into_response(),
    )
}

#[cfg(feature = "app_api")]
fn is_expected_ws_disconnect(error: &eyre::Report) -> bool {
    let msg = error.to_string().to_ascii_lowercase();
    msg.contains("connection is closed")
        || msg.contains("broken pipe")
        || msg.contains("connection reset")
        || msg.contains("already closed")
}

#[cfg(all(test, feature = "app_api"))]
mod ws_disconnect_classification_tests {
    use super::is_expected_ws_disconnect;

    #[test]
    fn detects_expected_disconnect_errors() {
        assert!(is_expected_ws_disconnect(&eyre::eyre!(
            "WebSocket error: IO error: Broken pipe (os error 32)"
        )));
        assert!(is_expected_ws_disconnect(&eyre::eyre!(
            "Event consumption resulted in an error: Connection is closed"
        )));
    }

    #[test]
    fn keeps_unexpected_errors_as_failures() {
        assert!(!is_expected_ws_disconnect(&eyre::eyre!(
            "event stream lagged; skipping buffered events"
        )));
    }
}

#[cfg(feature = "app_api")]
async fn handler_subscription_ws(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        // Subscribe before upgrade to buffer events emitted during the WS handshake.
        let events_rx = app.events.subscribe();
        return Ok(core::future::ready(ws.on_upgrade(move |ws| async move {
            if let Err(error) = routing::event::handle_events_stream_with_receiver(
                events_rx,
                ws,
                app.ws_message_timeout,
            )
            .await
            {
                if is_expected_ws_disconnect(&error) {
                    iroha_logger::debug!(%error, "Event streaming closed by client");
                } else {
                    iroha_logger::error!(%error, "Failure during event streaming");
                }
            }
        }))
        .await);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "subscription/stream",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    // Subscribe before upgrade to buffer events emitted during the WS handshake.
    let events_rx = app.events.subscribe();
    Ok(core::future::ready(ws.on_upgrade(move |ws| async move {
        if let Err(error) = routing::event::handle_events_stream_with_receiver(
            events_rx,
            ws,
            app.ws_message_timeout,
        )
        .await
        {
            if is_expected_ws_disconnect(&error) {
                iroha_logger::debug!(%error, "Event streaming closed by client");
            } else {
                iroha_logger::error!(%error, "Failure during event streaming");
            }
        }
    }))
    .await)
}

#[cfg(feature = "app_api")]
async fn handler_blocks_stream_ws(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        let kura = app.kura.clone();
        return Ok(core::future::ready(ws.on_upgrade(move |ws| async move {
            if let Err(error) =
                routing::block::handle_blocks_stream(kura, ws, app.ws_message_timeout).await
            {
                iroha_logger::error!(%error, "Failure during block streaming");
            }
        }))
        .await);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "blocks/stream", app.api_token_enforced());
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    let kura = app.kura.clone();
    Ok(core::future::ready(ws.on_upgrade(move |ws| async move {
        if let Err(error) =
            routing::block::handle_blocks_stream(kura, ws, app.ws_message_timeout).await
        {
            iroha_logger::error!(%error, "Failure during block streaming");
        }
    }))
    .await)
}

#[cfg(feature = "p2p_ws")]
async fn handler_p2p_ws(
    State(app): State<SharedAppState>,
    ws: WebSocketUpgrade,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> AxResponse {
    core::future::ready(ws.on_upgrade(move |ws| async move {
        routing::handle_p2p_ws(ws, app.p2p.clone(), remote).await
    }))
    .await
}
#[cfg(feature = "telemetry")]
async fn handler_sumeragi_params(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/params",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/params",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_params(State(app.state.clone()), accept).await
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_bls_keys(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/bls_keys",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/bls_keys",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_bls_keys(State(app.state.clone()), accept).await
}

// ---------------- Contracts/VK GET handlers ----------------

#[cfg(feature = "app_api")]
async fn handler_get_contract_code_bytes(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(code_hash): axum::extract::Path<String>,
) -> Result<impl IntoResponse, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/code-bytes/{code_hash}",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    crate::routing::handle_get_contract_code_bytes(
        app.state.clone(),
        axum::extract::Path(code_hash),
    )
    .await
}

// internal handler; tests should use routing::handle_get_contract_code_bytes via re-export below

#[cfg(feature = "app_api")]
async fn handler_get_contract_code(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(code_hash): axum::extract::Path<String>,
) -> Result<AxResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_get_contract_code(
            app.state.clone(),
            axum::extract::Path(code_hash),
        )
        .await
        .map(axum::response::IntoResponse::into_response);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/code:get",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_get_contract_code(
        app.state.clone(),
        axum::extract::Path(code_hash),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        ))) => Ok(axum::http::StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e),
    }
}

#[cfg(feature = "app_api")]
async fn handler_get_contract_state(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(q): AxQuery<crate::routing::ContractStateQuery>,
) -> Result<JsonBody<crate::routing::ContractStateResponse>, Error> {
    check_access(&app, &headers, None, "v1/contracts/state").await?;
    crate::routing::handle_get_contract_state(app.state.clone(), crate::NoritoQuery(q)).await
}

#[cfg(feature = "app_api")]
async fn handler_get_vk_by_backend_name(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path((backend, name)): axum::extract::Path<(String, String)>,
) -> Result<AxResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_get_vk(
            app.state.clone(),
            axum::extract::Path((backend, name)),
        )
        .await
        .map(axum::response::IntoResponse::into_response);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/zk/vk/{backend}/{name}",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_get_vk(app.state.clone(), axum::extract::Path((backend, name)))
        .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        ))) => Ok(axum::http::StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e),
    }
}

#[cfg(feature = "app_api")]
async fn handler_get_proof_by_backend_hash(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    axum::extract::Path((backend, hash)): axum::extract::Path<(String, String)>,
) -> Result<AxResponse, Error> {
    ensure_proof_api_version(&app, negotiated, "v1/zk/proof/{backend}/{hash}")?;
    let enforce = !limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/proof/{backend}/{hash}",
        1,
        enforce,
    )
    .await?;
    match crate::routing::handle_get_proof(
        app.state.clone(),
        app.proof_limits,
        &app.telemetry,
        Some(&headers),
        axum::extract::Path((backend, hash)),
    )
    .await
    {
        Ok(resp) => {
            enforce_proof_egress(&app, &headers, None, "v1/zk/proof", resp.bytes, enforce).await?;
            Ok(resp.into_response())
        }
        Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        ))) => Ok(axum::http::StatusCode::NOT_FOUND.into_response()),
        Err(e) => Err(e),
    }
}
#[cfg(feature = "app_api")]
async fn handler_list_vk(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(q): AxQuery<crate::routing::VkListQuery>,
) -> Result<impl IntoResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_list_vk(app.state.clone(), AxQuery(q)).await;
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/zk/vk", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    crate::routing::handle_list_vk(app.state.clone(), AxQuery(q)).await
}

#[cfg(feature = "app_api")]
async fn handler_list_proofs(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    AxQuery(q): AxQuery<crate::routing::ProofListQuery>,
) -> Result<impl IntoResponse, Error> {
    ensure_proof_api_version(&app, negotiated, "v1/zk/proofs")?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_list_proofs(
            app.state.clone(),
            app.proof_limits,
            app.telemetry.clone(),
            AxQuery(q),
        )
        .await;
    }
    let mut q = q;
    let requested_limit = q.limit.unwrap_or(app.proof_limits.max_list_limit);
    if requested_limit == 0 || requested_limit > app.proof_limits.max_list_limit {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "limit must be between 1 and {}",
                app.proof_limits.max_list_limit
            )),
        )));
    }
    q.limit = Some(requested_limit);
    let cost = u64::from(requested_limit).div_ceil(50);
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/proofs",
        cost.max(1),
        true,
    )
    .await?;
    crate::routing::handle_list_proofs(
        app.state.clone(),
        app.proof_limits,
        app.telemetry.clone(),
        AxQuery(q),
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_count_proofs(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    AxQuery(q): AxQuery<crate::routing::ProofListQuery>,
) -> Result<impl IntoResponse, Error> {
    ensure_proof_api_version(&app, negotiated, "v1/zk/proofs/count")?;
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_count_proofs(
            app.state.clone(),
            app.proof_limits,
            app.telemetry.clone(),
            AxQuery(q),
        )
        .await;
    }
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "v1/zk/proofs/count",
        1,
        true,
    )
    .await?;
    crate::routing::handle_count_proofs(
        app.state.clone(),
        app.proof_limits,
        app.telemetry.clone(),
        AxQuery(q),
    )
    .await
}
#[cfg(feature = "telemetry")]
async fn handler_rbc_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/sumeragi/rbc", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/rbc",
            &app.telemetry,
        ));
    }
    Ok(routing::handle_v1_sumeragi_rbc_status(&app.telemetry)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_debug_axt_cache(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/debug/axt/cache",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/debug/axt/cache",
            &app.telemetry,
        ));
    }
    let cache_snapshot = app.state.metrics().axt_proof_cache_status_snapshot();
    fn encode_json<T: JsonSerialize>(value: &T) -> Result<norito::json::Value, Error> {
        norito::json::to_value(value).map_err(|err| {
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                err.to_string(),
            ))
        })
    }
    let entries: Vec<norito::json::Value> = cache_snapshot
        .into_iter()
        .map(|entry| {
            let manifest_root = entry
                .manifest_root
                .map(hex::encode)
                .unwrap_or_else(|| "none".to_string());
            let mut map = norito::json::Map::new();
            map.insert(
                "dataspace".to_string(),
                encode_json(&entry.dataspace.as_u64())?,
            );
            map.insert("status".to_string(), encode_json(&entry.status)?);
            map.insert(
                "manifest_root_hex".to_string(),
                encode_json(&manifest_root)?,
            );
            map.insert(
                "verified_slot".to_string(),
                encode_json(&entry.verified_slot)?,
            );
            map.insert("expiry_slot".to_string(), encode_json(&entry.expiry_slot)?);
            Ok(norito::json::Value::Object(map))
        })
        .collect::<Result<_, Error>>()?;
    let policy_snapshot = app.state.axt_policy_snapshot();
    let snapshot_version =
        iroha_data_model::nexus::AxtPolicySnapshot::compute_version(&policy_snapshot.entries);
    let reject_hints = app
        .state
        .metrics()
        .axt_reject_hints_snapshot()
        .into_iter()
        .map(|hint| {
            let mut map = norito::json::Map::new();
            map.insert(
                "dataspace".to_string(),
                encode_json(&hint.dataspace.as_u64())?,
            );
            map.insert(
                "target_lane".to_string(),
                encode_json(&hint.target_lane.as_u32())?,
            );
            map.insert(
                "next_min_handle_era".to_string(),
                encode_json(&hint.next_min_handle_era)?,
            );
            map.insert(
                "next_min_sub_nonce".to_string(),
                encode_json(&hint.next_min_sub_nonce)?,
            );
            map.insert("reason".to_string(), encode_json(&hint.reason)?);
            Ok(norito::json::Value::Object(map))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let mut payload = norito::json::Map::new();
    payload.insert(
        "snapshot_version".to_string(),
        encode_json(&snapshot_version)?,
    );
    payload.insert(
        "reject_hints".to_string(),
        norito::json::Value::Array(reject_hints),
    );
    payload.insert("entries".to_string(), norito::json::Value::Array(entries));
    json_ok(norito::json::Value::Object(payload))
}

#[cfg(feature = "telemetry")]
async fn handler_debug_witness(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/debug/witness", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/debug/witness",
            &app.telemetry,
        ));
    }

    let witness = iroha_core::sumeragi::witness::snapshot_exec_witness();
    let format =
        crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)).map_err(|_| {
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                "Not Acceptable".to_string(),
            ))
        })?;

    match format {
        ResponseFormat::Json => json_ok(witness),
        ResponseFormat::Norito => Ok(utils::NoritoBody(witness).into_response()),
    }
}

async fn handler_sumeragi_evidence(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
    AxQuery(q): AxQuery<routing::EvidenceListQuery>,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/evidence",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/evidence",
            &app.telemetry,
        ));
    }
    routing::handle_v1_sumeragi_evidence_list(
        State(app.state.clone()),
        crate::NoritoQuery(q),
        accept.map(|e| e.0),
    )
    .await
    .map(axum::response::IntoResponse::into_response)
}

async fn handler_sumeragi_evidence_submit(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(request): NoritoJson<routing::EvidenceSubmitRequestDto>,
) -> Result<AxResponse, Error> {
    use axum::response::IntoResponse;

    let Some(handle) = app.sumeragi.clone() else {
        return Ok(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    };

    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let token_hdr = headers.get("x-api-token").and_then(|v| v.to_str().ok());
        let ok = token_hdr
            .as_ref()
            .is_some_and(|token| app.api_tokens_set.contains(*token));
        if !ok {
            return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
        }
    }

    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/evidence/submit",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    routing::handle_post_sumeragi_evidence_submit(
        handle,
        request,
        app.state.as_ref(),
        app.chain_id.as_ref(),
    )
}

async fn handler_sumeragi_evidence_count(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_evidence_count(State(app.state.clone()), accept).await
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/status",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_metrics() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/status",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    routing::handle_v1_sumeragi_status(accept, nexus_enabled)
        .await
        .map(axum::response::IntoResponse::into_response)
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_status_sse(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/status/sse",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    // This stream emits the same JSON payload as `/v1/sumeragi/status` but continuously.
    // Gate it as an expensive telemetry output (allowed in `extended`/`full`) rather than
    // a developer-only sink.
    if !(app.telemetry.allows_expensive_metrics() || app.telemetry.allows_developer_outputs()) {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/status/sse",
            &app.telemetry,
        ));
    }
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    Ok(routing::handle_v1_sumeragi_status_sse(1_000, nexus_enabled).into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_telemetry(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/telemetry",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_metrics() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/telemetry",
            &app.telemetry,
        ));
    }
    routing::handle_v1_sumeragi_telemetry(app.state.clone())
        .await
        .map(axum::response::IntoResponse::into_response)
}

async fn handler_sumeragi_vrf_penalties(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(epoch): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/vrf/penalties",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/vrf/penalties",
            &app.telemetry,
        ));
    }
    routing::handle_v1_sumeragi_vrf_penalties(AxPath(epoch))
        .await
        .map(axum::response::IntoResponse::into_response)
}

async fn handler_sumeragi_vrf_epoch(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath(epoch): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/vrf/epoch",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/vrf/epoch",
            &app.telemetry,
        ));
    }

    let epoch = if let Some(rest) = epoch.strip_prefix("0x") {
        u64::from_str_radix(rest, 16).map_err(|_| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid epoch".into(),
                ),
            ))
        })?
    } else {
        epoch.parse::<u64>().map_err(|_| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid epoch".into(),
                ),
            ))
        })?
    };

    routing::handle_v1_sumeragi_vrf_epoch(app.state.clone(), epoch)
        .await
        .map(axum::response::IntoResponse::into_response)
}

#[cfg(feature = "telemetry")]
async fn handler_rbc_delivered_height_view(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxPath((height, view)): AxPath<(u64, u64)>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/rbc/delivered",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/rbc/delivered",
            &app.telemetry,
        ));
    }
    Ok(
        routing::handle_v1_sumeragi_rbc_delivered_height_view(AxPath((height, view)))
            .await?
            .into_response(),
    )
}
#[cfg(feature = "telemetry")]
async fn handler_pacemaker_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/pacemaker",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/pacemaker",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_pacemaker(&app.telemetry, accept).await
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_phases(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/phases",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/phases",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_phases(accept).await
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_leader(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/leader",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/leader",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    routing::handle_v1_sumeragi_leader(accept).await
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_qc(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/sumeragi/qc", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/qc",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    Ok(routing::handle_v1_sumeragi_qc(accept)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_checkpoints(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/checkpoints",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/checkpoints",
            &app.telemetry,
        ));
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    Ok(routing::handle_v1_sumeragi_checkpoints(accept)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_commit_qcs(
    State(app): State<SharedAppState>,
    window: crate::NoritoQuery<routing::HistoryWindowQuery>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "/v1/sumeragi/commit-certificates",
        app.api_token_enforced(),
    );
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(
            &app.telemetry,
            &api_token,
            "v1/sumeragi/commit-certificates",
        );
    }
    rate_limit_requests(&app, &key).await?;
    Ok(routing::handle_v1_sumeragi_commit_qcs(window, accept)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_bridge_finality_proof(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "/v1/bridge/finality/{height}",
        app.api_token_enforced(),
    );
    rate_limit_requests(&app, &key).await?;
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(&app.telemetry, &api_token, "v1/bridge/finality");
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    Ok(
        routing::handle_v1_bridge_finality(app.state.clone(), height, accept)
            .await?
            .into_response(),
    )
}

#[cfg(feature = "telemetry")]
async fn handler_bridge_finality_bundle(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "/v1/bridge/finality/bundle/{height}",
        app.api_token_enforced(),
    );
    rate_limit_requests(&app, &key).await?;
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(
            &app.telemetry,
            &api_token,
            "v1/bridge/finality/bundle",
        );
    }
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    Ok(
        routing::handle_v1_bridge_finality_bundle(app.state.clone(), height, accept)
            .await?
            .into_response(),
    )
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_validator_sets(
    State(app): State<SharedAppState>,
    window: crate::NoritoQuery<routing::HistoryWindowQuery>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        iroha_torii_shared::uri::SUMERAGI_VALIDATOR_SETS,
        app.api_token_enforced(),
    );
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(
            &app.telemetry,
            &api_token,
            iroha_torii_shared::uri::SUMERAGI_VALIDATOR_SETS,
        );
    }
    rate_limit_requests(&app, &key).await?;
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    Ok(routing::handle_v1_sumeragi_validator_sets(window, accept)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_validator_set_by_height(
    State(app): State<SharedAppState>,
    AxPath(height): AxPath<u64>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        iroha_torii_shared::uri::SUMERAGI_VALIDATOR_SET_BY_HEIGHT,
        app.api_token_enforced(),
    );
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(
            &app.telemetry,
            &api_token,
            iroha_torii_shared::uri::SUMERAGI_VALIDATOR_SET_BY_HEIGHT,
        );
    }
    rate_limit_requests(&app, &key).await?;
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    Ok(
        routing::handle_v1_sumeragi_validator_set_by_height(AxPath(height), accept)
            .await?
            .into_response(),
    )
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_consensus_keys(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "/v1/sumeragi/consensus-keys",
        app.api_token_enforced(),
    );
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(
            &app.telemetry,
            &api_token,
            "v1/sumeragi/consensus-keys",
        );
    }
    rate_limit_requests(&app, &key).await?;
    Ok(routing::handle_v1_sumeragi_consensus_keys(&app, accept)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_sumeragi_key_lifecycle(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "/v1/sumeragi/key-lifecycle",
        app.api_token_enforced(),
    );
    let accept = headers.get(axum::http::header::ACCEPT).cloned();
    if let Some(api_token) = token_hdr {
        crate::telemetry::report_torii_api_hit(
            &app.telemetry,
            &api_token,
            "v1/sumeragi/key-lifecycle",
        );
    }
    rate_limit_requests(&app, &key).await?;
    Ok(routing::handle_v1_sumeragi_key_lifecycle(accept)
        .await?
        .into_response())
}

#[cfg(feature = "telemetry")]
async fn handler_commit_qc(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
    AxPath(hash): AxPath<String>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/commit_qc",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    routing::handle_v1_sumeragi_commit_qc(
        State(app.state.clone()),
        AxPath(hash),
        accept.map(|e| e.0),
    )
    .await
    .map(axum::response::IntoResponse::into_response)
}
// ---------------- Contracts/VK POST handlers ----------------

#[cfg(feature = "app_api")]
async fn handler_post_contract_code(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RegisterContractCodeDto>,
) -> Result<AxResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_post_contract_code(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            request,
        )
        .await
        .map(axum::response::IntoResponse::into_response);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("code"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/code",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.deploy_rate_limiter, &key, enforce).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("code"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_contract_code(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("code"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_contract_deploy(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::DeployContractDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("deploy"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/deploy",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("deploy"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_contract_deploy(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("deploy"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_contract_instance(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::DeployAndActivateInstanceDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("instance"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/instance",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("instance"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_contract_instance(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("instance"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_contract_instance_activate(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::ActivateInstanceDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("activate"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/instance/activate",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("activate"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_contract_instance_activate(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("activate"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_contract_call(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::ContractCallDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("call"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/contracts/call",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("call"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_contract_call(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("call"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_register_manifest(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    NoritoJson(request): NoritoJson<crate::routing::RegisterPinManifestDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/pin/register",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_register_manifest(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        crate::NoritoJson(request),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry.with_metrics(|tel| {
                tel.inc_sorafs_disputes("rejected");
                tel.inc_torii_contract_error("sorafs");
            });
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_declare(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RegisterCapacityDeclarationDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/declare",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_register_capacity_declaration(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_telemetry(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordCapacityTelemetryDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/telemetry",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_capacity_telemetry(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_dispute(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RegisterCapacityDisputeDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/dispute",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_register_capacity_dispute(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        app.sorafs_limits.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_schedule(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::ScheduleReplicationOrderDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/schedule",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_schedule_replication_order(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_complete(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::CompleteReplicationOrderDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/complete",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_complete_replication_order(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_deal_usage(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<routing::RecordDealUsageDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/deal/usage",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    let NoritoJson(dto) = request;
    let result = routing::handle_post_sorafs_record_deal_usage(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        NoritoJson(dto),
    )
    .await?;
    app.publish_deal_usage_event(&result.report, &result.outcome);
    Ok(result.response)
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_deal_settle(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<routing::SettleDealDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/deal/settle",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    let NoritoJson(dto) = request;
    let result = routing::handle_post_sorafs_settle_deal(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        NoritoJson(dto),
    )
    .await?;
    app.publish_deal_settlement_event(&result.outcome, &result.encoded, &result.encoded_b64);
    Ok(result.response)
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_uptime(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordUptimeObservationDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/uptime",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_uptime_observation(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_por(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordPorObservationDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/por",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_por_observation(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_por_challenge(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordPorChallengeDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/por-challenge",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_por_challenge(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        app.por_coordinator.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_por_proof(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordPorProofDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/por-proof",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_por_proof(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        app.por_coordinator.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_por_verdict(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordPorVerdictDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/por-verdict",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_por_verdict(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        app.sorafs_limits.clone(),
        app.por_coordinator.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_get_sorafs_por_status(
    State(app): State<SharedAppState>,
    AxQuery(query): AxQuery<crate::routing::PorStatusQueryDto>,
) -> Result<AxResponse, Error> {
    let statuses =
        crate::routing::handle_get_sorafs_por_status(app.por_coordinator.clone(), query)?;
    let body = norito::to_bytes(&statuses)
        .map_err(|err| conversion_error(format!("failed to encode PoR status response: {err}")))?;
    let mut resp = AxResponse::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/x-norito"),
    );
    Ok(resp)
}

#[cfg(feature = "app_api")]
async fn handler_get_sorafs_por_export(
    State(app): State<SharedAppState>,
    AxQuery(query): AxQuery<crate::routing::PorExportQueryDto>,
) -> Result<AxResponse, Error> {
    let export = crate::routing::handle_get_sorafs_por_export(app.por_coordinator.clone(), query)?;
    let body = norito::to_bytes(&export)
        .map_err(|err| conversion_error(format!("failed to encode PoR export payload: {err}")))?;
    let mut resp = AxResponse::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/octet-stream"),
    );
    Ok(resp)
}

#[cfg(feature = "app_api")]
async fn handler_get_sorafs_por_report(
    State(app): State<SharedAppState>,
    AxPath(week_label): AxPath<String>,
) -> Result<AxResponse, Error> {
    let cycle = crate::routing::parse_report_iso_week(&week_label)?;
    let report = crate::routing::handle_get_sorafs_por_report(app.por_coordinator.clone(), cycle)?;
    let body = norito::to_bytes(&report)
        .map_err(|err| conversion_error(format!("failed to encode PoR weekly report: {err}")))?;
    let mut resp = AxResponse::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/x-norito"),
    );
    Ok(resp)
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_capacity_failure(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::RecordReplicationFailureDto>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/capacity/failure",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_record_replication_failure(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        request,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_repair_report(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(report): NoritoJson<RepairReportV1>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/report",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_repair_report(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        report,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_repair_slash(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(proposal): NoritoJson<RepairSlashProposalV1>,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/slash",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_repair_slash(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        proposal,
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
fn enforce_sorafs_repair_worker_auth(
    app: &SharedAppState,
    ticket_id: &RepairTicketId,
    manifest_digest_hex: &str,
    worker_id: &str,
    idempotency_key: &str,
    action: RepairWorkerActionV1,
    signature: &SignatureOf<RepairWorkerSignaturePayloadV1>,
) -> Result<(), Error> {
    let record = app
        .sorafs_node
        .repair_task_record(ticket_id)
        .ok_or_else(|| conversion_error(format!("unknown repair ticket `{ticket_id}`")))?;
    let manifest_digest = {
        let trimmed = manifest_digest_hex.trim_start_matches("0x");
        let bytes = hex::decode(trimmed).map_err(|err| {
            conversion_error(format!(
                "invalid manifest_digest_hex (expected 32 bytes): {err}"
            ))
        })?;
        let len = bytes.len();
        let digest: [u8; 32] = bytes.try_into().map_err(|_| {
            conversion_error(format!("manifest_digest_hex must be 32 bytes (got {len})"))
        })?;
        digest
    };
    if manifest_digest != record.manifest_digest {
        return Err(conversion_error(
            "manifest_digest_hex does not match ticket record".to_string(),
        ));
    }
    let payload = RepairWorkerSignaturePayloadV1 {
        version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
        ticket_id: ticket_id.clone(),
        manifest_digest,
        provider_id: record.provider_id,
        worker_id: worker_id.to_string(),
        idempotency_key: idempotency_key.to_string(),
        action,
    };
    payload.validate().map_err(|err| {
        conversion_error(format!("invalid repair worker signature payload: {err}"))
    })?;

    let account_id: AccountId = worker_id
        .parse()
        .map_err(|err| conversion_error(format!("invalid worker_id `{worker_id}`: {err}")))?;
    let permission = Permission::from(CanOperateSorafsRepair {
        provider_id: ProviderId::new(record.provider_id),
    });
    let world = app.state.world_view();
    let permitted = world
        .account_permissions()
        .get(&account_id)
        .is_some_and(|perms| perms.contains(&permission));
    if !permitted {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(
                "repair worker lacks permission".to_string(),
            ),
        ));
    }
    let Some(signatory) = account_id.try_signatory() else {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(
                "repair worker must use a signatory account id".to_string(),
            ),
        ));
    };
    signature.verify(signatory, &payload).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::NotPermitted(format!(
            "repair worker signature invalid: {err}",
        )))
    })?;
    Ok(())
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_repair_claim(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(req): NoritoJson<crate::routing::RepairWorkerClaimDto>,
) -> Result<AxResponse, Error> {
    let action = RepairWorkerActionV1::Claim {
        claimed_at_unix: req.claimed_at_unix,
    };
    if let Err(err) = enforce_sorafs_repair_worker_auth(
        &app,
        &req.ticket_id,
        &req.manifest_digest_hex,
        &req.worker_id,
        &req.idempotency_key,
        action,
        &req.signature,
    ) {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
        return Err(err);
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/claim",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_repair_claim(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        NoritoJson(req),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_repair_heartbeat(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(req): NoritoJson<crate::routing::RepairWorkerHeartbeatDto>,
) -> Result<AxResponse, Error> {
    let action = RepairWorkerActionV1::Heartbeat {
        heartbeat_at_unix: req.heartbeat_at_unix,
    };
    if let Err(err) = enforce_sorafs_repair_worker_auth(
        &app,
        &req.ticket_id,
        &req.manifest_digest_hex,
        &req.worker_id,
        &req.idempotency_key,
        action,
        &req.signature,
    ) {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
        return Err(err);
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/heartbeat",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_repair_heartbeat(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        NoritoJson(req),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_repair_complete(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(req): NoritoJson<crate::routing::RepairWorkerCompleteDto>,
) -> Result<AxResponse, Error> {
    let action = RepairWorkerActionV1::Complete {
        completed_at_unix: req.completed_at_unix,
        resolution_notes: req.resolution_notes.clone(),
    };
    if let Err(err) = enforce_sorafs_repair_worker_auth(
        &app,
        &req.ticket_id,
        &req.manifest_digest_hex,
        &req.worker_id,
        &req.idempotency_key,
        action,
        &req.signature,
    ) {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
        return Err(err);
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/complete",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_repair_complete(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        NoritoJson(req),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_sorafs_repair_fail(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(req): NoritoJson<crate::routing::RepairWorkerFailDto>,
) -> Result<AxResponse, Error> {
    let action = RepairWorkerActionV1::Fail {
        failed_at_unix: req.failed_at_unix,
        reason: req.reason.clone(),
    };
    if let Err(err) = enforce_sorafs_repair_worker_auth(
        &app,
        &req.ticket_id,
        &req.manifest_digest_hex,
        &req.worker_id,
        &req.idempotency_key,
        action,
        &req.signature,
    ) {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
        return Err(err);
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/fail",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_post_sorafs_repair_fail(
        app.telemetry.clone(),
        app.sorafs_node.clone(),
        NoritoJson(req),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_get_sorafs_repair_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(manifest_hex): axum::extract::Path<String>,
    AxQuery(query): AxQuery<crate::routing::RepairStatusQueryDto>,
) -> Result<AxResponse, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/status",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_get_sorafs_repair_status(
        app.sorafs_node.clone(),
        axum::extract::Path(manifest_hex),
        AxQuery(query),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

#[cfg(feature = "app_api")]
async fn handler_get_sorafs_repair_status_all(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    AxQuery(query): AxQuery<crate::routing::RepairStatusQueryDto>,
) -> Result<AxResponse, Error> {
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sorafs/audit/repair/status",
        app.api_token_enforced(),
    );
    if !app.deploy_rate_limiter.allow(&key).await {
        app.telemetry
            .with_metrics(|tel| tel.inc_torii_contract_throttle("sorafs"));
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    match crate::routing::handle_get_sorafs_repair_status_all(
        app.sorafs_node.clone(),
        AxQuery(query),
    )
    .await
    {
        Ok(resp) => Ok(resp.into_response()),
        Err(err) => {
            app.telemetry
                .with_metrics(|tel| tel.inc_torii_contract_error("sorafs"));
            Err(err)
        }
    }
}

async fn handler_iso_pacs008(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, JsonBody<norito::json::native::Value>), Error> {
    check_access(&app, &headers, None, "v1/iso20022/pacs008").await?;
    let runtime = match &app.iso_bridge {
        Some(rt) => rt.clone(),
        None => {
            return Err(Error::Query(
                iroha_data_model::ValidationFail::NotPermitted("iso20022 bridge disabled".into()),
            ));
        }
    };
    if body.is_empty() {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted("empty ISO 20022 payload".into()),
        ));
    }

    let parsed =
        parse_message("pacs.008", &body).map_err(|err| Error::Query(map_iso_error(err)))?;
    let msg_id = parsed
        .field_text("MsgId")
        .ok_or_else(|| {
            Error::Query(iroha_data_model::ValidationFail::NotPermitted(
                "missing MsgId field".into(),
            ))
        })?
        .to_owned();

    if !runtime.check_and_record_message(&msg_id) {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted("duplicate message identifier".into()),
        ));
    }

    let (transaction, context) = match runtime.build_pacs008_transaction(
        &parsed,
        Arc::as_ref(&app.chain_id),
        &app.telemetry,
    ) {
        Ok(result) => result,
        Err(err) => {
            runtime.mark_rejected(&msg_id, Some(err.to_string()), None);
            return Err(Error::Query(map_iso_error(err)));
        }
    };

    runtime.update_message_context(&msg_id, context.clone());

    let tx_hash = transaction.hash();
    let tx_hash_str = format!("{}", tx_hash);

    if let Err(err) = routing::handle_transaction(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        transaction,
    )
    .await
    {
        let (detail, reason_code) = match &err {
            Error::PushIntoQueue { source, .. } => {
                let (code, detail) = queue_rejection_metadata(source.as_ref());
                (detail, Some(code))
            }
            _ => (err.to_string(), None),
        };
        runtime.mark_rejected(&msg_id, Some(detail), reason_code);
        return Err(err);
    }

    runtime.mark_queued(&msg_id);
    runtime.mark_accepted(&msg_id, &tx_hash_str);
    let status_snapshot = runtime
        .message_status(&msg_id)
        .expect("iso bridge status must exist immediately after mark_accepted");

    let mut payload = norito::json::native::Map::new();
    payload.insert(
        "message_id".into(),
        norito::json::native::Value::from(msg_id),
    );
    payload.insert(
        "transaction_hash".into(),
        norito::json::native::Value::from(tx_hash_str),
    );
    payload.insert(
        "status".into(),
        norito::json::native::Value::from(status_snapshot.status_label().to_string()),
    );
    payload.insert(
        "pacs002_code".into(),
        norito::json::native::Value::from(status_snapshot.pacs002_code().to_string()),
    );
    payload.insert(
        "hold_reason_code".into(),
        json_string_or_null(status_snapshot.hold_reason_code().map(ToString::to_string)),
    );
    let change_codes = status_snapshot
        .change_reason_codes()
        .iter()
        .cloned()
        .map(norito::json::native::Value::from)
        .collect::<Vec<_>>();
    payload.insert(
        "change_reason_codes".into(),
        norito::json::native::Value::Array(change_codes),
    );
    payload.insert(
        "rejection_reason_code".into(),
        json_string_or_null(
            status_snapshot
                .rejection_reason_code()
                .map(ToString::to_string),
        ),
    );
    payload.insert(
        "ledger_id".into(),
        json_string_or_null(context.ledger_id().map(ToString::to_string)),
    );
    payload.insert(
        "source_account_id".into(),
        json_string_or_null(context.source_account_id().map(ToString::to_string)),
    );
    payload.insert(
        "source_account_address".into(),
        json_string_or_null(context.source_account_address().map(ToString::to_string)),
    );
    payload.insert(
        "target_account_id".into(),
        json_string_or_null(context.target_account_id().map(ToString::to_string)),
    );
    payload.insert(
        "target_account_address".into(),
        json_string_or_null(context.target_account_address().map(ToString::to_string)),
    );
    payload.insert(
        "asset_definition_id".into(),
        json_string_or_null(context.asset_definition_id().map(ToString::to_string)),
    );
    payload.insert(
        "asset_id".into(),
        json_string_or_null(context.asset_id().map(ToString::to_string)),
    );
    Ok((
        StatusCode::ACCEPTED,
        JsonBody(norito::json::native::Value::Object(payload)),
    ))
}

async fn handler_iso_pacs009(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, JsonBody<norito::json::native::Value>), Error> {
    check_access(&app, &headers, None, "v1/iso20022/pacs009").await?;
    let runtime = match &app.iso_bridge {
        Some(rt) => rt.clone(),
        None => {
            return Err(Error::Query(
                iroha_data_model::ValidationFail::NotPermitted("iso20022 bridge disabled".into()),
            ));
        }
    };
    if body.is_empty() {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted("empty ISO 20022 payload".into()),
        ));
    }

    let parsed =
        parse_message("pacs.009", &body).map_err(|err| Error::Query(map_iso_error(err)))?;
    let msg_id = parsed
        .field_text("BizMsgIdr")
        .ok_or_else(|| {
            Error::Query(iroha_data_model::ValidationFail::NotPermitted(
                "missing BizMsgIdr field".into(),
            ))
        })?
        .to_owned();

    if !runtime.check_and_record_message(&msg_id) {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted("duplicate message identifier".into()),
        ));
    }

    let (transaction, context) = match runtime.build_pacs009_transaction(
        &parsed,
        Arc::as_ref(&app.chain_id),
        &app.telemetry,
    ) {
        Ok(result) => result,
        Err(err) => {
            runtime.mark_rejected(&msg_id, Some(err.to_string()), None);
            return Err(Error::Query(map_iso_error(err)));
        }
    };

    runtime.update_message_context(&msg_id, context.clone());

    let tx_hash = transaction.hash();
    let tx_hash_str = format!("{}", tx_hash);

    if let Err(err) = routing::handle_transaction(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        transaction,
    )
    .await
    {
        let (detail, reason_code) = match &err {
            Error::PushIntoQueue { source, .. } => {
                let (code, detail) = queue_rejection_metadata(source.as_ref());
                (detail, Some(code))
            }
            _ => (err.to_string(), None),
        };
        runtime.mark_rejected(&msg_id, Some(detail), reason_code);
        return Err(err);
    }

    runtime.mark_queued(&msg_id);
    runtime.mark_accepted(&msg_id, &tx_hash_str);
    let status_snapshot = runtime
        .message_status(&msg_id)
        .expect("iso bridge status must exist immediately after mark_accepted");

    let mut payload = norito::json::native::Map::new();
    payload.insert(
        "message_id".into(),
        norito::json::native::Value::from(msg_id),
    );
    payload.insert(
        "transaction_hash".into(),
        norito::json::native::Value::from(tx_hash_str),
    );
    payload.insert(
        "status".into(),
        norito::json::native::Value::from(status_snapshot.status_label().to_string()),
    );
    payload.insert(
        "pacs002_code".into(),
        norito::json::native::Value::from(status_snapshot.pacs002_code().to_string()),
    );
    payload.insert(
        "hold_reason_code".into(),
        json_string_or_null(status_snapshot.hold_reason_code().map(ToString::to_string)),
    );
    let change_codes = status_snapshot
        .change_reason_codes()
        .iter()
        .cloned()
        .map(norito::json::native::Value::from)
        .collect::<Vec<_>>();
    payload.insert(
        "change_reason_codes".into(),
        norito::json::native::Value::Array(change_codes),
    );
    payload.insert(
        "rejection_reason_code".into(),
        json_string_or_null(
            status_snapshot
                .rejection_reason_code()
                .map(ToString::to_string),
        ),
    );
    payload.insert(
        "ledger_id".into(),
        json_string_or_null(context.ledger_id().map(ToString::to_string)),
    );
    payload.insert(
        "source_account_id".into(),
        json_string_or_null(context.source_account_id().map(ToString::to_string)),
    );
    payload.insert(
        "source_account_address".into(),
        json_string_or_null(context.source_account_address().map(ToString::to_string)),
    );
    payload.insert(
        "target_account_id".into(),
        json_string_or_null(context.target_account_id().map(ToString::to_string)),
    );
    payload.insert(
        "target_account_address".into(),
        json_string_or_null(context.target_account_address().map(ToString::to_string)),
    );
    payload.insert(
        "asset_definition_id".into(),
        json_string_or_null(context.asset_definition_id().map(ToString::to_string)),
    );
    payload.insert(
        "asset_id".into(),
        json_string_or_null(context.asset_id().map(ToString::to_string)),
    );
    Ok((
        StatusCode::ACCEPTED,
        JsonBody(norito::json::native::Value::Object(payload)),
    ))
}

async fn handler_iso_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(msg_id): axum::extract::Path<String>,
) -> Result<(StatusCode, JsonBody<norito::json::native::Value>), Error> {
    check_access(&app, &headers, None, "v1/iso20022/status").await?;
    let runtime = match &app.iso_bridge {
        Some(rt) => rt.clone(),
        None => {
            return Err(Error::Query(
                iroha_data_model::ValidationFail::NotPermitted("iso20022 bridge disabled".into()),
            ));
        }
    };

    let mut status = runtime.message_status(&msg_id).ok_or_else(|| {
        Error::Query(iroha_data_model::ValidationFail::NotPermitted(
            "unknown ISO 20022 message identifier".into(),
        ))
    })?;

    if status.derived_status() != Pacs002Status::Acsc {
        // Opportunistically mark the message as settled when the transaction hash
        // is already committed. This keeps the pacs.002 sequence progressing even
        // before a dedicated watcher threads `mark_settled` from the pipeline.
        if let Some(hash_str) = status.transaction_hash() {
            if let Ok(hash) = hash_str.parse::<HashOf<SignedTransaction>>() {
                if app.state.has_committed_transaction(hash) {
                    runtime.mark_settled(&msg_id, SystemTime::now());
                    if let Some(updated) = runtime.message_status(&msg_id) {
                        status = updated;
                    }
                }
            }
        }
    }

    let updated_ms = status
        .updated_at()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);

    let mut payload = norito::json::native::Map::new();
    payload.insert(
        "message_id".into(),
        norito::json::native::Value::from(status.message_id().to_string()),
    );
    payload.insert(
        "status".into(),
        norito::json::native::Value::from(status.status_label().to_string()),
    );
    payload.insert(
        "pacs002_code".into(),
        norito::json::native::Value::from(status.pacs002_code().to_string()),
    );
    payload.insert(
        "transaction_hash".into(),
        json_string_or_null(status.transaction_hash().map(str::to_string)),
    );
    payload.insert(
        "detail".into(),
        json_string_or_null(status.detail().map(str::to_string)),
    );
    payload.insert(
        "hold_reason_code".into(),
        json_string_or_null(status.hold_reason_code().map(str::to_string)),
    );
    let change_codes = status
        .change_reason_codes()
        .iter()
        .cloned()
        .map(norito::json::native::Value::from)
        .collect::<Vec<_>>();
    payload.insert(
        "change_reason_codes".into(),
        norito::json::native::Value::Array(change_codes),
    );
    payload.insert(
        "updated_at_ms".into(),
        norito::json::native::Value::from(updated_ms),
    );
    payload.insert(
        "ledger_id".into(),
        json_string_or_null(status.ledger_id().map(str::to_string)),
    );
    payload.insert(
        "source_account_id".into(),
        json_string_or_null(status.source_account_id().map(str::to_string)),
    );
    payload.insert(
        "source_account_address".into(),
        json_string_or_null(status.source_account_address().map(str::to_string)),
    );
    payload.insert(
        "target_account_id".into(),
        json_string_or_null(status.target_account_id().map(str::to_string)),
    );
    payload.insert(
        "target_account_address".into(),
        json_string_or_null(status.target_account_address().map(str::to_string)),
    );
    payload.insert(
        "asset_definition_id".into(),
        json_string_or_null(status.asset_definition_id().map(str::to_string)),
    );
    payload.insert(
        "asset_id".into(),
        json_string_or_null(status.asset_id().map(str::to_string)),
    );
    Ok((
        StatusCode::OK,
        JsonBody(norito::json::native::Value::Object(payload)),
    ))
}

fn map_iso_error(err: MsgError) -> iroha_data_model::ValidationFail {
    use iroha_data_model::ValidationFail;

    match err {
        MsgError::ValidationFailed => {
            ValidationFail::NotPermitted("ISO 20022 validation failed".into())
        }
        MsgError::MissingField(field) => {
            ValidationFail::NotPermitted(format!("missing ISO 20022 field `{field}`"))
        }
        MsgError::TooManyOccurrences { field, max, actual } => ValidationFail::NotPermitted(
            format!("field `{field}` exceeds max occurrences ({actual} > {max})"),
        ),
        MsgError::InvalidIdentifier { field, kind } => {
            ValidationFail::NotPermitted(format!("invalid {kind} value for field `{field}`"))
        }
        MsgError::InvalidInstrument { field } => ValidationFail::NotPermitted(format!(
            "field `{field}` must contain a valid ISIN or CUSIP identifier"
        )),
        MsgError::InvalidValue { field, kind } => {
            ValidationFail::NotPermitted(format!("invalid {kind} value for field `{field}`"))
        }
        MsgError::InvalidFormat => {
            ValidationFail::NotPermitted("ISO 20022 message format is invalid".into())
        }
        MsgError::UnknownMessageType => {
            ValidationFail::NotPermitted("unsupported ISO 20022 message type".into())
        }
        MsgError::UnsupportedChannel => {
            ValidationFail::NotPermitted("unsupported ISO 20022 delivery channel".into())
        }
        MsgError::HttpStatus(code) => {
            ValidationFail::NotPermitted(format!("ISO 20022 transport returned HTTP status {code}"))
        }
        MsgError::NoActiveMessage => {
            ValidationFail::InternalError("ISO 20022 parser stack underflow".into())
        }
        MsgError::Io(err) => ValidationFail::InternalError(err.to_string()),
    }
}

#[cfg(feature = "app_api")]
async fn handler_post_vk_register(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::ZkVkRegisterDto>,
) -> Result<AxResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_post_vk_register(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            request,
        )
        .await
        .map(IntoResponse::into_response);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/zk/vk/register",
        app.api_token_enforced(),
    );
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    crate::routing::handle_post_vk_register(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    .map(IntoResponse::into_response)
}

#[cfg(feature = "app_api")]
async fn handler_post_vk_update(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: NoritoJson<crate::routing::ZkVkUpdateDto>,
) -> Result<AxResponse, Error> {
    if limits::is_allowed_by_cidr(&headers, None, &app.allow_nets) {
        return crate::routing::handle_post_vk_update(
            app.chain_id.clone(),
            app.queue.clone(),
            app.state.clone(),
            app.telemetry.clone(),
            request,
        )
        .await
        .map(IntoResponse::into_response);
    }
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/zk/vk/update", app.api_token_enforced());
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    crate::routing::handle_post_vk_update(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        request,
    )
    .await
    .map(IntoResponse::into_response)
}

async fn handler_post_transaction(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoVersioned(transaction): NoritoVersioned<iroha_data_model::transaction::SignedTransaction>,
) -> Result<impl IntoResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let auth_id = format!("{}", transaction.authority());
    let key = token_hdr.unwrap_or(auth_id);
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.tx_rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    let tx_hash = transaction.hash();
    routing::handle_transaction_with_metrics(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        transaction,
        app.telemetry.clone(),
        iroha_torii_shared::uri::TRANSACTION,
    )
    .await?;
    let submitted_at_height = u64::try_from(app.state.committed_height()).unwrap_or(0);
    let submitted_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    let payload = TransactionSubmissionReceiptPayload {
        tx_hash,
        submitted_at_ms,
        submitted_at_height,
        signer: app.da_receipt_signer.public_key().clone(),
    };
    let receipt = TransactionSubmissionReceipt::sign(payload, &app.da_receipt_signer);
    Ok((StatusCode::ACCEPTED, NoritoBody(receipt)))
}

async fn handler_proof_record_get(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    AxPath(id): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    ensure_proof_api_version(&app, negotiated, "/v1/proofs/{id}")?;
    let enforce = !limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    let start = std::time::Instant::now();
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        "/v1/proofs/{id}",
        1,
        enforce,
    )
    .await?;
    let NoritoBody(rec) = routing::handle_get_proof_record(app.state.clone(), AxPath(id)).await?;
    let etag_value = format!("\"{}:{}\"", rec.id.backend, hex::encode(rec.id.proof_hash));
    let cache_control_value = format!(
        "public, max-age={}",
        app.proof_limits.cache_max_age.as_secs().max(1)
    );
    if let Some(if_none_match) = headers
        .get(axum::http::header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    {
        let token = if_none_match
            .trim()
            .trim_start_matches("W/")
            .trim_matches('"');
        if token.eq_ignore_ascii_case(etag_value.trim_matches('"')) {
            app.telemetry.with_metrics(|tel| {
                tel.inc_torii_proof_cache_hit("/v1/proofs/{id}");
                tel.observe_torii_proof_request(
                    "/v1/proofs/{id}",
                    "not_modified",
                    0,
                    start.elapsed(),
                )
            });
            let mut resp = axum::response::Response::builder()
                .status(axum::http::StatusCode::NOT_MODIFIED)
                .body(axum::body::Body::empty())
                .map_err(|err| {
                    Error::Query(iroha_data_model::ValidationFail::InternalError(
                        err.to_string(),
                    ))
                })?;
            if let Ok(cache_header) = axum::http::HeaderValue::from_str(&cache_control_value) {
                resp.headers_mut()
                    .insert(axum::http::header::CACHE_CONTROL, cache_header);
            }
            if let Ok(etag) = axum::http::HeaderValue::from_str(&etag_value) {
                resp.headers_mut().insert(axum::http::header::ETAG, etag);
            }
            return Ok(resp);
        }
    }

    let bytes = norito::to_bytes(&rec).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            err.to_string(),
        ))
    })?;
    let body_len = bytes.len() as u64;
    enforce_proof_egress(&app, &headers, None, "/v1/proofs/{id}", body_len, enforce).await?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(bytes));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static(utils::NORITO_MIME_TYPE),
    );
    if let Ok(cache_header) = axum::http::HeaderValue::from_str(&cache_control_value) {
        resp.headers_mut()
            .insert(axum::http::header::CACHE_CONTROL, cache_header);
    }
    if let Ok(etag) = axum::http::HeaderValue::from_str(&etag_value) {
        resp.headers_mut().insert(axum::http::header::ETAG, etag);
    }
    app.telemetry.with_metrics(|tel| {
        tel.observe_torii_proof_request("/v1/proofs/{id}", "ok", body_len, start.elapsed())
    });
    Ok(resp)
}

async fn handler_proof_retention_status(
    State(app): State<SharedAppState>,
    Extension(negotiated): Extension<api_version::NegotiatedVersion>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    ensure_proof_api_version(
        &app,
        negotiated,
        iroha_torii_shared::uri::PROOF_RETENTION_STATUS,
    )?;
    let enforce = !limits::is_allowed_by_cidr(&headers, None, &app.allow_nets);
    check_proof_access(
        &app,
        negotiated,
        &headers,
        None,
        iroha_torii_shared::uri::PROOF_RETENTION_STATUS,
        1,
        enforce,
    )
    .await?;
    Ok(crate::utils::respond_with_format(
        routing::handle_proof_retention_status(app.state.clone()),
        format,
    ))
}

/// Debug endpoint exposing the current AXT proof cache state per dataspace.
#[cfg(feature = "telemetry")]
async fn handler_axt_proof_cache_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    check_access(&app, &headers, Some(remote_ip), "debug/axt/cache").await?;
    let snapshot = app.state.metrics().axt_debug_status();
    Ok(crate::utils::JsonBody(snapshot))
}

/// Fallback when telemetry is disabled.
#[cfg(not(feature = "telemetry"))]
async fn handler_axt_proof_cache_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let _ = (headers, remote);
    Ok(telemetry_unavailable_response(
        iroha_torii_shared::uri::AXT_PROOF_CACHE_STATUS,
        &app.telemetry,
    ))
}

async fn handler_pipeline_recovery(
    State(app): State<SharedAppState>,
    AxPath(height): AxPath<u64>,
) -> Result<impl IntoResponse, Error> {
    app.kura.read_pipeline_metadata(height).map_or_else(
        || {
            Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            )))
        },
        |sidecar| match norito::json::to_json_pretty(&sidecar.to_json_value()) {
            Ok(serialized) => {
                let body = axum::body::Body::from(serialized);
                Ok::<_, Error>(
                    axum::http::Response::builder()
                        .status(axum::http::StatusCode::OK)
                        .header(axum::http::header::CONTENT_TYPE, "application/json")
                        .body(body)
                        .unwrap(),
                )
            }
            Err(err) => Err(Error::SerializationFailure {
                context: "pipeline_recovery_sidecar",
                source: err,
            }),
        },
    )
}

#[derive(JsonDeserialize)]
struct PipelineStatusQuery {
    #[norito(default)]
    hash: Option<String>,
}

fn parse_signed_transaction_hash(raw: &str) -> Result<HashOf<SignedTransaction>, Error> {
    raw.trim()
        .parse::<HashOf<SignedTransaction>>()
        .map_err(|_| conversion_error("invalid signed transaction hash".to_owned()))
}

fn pipeline_status_payload(
    hash: &HashOf<SignedTransaction>,
    entry: &PipelineStatusEntry,
) -> norito::json::Value {
    let mut status = norito::json::Map::new();
    status.insert(
        "kind".into(),
        norito::json::Value::from(entry.kind.as_str()),
    );
    if let Some(height) = entry.block_height {
        status.insert(
            "block_height".into(),
            norito::json::Value::from(height.get()),
        );
    }
    let rejection = match entry.kind {
        PipelineStatusKind::Rejected => entry.rejection.as_ref().and_then(|reason| {
            norito::to_bytes(reason)
                .ok()
                .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
        }),
        _ => None,
    };
    status.insert(
        "content".into(),
        rejection
            .map(norito::json::Value::from)
            .unwrap_or(norito::json::Value::Null),
    );

    let mut content = norito::json::Map::new();
    content.insert("hash".into(), norito::json::Value::from(hash.to_string()));
    content.insert("status".into(), norito::json::Value::Object(status));

    let mut envelope = norito::json::Map::new();
    envelope.insert("kind".into(), norito::json::Value::from("Transaction"));
    envelope.insert("content".into(), norito::json::Value::Object(content));
    norito::json::Value::Object(envelope)
}

fn pipeline_status_from_state(
    app: &AppState,
    hash: &HashOf<SignedTransaction>,
) -> Option<PipelineStatusEntry> {
    let height = app.state.committed_transaction_height(hash)?;
    let height_u64 = u64::try_from(height.get()).ok()?;
    let height_nz = NonZeroU64::new(height_u64)?;
    let block = app.kura.get_block(height)?;
    let block_ref = block.as_ref();
    let external_total = block_ref.external_transactions().len();
    for (tx, result) in block_ref
        .external_transactions()
        .zip(block_ref.results().take(external_total))
    {
        if tx.hash() != *hash {
            continue;
        }
        let (kind, rejection) = match &result.0 {
            Ok(_) => (PipelineStatusKind::Applied, None),
            Err(reason) => (PipelineStatusKind::Rejected, Some(reason.clone())),
        };
        return Some(PipelineStatusEntry::fresh(kind, Some(height_nz), rejection));
    }
    None
}

async fn handler_pipeline_transaction_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    AxQuery(query): AxQuery<PipelineStatusQuery>,
) -> Result<Response, Error> {
    check_access(&app, &headers, None, "v1/pipeline/transactions/status").await?;
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(format) => format,
        Err(resp) => return Ok(resp),
    };
    let hash_raw = query
        .hash
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| conversion_error("missing hash query parameter".to_owned()))?;
    let hash = parse_signed_transaction_hash(hash_raw)?;
    app.pipeline_status_cache.refresh_pending_blocks(&app.kura);

    if let Some(entry) = app.pipeline_status_cache.lookup(&hash) {
        return Ok(crate::utils::respond_value_with_format(
            pipeline_status_payload(&hash, &entry),
            format,
        ));
    }

    let queued = app.queue.contains_pending_hash(hash.clone(), &app.state);
    if queued {
        let entry = PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None);
        app.pipeline_status_cache.record_entry(hash, entry.clone());
        return Ok(crate::utils::respond_value_with_format(
            pipeline_status_payload(&hash, &entry),
            format,
        ));
    }

    if let Some(entry) = pipeline_status_from_state(&app, &hash) {
        app.pipeline_status_cache.record_entry(hash, entry.clone());
        return Ok(crate::utils::respond_value_with_format(
            pipeline_status_payload(&hash, &entry),
            format,
        ));
    }

    Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::NotFound,
    )))
}

async fn handler_policy(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    #[allow(clippy::too_many_arguments, clippy::ref_option)]
    fn build_policy_body(
        require_token: bool,
        fee_policy: &FeePolicy,
        queue_len: u64,
        normal_th: usize,
        stream_th: usize,
        sub_th: usize,
        token_required: bool,
    ) -> axum::response::Response {
        let mut obj = norito::json::Map::new();
        obj.insert(
            "require_api_token".into(),
            norito::json::Value::from(require_token),
        );
        obj.insert(
            "token_required".into(),
            norito::json::Value::from(token_required),
        );
        match fee_policy.asset_id() {
            Some(asset) => obj.insert(
                "fee_asset_id".into(),
                norito::json::Value::from(asset.to_owned()),
            ),
            None => obj.insert("fee_asset_id".into(), norito::json::Value::Null),
        };
        match fee_policy.receiver() {
            Some(r) => obj.insert(
                "fee_receiver".into(),
                norito::json::Value::from(r.to_owned()),
            ),
            None => obj.insert("fee_receiver".into(), norito::json::Value::Null),
        };
        match fee_policy.amount() {
            Some(amount) => obj.insert("fee_amount".into(), norito::json::Value::from(amount)),
            None => obj.insert("fee_amount".into(), norito::json::Value::Null),
        };
        obj.insert("queue_len".into(), norito::json::Value::from(queue_len));
        obj.insert(
            "rate_limit_threshold".into(),
            norito::json::Value::from(normal_th as u64),
        );
        obj.insert(
            "stream_rate_limit_threshold".into(),
            norito::json::Value::from(stream_th as u64),
        );
        obj.insert(
            "subscription_rate_limit_threshold".into(),
            norito::json::Value::from(sub_th as u64),
        );
        let fees_enabled = fee_policy.is_enabled();
        let enforced = fees_enabled || (queue_len as usize) >= normal_th;
        let stream_enforced = fees_enabled || (queue_len as usize) >= stream_th;
        let sub_enforced = fees_enabled || (queue_len as usize) >= sub_th;
        obj.insert(
            "rate_limit_enforced".into(),
            norito::json::Value::from(enforced),
        );
        obj.insert(
            "stream_rate_limit_enforced".into(),
            norito::json::Value::from(stream_enforced),
        );
        obj.insert(
            "subscription_rate_limit_enforced".into(),
            norito::json::Value::from(sub_enforced),
        );
        let explain = format!(
            "fees_enabled={}, queue_len={}, thresholds(normal={}, stream={}, subscription={})",
            fees_enabled, queue_len, normal_th, stream_th, sub_th
        );
        obj.insert("explain".into(), norito::json::Value::from(explain));
        let body = norito::json::to_json_pretty(&norito::json::Value::Object(obj))
            .unwrap_or_else(|_| "{}".into());
        let mut resp = axum::response::Response::new(axum::body::Body::from(body));
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        resp
    }

    let queue_len = app.queue.active_len() as u64;
    let token_required = app.require_api_token && !app.api_tokens_set.is_empty();

    if limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.allow_nets) {
        return Ok(build_policy_body(
            app.require_api_token,
            &app.fee_policy,
            queue_len,
            app.high_load_tx_threshold,
            app.high_load_stream_tx_threshold,
            app.high_load_subscription_tx_threshold,
            token_required,
        ));
    }

    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "v1/policy", app.api_token_enforced());
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    Ok(build_policy_body(
        app.require_api_token,
        &app.fee_policy,
        queue_len,
        app.high_load_tx_threshold,
        app.high_load_stream_tx_threshold,
        app.high_load_subscription_tx_threshold,
        token_required,
    ))
}

async fn handler_signed_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    NoritoVersioned(query_request): NoritoVersioned<iroha_data_model::query::SignedQuery>,
) -> Result<Response, Error> {
    let tel = app.telemetry.clone();
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.allow_nets) {
        return routing::handle_queries_with_opts(
            app.query_service.clone(),
            app.state.clone(),
            query_request,
            tel.clone(),
            crate::NoritoQuery(QueryOptions::default()),
            format,
        )
        .await;
    }

    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = token_hdr.unwrap_or_else(|| "query".to_string());
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    routing::handle_queries_with_opts(
        app.query_service.clone(),
        app.state.clone(),
        query_request,
        tel,
        crate::NoritoQuery(QueryOptions::default()),
        format,
    )
    .await
}

#[cfg(feature = "connect")]
async fn handle_connect_ws_logic(
    app: SharedAppState,
    headers: axum::http::HeaderMap,
    q: routing::ConnectWsQuery,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    let bus = app.connect_bus.clone();
    let remote_ip = headers
        .get(limits::REMOTE_ADDR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let mut permit = match bus.pre_ws_handshake(remote_ip).await {
        Ok(permit) => permit,
        Err((code, msg)) => return (code, msg).into_response(),
    };
    let sid = match crate::connect::decode_sid(&q.sid) {
        Ok(sid) => sid,
        Err(_) => {
            permit.release().await;
            return (StatusCode::BAD_REQUEST, "connect: bad sid").into_response();
        }
    };
    let role = match q.role.as_str() {
        "app" => iroha_torii_shared::connect::Role::App,
        "wallet" => iroha_torii_shared::connect::Role::Wallet,
        other => {
            permit.release().await;
            return (
                StatusCode::BAD_REQUEST,
                format!("connect: bad role {other}"),
            )
                .into_response();
        }
    };
    let token = match resolve_connect_ws_token(&headers) {
        Ok(token) => token,
        Err(response) => {
            permit.release().await;
            return response;
        }
    };
    if let Err((code, msg)) = bus.authorize_token(sid, role, &token.token).await {
        permit.release().await;
        return (code, msg).into_response();
    }
    let ws = if let Some(protocol) = token.protocol {
        ws.protocols([protocol])
    } else {
        ws
    };
    ws.on_upgrade(move |ws| async move {
        let result = connect::handle_ws(bus, q, ws).await;
        permit.release().await;
        if let Err(e) = result {
            iroha_logger::warn!(%e, "connect ws session ended with error");
        }
    })
}

#[cfg(feature = "connect")]
#[allow(dead_code)]
fn _assert_websocket_send() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<WebSocketUpgrade>();
    assert_sync::<WebSocketUpgrade>();
}

#[cfg(feature = "connect")]
async fn handler_connect_ws(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    let query = match parse_connect_ws_query(raw_query.as_deref()) {
        Ok(q) => q,
        Err(response) => return response,
    };
    handle_connect_ws_logic(app, headers, query, ws).await
}

#[cfg(feature = "connect")]
async fn handler_connect_session(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(req): NoritoJson<routing::ConnectSessionRequest>,
) -> Result<JsonBody<routing::ConnectSessionResponse>> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let remote_ip = headers
        .get(limits::REMOTE_ADDR_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    app.connect_bus
        .pre_session_create(remote_ip)
        .await
        .map_err(|(code, msg)| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "connect session rejected ({code}): {msg}"
                )),
            ))
        })?;

    let missing_sid = req.sid.is_none();
    let chain_id = app.chain_id.clone();
    let JsonBody(response) = routing::handle_connect_session(chain_id, NoritoJson(req)).await?;
    assert!(!missing_sid, "connect session handler missing sid");

    let malformed = |msg: String| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(msg),
        ))
    };

    let sid_bytes = B64
        .decode(response.sid.as_bytes())
        .map_err(|err| malformed(format!("connect session sid decode failed: {err}")))?;
    let sid: crate::connect::Sid = sid_bytes
        .try_into()
        .map_err(|_| malformed("connect session sid had wrong length".into()))?;

    app.connect_bus
        .clone()
        .register_tokens(
            sid,
            response.token_app.clone(),
            response.token_wallet.clone(),
        )
        .await
        .map_err(|err| match err {
            crate::connect::RegisterSessionError::Exists => {
                malformed("connect session already exists for provided sid".to_string())
            }
            crate::connect::RegisterSessionError::Capacity => {
                malformed("connect session capacity reached; try again later".to_string())
            }
        })?;

    Ok(JsonBody(response))
}

#[cfg(feature = "connect")]
async fn handler_connect_session_delete(
    State(app): State<SharedAppState>,
    axum::extract::Path(sid_str): axum::extract::Path<String>,
) -> axum::response::Response {
    use axum::http::StatusCode;

    match crate::connect::decode_sid(&sid_str) {
        Ok(sid) => {
            let removed = app
                .connect_bus
                .clone()
                .terminate_session(sid, crate::connect::CLOSE_REASON_PURGED)
                .await;
            if removed {
                StatusCode::NO_CONTENT.into_response()
            } else {
                StatusCode::NOT_FOUND.into_response()
            }
        }
        Err(_) => (StatusCode::BAD_REQUEST, "connect: bad sid").into_response(),
    }
}

#[cfg(feature = "connect")]
#[allow(clippy::result_large_err)]
fn parse_connect_ws_query(
    raw: Option<&str>,
) -> Result<routing::ConnectWsQuery, axum::response::Response> {
    use axum::http::StatusCode;
    let mut sid = None;
    let mut role = None;
    if let Some(qs) = raw {
        for pair in qs.split('&') {
            if pair.is_empty() {
                continue;
            }
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("");
            let value_raw = parts.next().unwrap_or("");
            let value = match urlencoding::decode(value_raw) {
                Ok(decoded) => decoded.into_owned(),
                Err(_) => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "connect query has invalid encoding",
                    )
                        .into_response());
                }
            };
            match key {
                "sid" => sid = Some(value),
                "role" => role = Some(value),
                "token" => {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        "connect query must not include token; use Authorization or Sec-WebSocket-Protocol",
                    )
                        .into_response());
                }
                _ => {}
            }
        }
    }
    fn normalize_role(raw: &str) -> Option<String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "app" => Some("app".to_string()),
            "wallet" => Some("wallet".to_string()),
            _ => None,
        }
    }

    match (sid, role) {
        (Some(sid), Some(role_raw)) => {
            let Some(role) = normalize_role(&role_raw) else {
                return Err(
                    (StatusCode::BAD_REQUEST, "connect query has invalid role").into_response()
                );
            };
            Ok(routing::ConnectWsQuery { sid, role })
        }
        _ => Err((StatusCode::BAD_REQUEST, "connect query missing sid/role").into_response()),
    }
}

const CONNECT_PROTOCOL_TOKEN_PREFIX: &str = "iroha-connect.token.v1.";

#[derive(Debug)]
struct ConnectWsToken {
    token: String,
    protocol: Option<String>,
}

#[derive(Debug)]
struct ProtocolToken {
    token: String,
    protocol: String,
}

#[allow(clippy::result_large_err)]
fn resolve_connect_ws_token(
    headers: &axum::http::HeaderMap,
) -> Result<ConnectWsToken, axum::response::Response> {
    use axum::http::StatusCode;

    let auth_token = parse_authorization_token(headers)?;
    let protocol_token = parse_protocol_token(headers)?;

    if let Some(auth) = auth_token {
        if let Some(proto) = protocol_token.as_ref() {
            if proto.token != auth {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "connect: conflicting tokens in Authorization and Sec-WebSocket-Protocol headers",
                )
                    .into_response());
            }
            return Ok(ConnectWsToken {
                token: auth,
                protocol: Some(proto.protocol.clone()),
            });
        }
        return Ok(ConnectWsToken {
            token: auth,
            protocol: None,
        });
    }

    if let Some(proto) = protocol_token {
        return Ok(ConnectWsToken {
            token: proto.token,
            protocol: Some(proto.protocol),
        });
    }

    Err((
        StatusCode::BAD_REQUEST,
        "connect: missing token; send Authorization: Bearer <token> or Sec-WebSocket-Protocol: iroha-connect.token.v1.<base64url>",
    )
        .into_response())
}

#[cfg(all(test, feature = "connect"))]
mod connect_token_tests {
    use axum::http::{HeaderMap, StatusCode, header};

    use super::{parse_connect_ws_query, resolve_connect_ws_token};

    #[test]
    fn connect_query_rejects_token_param() {
        let err = parse_connect_ws_query(Some("sid=abc&role=app&token=deadbeef"))
            .expect_err("token param should be rejected");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn connect_query_rejects_invalid_role() {
        let err = parse_connect_ws_query(Some("sid=abc&role=operator"))
            .expect_err("invalid role should be rejected");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn connect_query_accepts_case_insensitive_role() {
        let ok = parse_connect_ws_query(Some("sid=abc&role=Wallet"))
            .expect("case-insensitive role should be accepted");
        assert_eq!(ok.role, "wallet");
    }

    #[test]
    fn resolve_connect_ws_token_requires_headers() {
        let headers = HeaderMap::new();
        let err = resolve_connect_ws_token(&headers).expect_err("missing token should fail");
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn resolve_connect_ws_token_accepts_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer test-token".parse().unwrap());
        let token = resolve_connect_ws_token(&headers).expect("bearer token ok");
        assert_eq!(token.token, "test-token");
        assert!(token.protocol.is_none());
    }
}

#[allow(clippy::result_large_err)]
fn parse_authorization_token(
    headers: &axum::http::HeaderMap,
) -> Result<Option<String>, axum::response::Response> {
    use axum::http::{StatusCode, header};

    let Some(value) = headers.get(header::AUTHORIZATION) else {
        return Ok(None);
    };
    let value_str = value
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "connect: invalid authorization header",
            )
                .into_response()
        })?
        .trim();
    let mut parts = value_str.split_whitespace();
    let scheme = parts.next().unwrap_or_default();
    let token = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            "connect: malformed authorization header",
        )
            .into_response());
    }
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err((
            StatusCode::BAD_REQUEST,
            "connect: authorization header must use Bearer scheme",
        )
            .into_response());
    }
    let token = token.trim();
    if token.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "connect: bearer token is empty").into_response());
    }
    Ok(Some(token.to_owned()))
}

#[allow(clippy::result_large_err)]
fn parse_protocol_token(
    headers: &axum::http::HeaderMap,
) -> Result<Option<ProtocolToken>, axum::response::Response> {
    use axum::http::{StatusCode, header};

    let mut values_iter = headers.get_all(header::SEC_WEBSOCKET_PROTOCOL).iter();
    let Some(first_value) = values_iter.next() else {
        return Ok(None);
    };
    for value in std::iter::once(first_value).chain(values_iter) {
        let value_str = value
            .to_str()
            .map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "connect: invalid Sec-WebSocket-Protocol header",
                )
                    .into_response()
            })?
            .trim();
        for entry in value_str.split(',') {
            let candidate = entry.trim();
            if let Some(encoded) = candidate.strip_prefix(CONNECT_PROTOCOL_TOKEN_PREFIX) {
                let token = decode_protocol_token(encoded)?;
                return Ok(Some(ProtocolToken {
                    token,
                    protocol: candidate.to_string(),
                }));
            }
        }
    }
    Ok(None)
}

#[allow(clippy::result_large_err)]
fn decode_protocol_token(encoded: &str) -> Result<String, axum::response::Response> {
    use axum::http::StatusCode;
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};

    let bytes = B64.decode(encoded).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "connect: invalid protocol token encoding",
        )
            .into_response()
    })?;
    if bytes.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "connect: protocol token is empty").into_response());
    }
    String::from_utf8(bytes).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "connect: protocol token must be valid utf-8",
        )
            .into_response()
    })
}

#[cfg(feature = "connect")]
async fn handler_connect_status(State(app): State<SharedAppState>) -> impl IntoResponse {
    let bus = app.connect_bus.clone();
    let status = bus.status().await;
    #[cfg(feature = "telemetry")]
    {
        if app.telemetry.allows_metrics() {
            let metrics = app.telemetry.metrics().await;
            metrics
                .torii_connect_sessions_total
                .set(status.sessions_total as u64);
            metrics
                .torii_connect_sessions_active
                .set(status.sessions_active as u64);
            metrics
                .torii_connect_buffered_sessions
                .set(status.buffered_sessions as u64);
            metrics
                .torii_connect_total_buffer_bytes
                .set(status.total_buffer_bytes as u64);
            metrics
                .torii_connect_dedupe_size
                .set(status.dedupe_size as u64);
            for entry in &status.per_ip_sessions {
                metrics
                    .torii_connect_per_ip_sessions
                    .with_label_values(&[entry.ip.as_str()])
                    .set(entry.sessions as u64);
            }
        }
    }
    JsonBody(status)
}

async fn handler_alias_voprf_evaluate(
    NoritoJson(request): NoritoJson<routing::AliasVoprfEvaluateRequestDto>,
) -> Result<JsonBody<routing::AliasVoprfEvaluateResponseDto>, Error> {
    let blinded_bytes =
        hex::decode(request.blinded_element_hex.trim_start_matches("0x")).map_err(|err| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(err.to_string()),
            ))
        })?;

    let evaluated = iroha_core::alias::evaluate_alias_voprf(&blinded_bytes).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(err.to_string()),
        ))
    })?;
    let evaluated_element_hex = hex::encode(evaluated.evaluated_element);
    let payload = routing::AliasVoprfEvaluateResponseDto {
        evaluated_element_hex,
        backend: routing::AliasVoprfBackendDto::Blake2b512Mock,
    };
    Ok(JsonBody(payload))
}

async fn handler_alias_resolve(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<routing::AliasResolveRequestDto>,
) -> Result<AxResponse, Error> {
    if request.alias.trim().is_empty() {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "alias must not be empty".to_string(),
            ),
        )));
    }

    if let Some(service) = &app.alias_service {
        return resolve_alias_via_service(service.as_ref(), &request.alias);
    }

    let Some(runtime) = app.iso_bridge.as_ref() else {
        return Ok(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    };

    match runtime.resolve_account(&request.alias) {
        Some(account_id) => {
            let index = runtime.resolve_alias_index(&request.alias);
            let alias = index
                .and_then(|idx| runtime.resolve_account_by_index(idx))
                .map(|(alias, _)| alias)
                .unwrap_or_else(|| normalise_alias(&request.alias));
            let account_id_string = account_id.to_string();
            alias_resolve_ok(
                &alias,
                &account_id_string,
                index.map(|idx| idx.0),
                "iso_bridge",
            )
        }
        None => Ok(axum::http::StatusCode::NOT_FOUND.into_response()),
    }
}

async fn handler_alias_resolve_index(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<routing::AliasResolveIndexRequestDto>,
) -> Result<AxResponse, Error> {
    if let Some(service) = &app.alias_service {
        return resolve_alias_index_via_service(service.as_ref(), request.index);
    }

    let Some(runtime) = app.iso_bridge.as_ref() else {
        return Ok(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    };

    let index = AliasIndex(request.index);
    match runtime.resolve_account_by_index(index) {
        Some((alias, account_id)) => {
            let account_id_string = account_id.to_string();
            alias_resolve_index_ok(index.0, &alias, &account_id_string, "iso_bridge")
        }
        None => Ok(axum::http::StatusCode::NOT_FOUND.into_response()),
    }
}

fn normalise_alias(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

#[cfg(feature = "telemetry")]
async fn handler_rbc_sessions(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
) -> Result<AxResponse, Error> {
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/rbc/sessions",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_developer_outputs() {
        return Ok(telemetry_unavailable_response(
            "/v1/sumeragi/rbc/sessions",
            &app.telemetry,
        ));
    }
    Ok(routing::handle_v1_sumeragi_rbc_sessions()
        .await?
        .into_response())
}

async fn handler_rbc_sample(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(request): NoritoJson<routing::RbcSampleRequestDto>,
) -> Result<AxResponse, Error> {
    use axum::response::IntoResponse;

    if !app.rbc_sampling_enabled {
        return Ok(axum::http::StatusCode::NOT_FOUND.into_response());
    }

    let Some(store_dir) = app.rbc_sampling_store_dir.as_ref() else {
        return Ok(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    };

    if app.api_tokens_set.is_empty() {
        return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
    }

    let token_hdr = headers.get("x-api-token").and_then(|v| v.to_str().ok());

    if !token_hdr
        .as_ref()
        .is_some_and(|t| app.api_tokens_set.contains(*t))
    {
        return Ok(axum::http::StatusCode::UNAUTHORIZED.into_response());
    }

    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/rbc/sample",
        app.api_token_enforced(),
    );
    if !limits::allow_conditionally(&app.rbc_sampling_limiter, &key, true).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    let requested = if request.count == 0 { 1 } else { request.count };
    let sample_count = requested.min(app.rbc_sampling_max_samples.max(1));

    let block_hash = match iroha_crypto::HashOf::<BlockHeader>::from_str(&request.block_hash) {
        Ok(hash) => hash,
        Err(err) => {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(err.to_string()),
            )));
        }
    };

    let session = match iroha_core::sumeragi::rbc_sampling::sample_from_store(
        store_dir,
        (block_hash, request.height, request.view),
        &app.rbc_chain_hash,
        &app.rbc_sampling_manifest,
        sample_count,
        request.seed,
    ) {
        Ok(Some(sample)) => sample,
        Ok(None) => return Ok(axum::http::StatusCode::NOT_FOUND.into_response()),
        Err(err) => {
            iroha_logger::warn!(?err, "rbc sampling failed");
            return Err(Error::Query(
                iroha_data_model::ValidationFail::InternalError(err.to_string()),
            ));
        }
    };

    let iroha_core::sumeragi::rbc_sampling::SessionSample {
        block_hash,
        height,
        view,
        total_chunks,
        chunk_root,
        payload_hash,
        samples,
    } = session;

    if samples.is_empty() {
        return Ok(axum::http::StatusCode::NOT_FOUND.into_response());
    }

    let bytes_total: u64 = samples.iter().map(|sample| sample.bytes.len() as u64).sum();
    if app.rbc_sampling_max_bytes > 0 && bytes_total > app.rbc_sampling_max_bytes {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    app.consume_rbc_sampling_budget(&key, bytes_total)?;

    let sample_indices: Vec<u32> = samples.iter().map(|s| s.index).collect();
    let dto_samples: Vec<routing::RbcChunkProofDto> = samples
        .iter()
        .map(|sample| {
            let proof = &sample.proof;
            let audit_path = proof
                .audit_path()
                .iter()
                .map(|opt| opt.map(|sib| hex::encode(sib.as_ref())))
                .collect();
            routing::RbcChunkProofDto {
                index: sample.index,
                chunk_hex: hex::encode(&sample.bytes),
                digest_hex: hex::encode(sample.digest),
                proof: routing::RbcMerkleProofDto {
                    leaf_index: proof.leaf_index(),
                    depth: Some(proof.audit_path().len() as u32),
                    audit_path,
                },
            }
        })
        .collect();

    let response = routing::RbcSampleResponseDto {
        block_hash: hex::encode(block_hash.as_ref().as_ref()),
        height,
        view,
        total_chunks,
        chunk_root: hex::encode(chunk_root.as_ref()),
        payload_hash: payload_hash.map(|hash| hex::encode(hash.as_ref())),
        samples: dto_samples,
    };

    let body = norito::json::to_json_pretty(&response).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );

    iroha_logger::info!(
        target: "torii::rbc_sampling",
        key = %key,
        block_hash = %request.block_hash,
        height,
        view,
        samples = ?sample_indices,
        bytes = bytes_total,
    );

    Ok(resp)
}

const LEDGER_HEADER_PAGE_CAP: u64 = 512;

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
struct StateRootResponse {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    /// State root if available (from commit QC or result Merkle root).
    state_root: iroha_crypto::Hash,
    /// Source of the state root for observability (`commit_qc` or `result_merkle_root`).
    source: String,
    /// Commit QC payload when available for attestation (subject hash, bitmap, aggregate sig).
    commit_qc: Option<iroha_data_model::consensus::Qc>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
struct StateProofResponse {
    height: u64,
    block_hash: HashOf<BlockHeader>,
    state_root: iroha_crypto::Hash,
    commit_qc: iroha_data_model::consensus::Qc,
}

async fn handler_ledger_headers(
    State(app): State<SharedAppState>,
    crate::NoritoQuery(window): crate::NoritoQuery<routing::HistoryWindowQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    let accept = headers.get(axum::http::header::ACCEPT);
    let format = match crate::utils::negotiate_response_format(accept) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    let current_height = u64::try_from(app.state.committed_height()).unwrap_or(u64::MAX);
    let start_height = match window.from {
        Some(0) => return Err(conversion_error("from must be at least 1".to_owned())),
        Some(from) => {
            if current_height == 0 {
                from
            } else {
                from.min(current_height)
            }
        }
        None => current_height,
    };
    let limit = window
        .limit
        .filter(|&lim| lim > 0)
        .unwrap_or(LEDGER_HEADER_PAGE_CAP)
        .min(LEDGER_HEADER_PAGE_CAP)
        .min(usize::MAX as u64) as usize;

    let mut headers: Vec<BlockHeader> = Vec::new();
    let mut height = start_height;
    while height >= 1 && headers.len() < limit {
        if height > usize::MAX as u64 {
            return Err(conversion_error(
                "block height exceeds host pointer width".to_owned(),
            ));
        }
        let Some(nz_height) = NonZeroUsize::new(height as usize) else {
            break;
        };
        if let Some(block) = app.state.block_by_height(nz_height) {
            headers.push(block.header());
        } else {
            break;
        }
        if height == 1 {
            break;
        }
        height -= 1;
    }

    match format {
        ResponseFormat::Norito => Ok(NoritoBody(headers).into_response()),
        ResponseFormat::Json => {
            let body = norito::json::to_json_pretty(&headers).map_err(|err| {
                Error::Query(iroha_data_model::ValidationFail::InternalError(
                    err.to_string(),
                ))
            })?;
            let mut resp = Response::new(Body::from(body));
            resp.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(resp)
        }
    }
}

async fn handler_ledger_state_root(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    let accept = headers.get(axum::http::header::ACCEPT);
    let format = match crate::utils::negotiate_response_format(accept) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let height_nz = NonZeroU64::new(height)
        .ok_or_else(|| conversion_error("height must be at least 1".to_owned()))?;
    let Some(height_usize) = NonZeroUsize::new(
        height_nz
            .get()
            .try_into()
            .map_err(|_| conversion_error("height exceeds host pointer width".to_owned()))?,
    ) else {
        return Err(conversion_error("height must be at least 1".to_owned()));
    };

    let Some(block) = app.state.block_by_height(height_usize) else {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )));
    };
    let block_hash = block.hash();
    let world = app.state.world_view();
    let commit_qc = world.commit_qcs().get(&block_hash).cloned();
    let payload = if let Some(qc) = commit_qc {
        StateRootResponse {
            height,
            block_hash,
            state_root: qc.post_state_root,
            source: "commit_qc".to_owned(),
            commit_qc: Some(qc),
        }
    } else if let Some(result_root) = block.header().result_merkle_root() {
        StateRootResponse {
            height,
            block_hash,
            state_root: iroha_crypto::Hash::prehashed(*result_root.as_ref()),
            source: "result_merkle_root".to_owned(),
            commit_qc: None,
        }
    } else {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )));
    };

    match format {
        ResponseFormat::Norito => Ok(NoritoBody(payload).into_response()),
        ResponseFormat::Json => {
            let body = norito::json::to_json_pretty(&payload).map_err(|e| {
                Error::Query(iroha_data_model::ValidationFail::InternalError(
                    e.to_string(),
                ))
            })?;
            let mut resp = Response::new(Body::from(body));
            resp.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(resp)
        }
    }
}

async fn handler_ledger_state_proof(
    State(app): State<SharedAppState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
    headers: axum::http::HeaderMap,
) -> Result<Response, Error> {
    let accept = headers.get(axum::http::header::ACCEPT);
    let format = match crate::utils::negotiate_response_format(accept) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let height_nz = NonZeroU64::new(height)
        .ok_or_else(|| conversion_error("height must be at least 1".to_owned()))?;
    let Some(height_usize) = NonZeroUsize::new(
        height_nz
            .get()
            .try_into()
            .map_err(|_| conversion_error("height exceeds host pointer width".to_owned()))?,
    ) else {
        return Err(conversion_error("height must be at least 1".to_owned()));
    };

    let Some(block) = app.state.block_by_height(height_usize) else {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )));
    };
    let block_hash = block.hash();
    // Ledger state proof requires a persisted commit QC; avoid placeholder synthesis.
    let world = app.state.world_view();
    let commit_qc = world
        .commit_qcs()
        .get(&block_hash)
        .cloned()
        .ok_or_else(|| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            ))
        })?;
    let payload = StateProofResponse {
        height,
        block_hash,
        state_root: commit_qc.post_state_root,
        commit_qc,
    };

    match format {
        ResponseFormat::Norito => Ok(NoritoBody(payload).into_response()),
        ResponseFormat::Json => {
            let body = norito::json::to_json_pretty(&payload).map_err(|e| {
                Error::Query(iroha_data_model::ValidationFail::InternalError(
                    e.to_string(),
                ))
            })?;
            let mut resp = Response::new(Body::from(body));
            resp.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(resp)
        }
    }
}

async fn handler_block_proof(
    State(app): State<SharedAppState>,
    axum::extract::Path((height, entry_hex)): axum::extract::Path<(u64, String)>,
) -> Result<NoritoBody<BlockProofs>, Error> {
    let block_height = NonZeroU64::new(height)
        .ok_or_else(|| conversion_error("block height must be at least 1".to_owned()))?;
    let normalized = entry_hex.trim_start_matches("0x");
    let entry_hash: HashOf<TransactionEntrypoint> = normalized
        .parse()
        .map_err(|err| conversion_error(format!("invalid entry hash: {err}")))?;

    let proofs = app
        .state
        .block_proofs_for_entry(block_height, entry_hash)
        .map_err(|err| match err {
            BlockProofError::ZeroHeight | BlockProofError::HeightOutOfRange(_) => {
                conversion_error(err.to_string())
            }
            BlockProofError::BlockNotFound(_)
            | BlockProofError::MissingResults(_)
            | BlockProofError::EntrypointNotFound { .. }
            | BlockProofError::ExecutionResultMissing { .. }
            | BlockProofError::MerkleProofUnavailable { .. } => {
                Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::NotFound,
                ))
            }
        })?;
    #[cfg(feature = "connect")]
    if app.connect_enabled {
        if let Err(err) = app
            .connect_bus
            .broadcast_block_proof(block_height, &entry_hash, &proofs)
            .await
        {
            iroha_logger::warn!(
                %err,
                height,
                entry_hex = %entry_hex,
                "connect block proof broadcast failed"
            );
        }
    }
    Ok(NoritoBody(proofs))
}

async fn handler_sumeragi_vrf_commit(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(request): NoritoJson<routing::VrfCommitRequestDto>,
) -> Result<AxResponse, Error> {
    use axum::response::IntoResponse;

    let Some(handle) = app.sumeragi.clone() else {
        return Ok(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    };

    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let token_hdr = headers.get("x-api-token").and_then(|v| v.to_str().ok());
        let ok = token_hdr
            .as_ref()
            .is_some_and(|token| app.api_tokens_set.contains(*token));
        if !ok {
            return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
        }
    }

    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/vrf/commit",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    routing::handle_post_sumeragi_vrf_commit(handle, request)
}

async fn handler_sumeragi_vrf_reveal(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    NoritoJson(request): NoritoJson<routing::VrfRevealRequestDto>,
) -> Result<AxResponse, Error> {
    use axum::response::IntoResponse;

    let Some(handle) = app.sumeragi.clone() else {
        return Ok(axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response());
    };

    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let token_hdr = headers.get("x-api-token").and_then(|v| v.to_str().ok());
        let ok = token_hdr
            .as_ref()
            .is_some_and(|token| app.api_tokens_set.contains(*token));
        if !ok {
            return Ok(axum::http::StatusCode::FORBIDDEN.into_response());
        }
    }

    let key = rate_limit_key(
        &headers,
        None,
        "v1/sumeragi/vrf/reveal",
        app.api_token_enforced(),
    );
    if !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }

    routing::handle_post_sumeragi_vrf_reveal(handle, request)
}

#[cfg(feature = "telemetry")]
async fn handler_status_root_v2(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
) -> Result<impl IntoResponse, Error> {
    // Token + rate limit only (no remote IP CIDR check here)
    let token_hdr = headers
        .get("x-api-token")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);
    if app.require_api_token && !app.api_tokens_set.is_empty() {
        let ok = token_hdr
            .as_ref()
            .is_some_and(|t| app.api_tokens_set.contains(t));
        if !ok {
            return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
            )));
        }
    }
    let key = rate_limit_key(&headers, None, "status", app.api_token_enforced());
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if enforce && !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_metrics() {
        return Ok(telemetry_unavailable_response(uri::STATUS, &app.telemetry));
    }
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    routing::handle_status(&app.telemetry, accept.map(|e| e.0), None, nexus_enabled).await
}

#[cfg(feature = "telemetry")]
async fn handler_status_tail_v2(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    accept: Option<utils::extractors::ExtractAccept>,
    AxPath(tail): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    validate_api_token(&app, &headers)?;
    let key = rate_limit_key(&headers, None, "status", app.api_token_enforced());
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    if enforce && !app.rate_limiter.allow(&key).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    if !app.telemetry.allows_metrics() {
        return Ok(telemetry_unavailable_response(uri::STATUS, &app.telemetry));
    }
    let nexus_enabled = app.state.nexus_snapshot().enabled;
    routing::handle_status(
        &app.telemetry,
        accept.map(|e| e.0),
        Some(&tail),
        nexus_enabled,
    )
    .await
}

// -------------- Runtime handlers (removed AppState-based; use closures in router) --------------
// (re-exports consolidated above)
mod da;
pub use self::da::compute_taikai_ingest_tags;
mod stream;
#[cfg(feature = "app_api")]
pub(crate) mod webhook;
#[cfg(feature = "app_api")]
pub mod zk_attachments;
#[cfg(feature = "app_api")]
pub mod zk_prover;

const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_mins(1);
const HEADER_NORITO_RPC_ERROR: &str = "x-iroha-error-code";
const NORITO_RPC_RETRY_AFTER_SECONDS: &str = "300";
const HEADER_API_TOKEN: &str = "x-api-token";
const HEADER_MTLS_FORWARD: &str = "x-forwarded-client-cert";
const NORITO_RPC_DISABLED_CODE: &str = "norito_rpc_disabled";
const NORITO_RPC_CANARY_DENIED_CODE: &str = "norito_rpc_canary_denied";
const NORITO_RPC_MTLS_REQUIRED_CODE: &str = "norito_rpc_mtls_required";

#[derive(Clone, Debug)]
enum FeePolicy {
    Disabled,
    Manual {
        asset_id: String,
        amount: u64,
        receiver: String,
    },
}

impl FeePolicy {
    fn is_enabled(&self) -> bool {
        matches!(self, Self::Manual { .. })
    }

    fn asset_id(&self) -> Option<&str> {
        match self {
            Self::Manual { asset_id, .. } => Some(asset_id.as_str()),
            Self::Disabled => None,
        }
    }

    fn amount(&self) -> Option<u64> {
        match self {
            Self::Manual { amount, .. } => Some(*amount),
            Self::Disabled => None,
        }
    }

    fn receiver(&self) -> Option<&str> {
        match self {
            Self::Manual { receiver, .. } => Some(receiver.as_str()),
            Self::Disabled => None,
        }
    }
}

#[cfg(feature = "app_api")]
#[derive(Clone)]
struct AccountOnboardingSigner {
    authority: AccountId,
    private_key: ExposedPrivateKey,
    allowed_domain: Option<DomainId>,
}

#[cfg(feature = "app_api")]
#[derive(Clone)]
struct OfflineIssuerSigner {
    operator_keypair: KeyPair,
    allowed_controllers: Vec<AccountId>,
}

/// Main network handler and the only entrypoint of the Iroha.
pub struct Torii {
    chain_id: Arc<ChainId>,
    kiso: KisoHandle,
    queue: Arc<Queue>,
    pipeline_status_cache: Arc<PipelineStatusCache>,
    events: EventsSender,
    query_service: LiveQueryStoreHandle,
    kura: Arc<Kura>,
    transaction_max_content_len: ConfigBytes<u64>,
    ws_message_timeout: Duration,
    address: WithOrigin<SocketAddr>,
    state: Arc<CoreState>,
    telemetry: routing::MaybeTelemetry,
    telemetry_profile: TelemetryProfile,
    api_versions: api_version::ApiVersionPolicy,
    online_peers: OnlinePeersProvider,
    #[cfg(all(feature = "app_api", feature = "telemetry"))]
    peer_telemetry_urls: Vec<telemetry::peers::ToriiUrl>,
    #[cfg(all(feature = "app_api", feature = "telemetry"))]
    peer_geo: telemetry::peers::GeoLookupConfig,
    sumeragi: Option<iroha_core::sumeragi::SumeragiHandle>,
    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    p2p: Option<iroha_core::IrohaNetwork>,
    // Query and transaction rate limits (operator-local)
    #[allow(dead_code)]
    query_rate_per_authority_per_sec: Option<std::num::NonZeroU32>,
    #[allow(dead_code)]
    query_burst_per_authority: Option<std::num::NonZeroU32>,
    #[allow(dead_code)]
    tx_rate_per_authority_per_sec: Option<std::num::NonZeroU32>,
    #[allow(dead_code)]
    tx_burst_per_authority: Option<std::num::NonZeroU32>,
    #[allow(dead_code)]
    deploy_rate_per_origin_per_sec: Option<std::num::NonZeroU32>,
    #[allow(dead_code)]
    deploy_burst_per_origin: Option<std::num::NonZeroU32>,
    rate_limiter: limits::RateLimiter,
    tx_rate_limiter: limits::RateLimiter,
    deploy_rate_limiter: limits::RateLimiter,
    proof_rate_limiter: limits::RateLimiter,
    proof_egress_limiter: limits::RateLimiter,
    content_request_limiter: limits::RateLimiter,
    content_egress_limiter: limits::RateLimiter,
    proof_limits: routing::ProofApiLimits,
    zk_prover_keys_dir: PathBuf,
    zk_ivm_prove_max_inflight: usize,
    zk_ivm_prove_max_queue: usize,
    zk_ivm_prove_job_ttl_ms: u64,
    zk_ivm_prove_job_max_entries: usize,
    content_config: iroha_config::parameters::actual::Content,
    preauth_gate: Arc<limits::PreAuthGate>,
    fee_policy: FeePolicy,
    norito_rpc: iroha_config::parameters::actual::NoritoRpcTransport,
    mcp: iroha_config::parameters::actual::ToriiMcp,
    mcp_rate_limiter: limits::RateLimiter,
    require_api_token: bool,
    api_tokens_set: std::sync::Arc<std::collections::HashSet<String>>,
    operator_auth: Arc<operator_auth::OperatorAuth>,
    operator_signatures: Arc<operator_signatures::OperatorSignatures>,
    soranet_privacy_ingest: iroha_config::parameters::actual::SoranetPrivacyIngest,
    soranet_privacy_tokens: std::sync::Arc<std::collections::HashSet<String>>,
    soranet_privacy_allow_nets: std::sync::Arc<Vec<limits::IpNet>>,
    soranet_privacy_rate_limiter: limits::RateLimiter,
    allow_nets: std::sync::Arc<Vec<limits::IpNet>>,
    high_load_tx_threshold: usize,
    high_load_stream_tx_threshold: usize,
    high_load_subscription_tx_threshold: usize,
    #[cfg(feature = "connect")]
    connect_bus: connect::Bus,
    #[cfg(feature = "connect")]
    connect_enabled: bool,
    #[cfg(feature = "push")]
    push: Option<push::PushBridge>,
    #[cfg(feature = "push")]
    push_rate_limiter: limits::RateLimiter,
    iso_bridge: Option<Arc<Iso20022BridgeRuntime>>,
    alias_service: Option<Arc<AliasService>>,
    rbc_store_dir: Option<PathBuf>,
    rbc_sampling: iroha_config::parameters::actual::RbcSampling,
    da_receipt_signer: KeyPair,
    da_ingest: iroha_config::parameters::actual::DaIngest,
    #[cfg(feature = "app_api")]
    sorafs_cache: Option<Arc<RwLock<sorafs::ProviderAdvertCache>>>,
    #[cfg(feature = "app_api")]
    sorafs_node: sorafs_node::NodeHandle,
    #[cfg(feature = "app_api")]
    sorafs_limits: Arc<sorafs::SorafsQuotaEnforcer>,
    #[cfg(feature = "app_api")]
    por_coordinator: Arc<sorafs::PorCoordinator>,
    #[cfg(feature = "app_api")]
    por_runtime: Option<Arc<sorafs::PorCoordinatorRuntime>>,
    #[cfg(feature = "app_api")]
    repair_runtime: Option<Arc<sorafs::RepairWorkerRuntime>>,
    #[cfg(feature = "app_api")]
    gc_runtime: Option<Arc<sorafs::GcSweeperRuntime>>,
    #[cfg(feature = "app_api")]
    sorafs_alias_cache: sorafs::AliasCachePolicy,
    #[cfg(feature = "app_api")]
    sorafs_alias_enforcement: sorafs::AliasCacheEnforcement,
    #[cfg(feature = "app_api")]
    sorafs_gateway: iroha_config::parameters::actual::SorafsGateway,
    #[cfg(feature = "app_api")]
    sorafs_gateway_security: Option<GatewaySecurityComponents>,
    #[cfg(feature = "app_api")]
    sorafs_pin_policy: sorafs::PinSubmissionPolicy,
    #[cfg(feature = "app_api")]
    sorafs_admission: Option<Arc<sorafs::AdmissionRegistry>>,
    #[cfg(feature = "app_api")]
    stream_token_issuer: Option<Arc<sorafs::StreamTokenIssuer>>,
    #[cfg(feature = "app_api")]
    offline_issuer: Option<OfflineIssuerSigner>,
    #[cfg(feature = "app_api")]
    uaid_onboarding: Option<AccountOnboardingSigner>,
}

impl Torii {
    #[cfg(feature = "telemetry")]
    #[allow(clippy::unused_self)]
    fn add_telemetry_routes(&self, builder: &mut RouterBuilder) {
        builder.apply_with_state(|router, state| {
            let operator_layer = axum::middleware::from_fn_with_state(
                state.clone(),
                operator_signatures::enforce_operator_access,
            );
            let operator_router = Router::new()
                // Telemetry-gated Sumeragi endpoints (runtime gate inside handlers)
                .route(
                    "/v1/sumeragi/rbc",
                    get(handler_rbc_status).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/sumeragi/rbc/delivered/{height}/{view}",
                    get(handler_rbc_delivered_height_view).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/sumeragi/pacemaker",
                    get(handler_pacemaker_status).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/sumeragi/phases",
                    get(handler_sumeragi_phases).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/debug/axt/cache",
                    get(handler_debug_axt_cache).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/debug/witness",
                    get(handler_debug_witness).layer(operator_layer.clone()),
                );

            let public_router = Router::new()
                .route(
                    routing::SORANET_PRIVACY_EVENT_ENDPOINT,
                    axum::routing::post(handler_post_soranet_privacy_event),
                )
                .route(
                    routing::SORANET_PRIVACY_SHARE_ENDPOINT,
                    axum::routing::post(handler_post_soranet_privacy_share),
                )
                .route(
                    "/v1/assets/{definition_id}/holders",
                    get(handler_asset_holders),
                )
                .route(
                    "/v1/assets/{definition_id}/holders/query",
                    post(handler_asset_holders_query),
                )
                // `/status` and `/metrics` are used by localnet/perf harnesses and typical
                // monitoring setups; keep them outside of operator signature middleware.
                .route(uri::STATUS, get(handler_status_root))
                .route(
                    &format!("{}/{{*tail}}", uri::STATUS),
                    get(handler_status_tail),
                )
                .route("/v1/soracloud/status", get(handler_soracloud_status))
                .route(uri::METRICS, get(handler_metrics));

            router.merge(operator_router).merge(public_router)
        });
    }

    #[cfg(not(feature = "telemetry"))]
    #[allow(clippy::unused_self)]
    fn add_telemetry_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router
                .route(uri::STATUS, get(routing::telemetry_not_implemented))
                .route(
                    &format!("{}/{{*rest}}", uri::STATUS),
                    get(routing::telemetry_not_implemented),
                )
                .route(
                    "/v1/debug/axt/cache",
                    get(routing::telemetry_not_implemented),
                )
                .route("/v1/debug/witness", get(routing::telemetry_not_implemented))
                .route(
                    "/v1/assets/{definition_id}/holders",
                    get(handler_asset_holders),
                )
                .route(
                    "/v1/assets/{definition_id}/holders/query",
                    post(handler_asset_holders_query),
                )
                .route(
                    "/v1/soracloud/status",
                    get(routing::telemetry_not_implemented),
                )
                .route(uri::METRICS, get(routing::telemetry_not_implemented))
        });
    }

    #[cfg(feature = "app_api")]
    fn spawn_pin_registry_metrics_worker(&self, shutdown_signal: ShutdownSignal) {
        if !self.telemetry.allows_metrics() {
            return;
        }

        let state = self.state.clone();
        let telemetry = self.telemetry.clone();

        tokio::spawn(async move {
            const PIN_REGISTRY_METRICS_INTERVAL_SECS: u64 = 30;
            let mut ticker =
                tokio::time::interval(Duration::from_secs(PIN_REGISTRY_METRICS_INTERVAL_SECS));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            if let Err(err) = Self::sample_pin_registry_metrics(&state, &telemetry) {
                iroha_logger::error!(
                    ?err,
                    "failed to collect initial SoraFS pin registry metrics snapshot"
                );
            }

            loop {
                tokio::select! {
                    _ = shutdown_signal.receive() => break,
                    _ = ticker.tick() => {
                        if let Err(err) = Self::sample_pin_registry_metrics(&state, &telemetry) {
                            iroha_logger::error!(?err, "failed to collect SoraFS pin registry metrics snapshot");
                        }
                    }
                }
            }
        });
    }

    #[cfg(feature = "app_api")]
    fn sample_pin_registry_metrics(
        state: &Arc<CoreState>,
        telemetry: &routing::MaybeTelemetry,
    ) -> Result<(), crate::sorafs::registry::PinRegistryError> {
        let world = state.world_view();
        let snapshot = crate::sorafs::registry::collect_pin_registry(&world)?;
        crate::sorafs::registry::record_pin_registry_metrics(telemetry, &snapshot);
        Ok(())
    }

    #[cfg(feature = "app_api")]
    fn spawn_por_ingestion_metrics_worker(&self, shutdown_signal: ShutdownSignal) {
        if !self.telemetry.allows_metrics() || !self.sorafs_node.is_enabled() {
            return;
        }

        let telemetry = self.telemetry.clone();
        let sorafs_node = self.sorafs_node.clone();

        tokio::spawn(async move {
            const POR_INGEST_METRICS_INTERVAL_SECS: u64 = 30;
            let mut ticker =
                tokio::time::interval(Duration::from_secs(POR_INGEST_METRICS_INTERVAL_SECS));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            let mut known_pairs: HashSet<([u8; 32], [u8; 32])> = HashSet::new();

            Self::sample_por_ingestion_metrics(&sorafs_node, &telemetry, &mut known_pairs);

            loop {
                tokio::select! {
                    _ = shutdown_signal.receive() => break,
                    _ = ticker.tick() => {
                        Self::sample_por_ingestion_metrics(
                            &sorafs_node,
                            &telemetry,
                            &mut known_pairs,
                        );
                    }
                }
            }
        });
    }

    #[cfg(feature = "app_api")]
    fn sample_por_ingestion_metrics(
        sorafs_node: &sorafs_node::NodeHandle,
        telemetry: &routing::MaybeTelemetry,
        known_pairs: &mut HashSet<([u8; 32], [u8; 32])>,
    ) {
        let statuses = sorafs_node.por_ingestion_overview();
        let mut active_pairs: HashSet<([u8; 32], [u8; 32])> = HashSet::new();

        telemetry.with_metrics(|metrics| {
            for status in &statuses {
                let pair = (status.manifest_digest, status.provider_id);
                active_pairs.insert(pair);

                let manifest_hex = hex::encode(status.manifest_digest);
                let provider_hex = hex::encode(status.provider_id);
                metrics.record_sorafs_por_ingestion_backlog(
                    &provider_hex,
                    &manifest_hex,
                    status.pending_challenges,
                );
                metrics.record_sorafs_por_ingestion_failures(
                    &provider_hex,
                    &manifest_hex,
                    status.failures_total,
                );
            }
        });

        telemetry.with_metrics(|metrics| {
            for pair in known_pairs.iter() {
                if active_pairs.contains(pair) {
                    continue;
                }
                let manifest_hex = hex::encode(pair.0);
                let provider_hex = hex::encode(pair.1);
                metrics.record_sorafs_por_ingestion_backlog(&provider_hex, &manifest_hex, 0);
                metrics.record_sorafs_por_ingestion_failures(&provider_hex, &manifest_hex, 0);
            }
        });

        *known_pairs = active_pairs;
    }

    fn add_sumeragi_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply_with_state(|router, state| {
            let operator_layer = axum::middleware::from_fn_with_state(
                state.clone(),
                operator_signatures::enforce_operator_access,
            );
            let sumeragi = Router::new()
                .route(
                    "/v1/sumeragi/evidence/count",
                    get(handler_sumeragi_evidence_count),
                )
                .route("/v1/sumeragi/evidence", get(handler_sumeragi_evidence));

            #[cfg(feature = "telemetry")]
            let sumeragi = sumeragi
                .route("/v1/sumeragi/new_view/sse", get(handler_new_view_sse))
                .route("/v1/sumeragi/new_view/json", get(handler_new_view_json))
                .route("/v1/sumeragi/status", get(handler_sumeragi_status))
                .route("/v1/sumeragi/status/sse", get(handler_sumeragi_status_sse))
                .route("/v1/sumeragi/leader", get(handler_sumeragi_leader))
                .route("/v1/sumeragi/bls_keys", get(handler_sumeragi_bls_keys))
                .route("/v1/sumeragi/qc", get(handler_sumeragi_qc))
                .route(
                    "/v1/sumeragi/checkpoints",
                    get(handler_sumeragi_checkpoints),
                )
                .route(
                    "/v1/sumeragi/commit-certificates",
                    get(handler_sumeragi_commit_qcs),
                )
                .route(
                    "/v1/bridge/finality/{height}",
                    get(handler_bridge_finality_proof),
                )
                .route(
                    "/v1/bridge/finality/bundle/{height}",
                    get(handler_bridge_finality_bundle),
                )
                .route(
                    iroha_torii_shared::uri::SUMERAGI_VALIDATOR_SETS,
                    get(handler_sumeragi_validator_sets),
                )
                .route(
                    iroha_torii_shared::uri::SUMERAGI_VALIDATOR_SET_BY_HEIGHT,
                    get(handler_sumeragi_validator_set_by_height),
                )
                .route(
                    "/v1/sumeragi/consensus-keys",
                    get(handler_sumeragi_consensus_keys),
                )
                .route(
                    "/v1/sumeragi/key-lifecycle",
                    get(handler_sumeragi_key_lifecycle),
                )
                .route("/v1/sumeragi/telemetry", get(handler_sumeragi_telemetry))
                .route("/v1/sumeragi/params", get(handler_sumeragi_params))
                .route("/v1/sumeragi/rbc/sessions", get(handler_rbc_sessions))
                .route("/v1/sumeragi/commit_qc/{hash}", get(handler_commit_qc))
                .route("/v1/sumeragi/collectors", get(handler_sumeragi_collectors));

            let sumeragi = sumeragi
                .route(
                    "/v1/sumeragi/vrf/penalties/{epoch}",
                    get(handler_sumeragi_vrf_penalties),
                )
                .route(
                    "/v1/sumeragi/vrf/epoch/{epoch}",
                    get(handler_sumeragi_vrf_epoch),
                );

            let operator_sumeragi = Router::new()
                .route(
                    "/v1/sumeragi/evidence",
                    post(handler_sumeragi_evidence_submit).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/sumeragi/vrf/commit",
                    post(handler_sumeragi_vrf_commit).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/sumeragi/vrf/reveal",
                    post(handler_sumeragi_vrf_reveal).layer(operator_layer.clone()),
                )
                .route(
                    "/v1/sumeragi/rbc/sample",
                    post(handler_rbc_sample).layer(operator_layer.clone()),
                );

            router.merge(sumeragi).merge(operator_sumeragi)
        });
    }

    fn add_core_info_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply_with_state(|router, state| {
            let operator_layer = axum::middleware::from_fn_with_state(
                state.clone(),
                operator_signatures::enforce_operator_access,
            );
            let operator_router = Router::new()
                .route(
                    uri::CONFIGURATION,
                    get(handler_get_configuration)
                        .post(handler_post_configuration)
                        .layer(operator_layer.clone()),
                )
                .route(
                    iroha_torii_shared::uri::NEXUS_LANE_LIFECYCLE,
                    post(handler_post_nexus_lane_lifecycle).layer(operator_layer.clone()),
                );
            let public_router = Router::new()
                .route(uri::API_VERSION, get(handler_version))
                .route(uri::API_VERSIONS, get(handler_api_versions))
                .route(uri::PEERS, get(handler_peers))
                .route(uri::HEALTH, get(handler_health))
                .route(uri::LEDGER_HEADERS, get(handler_ledger_headers))
                .route(uri::LEDGER_STATE_ROOT, get(handler_ledger_state_root))
                .route(uri::LEDGER_STATE_PROOF, get(handler_ledger_state_proof))
                .route(uri::LEDGER_BLOCK_PROOF, get(handler_block_proof));
            router.merge(operator_router).merge(public_router)
        });
    }

    fn add_alias_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router
                .route(
                    "/v1/aliases/voprf/evaluate",
                    post(handler_alias_voprf_evaluate),
                )
                .route("/v1/aliases/resolve", post(handler_alias_resolve))
                .route(
                    "/v1/aliases/resolve_index",
                    post(handler_alias_resolve_index),
                )
        });
    }

    fn add_time_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router
                .route("/v1/time/now", get(handler_time_now))
                .route("/v1/time/status", get(handler_time_status))
        });
    }

    #[cfg(feature = "schema")]
    #[allow(clippy::unused_self)]
    fn add_schema_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| router.route(uri::SCHEMA, get(handler_schema)));
    }

    #[cfg(not(feature = "schema"))]
    #[allow(clippy::unused_self)]
    fn add_schema_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| router.route(uri::SCHEMA, get(routing::schema_not_implemented)));
    }

    #[allow(clippy::unused_self)]
    fn add_openapi_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router
                .route("/openapi.json", get(handler_openapi))
                .route("/openapi", get(handler_openapi))
        });
    }

    #[allow(clippy::unused_self)]
    fn add_operator_auth_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| {
            router
                .route(
                    "/v1/operator/auth/registration/options",
                    post(operator_auth::handle_operator_register_options),
                )
                .route(
                    "/v1/operator/auth/registration/verify",
                    post(operator_auth::handle_operator_register_verify),
                )
                .route(
                    "/v1/operator/auth/login/options",
                    post(operator_auth::handle_operator_login_options),
                )
                .route(
                    "/v1/operator/auth/login/verify",
                    post(operator_auth::handle_operator_login_verify),
                )
        });
    }

    #[cfg(feature = "profiling")]
    #[allow(clippy::unused_self)]
    fn add_profiling_routes(&self, builder: &mut RouterBuilder) {
        builder.apply_with_state(|router, state| {
            let _ = state;
            // Profiling endpoints are feature-gated (`profiling-endpoint`) and used for local
            // debugging/perf runs. Keep them reachable without operator auth so localnet harnesses
            // can reliably collect profiles.
            let profiling = Router::new().route(uri::PROFILE, get(handler_profile));
            router.merge(profiling)
        });
    }

    #[cfg(not(feature = "profiling"))]
    #[allow(clippy::unused_self)]
    fn add_profiling_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| router.route(uri::PROFILE, get(routing::profiling_not_implemented)));
    }

    // ---------------- Additional route-group helpers (extracted) ----------------

    /// Governance VRF helpers (feature-gated)
    fn add_gov_vrf_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router.route(
                "/v1/gov/council/derive-vrf",
                post(handler_gov_council_derive_vrf),
            )
        });
    }

    /// Transactions (binary Norito) endpoint
    fn add_transaction_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| {
            let router = router
                .route(uri::TRANSACTION, post(handler_post_transaction))
                .layer(DefaultBodyLimit::max(
                    self.transaction_max_content_len
                        .get()
                        .try_into()
                        .expect("should't exceed usize"),
                ));

            router
                .route("/v1/iso20022/pacs008", post(handler_iso_pacs008))
                .route("/v1/iso20022/pacs009", post(handler_iso_pacs009))
                .route("/v1/iso20022/status/{msg_id}", get(handler_iso_status))
        });
    }

    /// Data-availability ingest endpoints.
    #[allow(clippy::unused_self)]
    fn add_da_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| {
            router
                .route("/v1/da/ingest", post(da::handler_post_da_ingest))
                .route(
                    "/v1/da/manifests/{ticket}",
                    get(da::handler_get_da_manifest),
                )
                .route(
                    "/v1/da/proof_policies",
                    get(da::commitments::handler_list_proof_policies),
                )
                .route(
                    "/v1/da/proof_policy_snapshot",
                    get(da::commitments::handler_proof_policy_bundle),
                )
                .route(
                    "/v1/da/commitments",
                    post(da::commitments::handler_list_commitments),
                )
                .route(
                    "/v1/da/commitments/prove",
                    post(da::commitments::handler_prove_commitment),
                )
                .route(
                    "/v1/da/commitments/verify",
                    post(da::commitments::handler_verify_commitment),
                )
                .route(
                    "/v1/da/pin_intents",
                    post(da::pin_intents::handler_list_pin_intents),
                )
                .route(
                    "/v1/da/pin_intents/prove",
                    post(da::pin_intents::handler_prove_pin_intent),
                )
                .route(
                    "/v1/da/pin_intents/verify",
                    post(da::pin_intents::handler_verify_pin_intent),
                )
        });
    }

    /// Contracts and VK registry routes
    #[allow(clippy::unused_self)]
    fn add_contracts_and_vk_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| {
            // Group contracts + VK endpoints into a small sub-router for clarity and merge it.
            let group = Router::new()
                .route("/v1/contracts/code", post(handler_post_contract_code))
                .route(
                    "/v1/contracts/code-bytes/{code_hash}",
                    get(handler_get_contract_code_bytes),
                )
                .route("/v1/contracts/deploy", post(handler_post_contract_deploy))
                .route(
                    "/v1/contracts/instance",
                    post(handler_post_contract_instance),
                )
                .route(
                    "/v1/contracts/instance/activate",
                    post(handler_post_contract_instance_activate),
                );

            #[cfg(feature = "app_api")]
            let group = group
                .route("/v1/contracts/call", post(handler_post_contract_call))
                .route("/v1/contracts/state", get(handler_get_contract_state));
            #[cfg(not(feature = "app_api"))]
            let group = group;

            let group = group
                .route(
                    "/v1/sorafs/pin/register",
                    post(handler_post_sorafs_register_manifest),
                )
                .route(
                    "/v1/sorafs/capacity/declare",
                    post(handler_post_sorafs_capacity_declare),
                )
                .route(
                    "/v1/sorafs/capacity/telemetry",
                    post(handler_post_sorafs_capacity_telemetry),
                )
                .route(
                    "/v1/sorafs/capacity/dispute",
                    post(handler_post_sorafs_capacity_dispute),
                )
                .route(
                    "/v1/sorafs/capacity/schedule",
                    post(handler_post_sorafs_capacity_schedule),
                )
                .route(
                    "/v1/sorafs/capacity/complete",
                    post(handler_post_sorafs_capacity_complete),
                )
                .route(
                    "/v1/sorafs/capacity/uptime",
                    post(handler_post_sorafs_capacity_uptime),
                )
                .route(
                    "/v1/sorafs/capacity/por-challenge",
                    post(handler_post_sorafs_capacity_por_challenge),
                )
                .route(
                    "/v1/sorafs/capacity/por-proof",
                    post(handler_post_sorafs_capacity_por_proof),
                )
                .route(
                    "/v1/sorafs/capacity/por-verdict",
                    post(handler_post_sorafs_capacity_por_verdict),
                )
                .route("/v1/sorafs/por/status", get(handler_get_sorafs_por_status))
                .route("/v1/sorafs/por/export", get(handler_get_sorafs_por_export))
                .route(
                    "/v1/sorafs/por/ingestion/{manifest_digest_hex}",
                    get(sorafs::api::handle_get_sorafs_por_ingestion),
                )
                .route(
                    "/v1/sorafs/por/report/{iso_week}",
                    get(handler_get_sorafs_por_report),
                )
                .route(
                    "/v1/sorafs/capacity/por",
                    post(handler_post_sorafs_capacity_por),
                )
                .route(
                    "/v1/sorafs/capacity/failure",
                    post(handler_post_sorafs_capacity_failure),
                );
            #[cfg(feature = "app_api")]
            let group = group
                .route(
                    "/v1/sorafs/audit/repair/report",
                    post(handler_post_sorafs_repair_report),
                )
                .route(
                    "/v1/sorafs/audit/repair/slash",
                    post(handler_post_sorafs_repair_slash),
                )
                .route(
                    "/v1/sorafs/audit/repair/claim",
                    post(handler_post_sorafs_repair_claim),
                )
                .route(
                    "/v1/sorafs/audit/repair/heartbeat",
                    post(handler_post_sorafs_repair_heartbeat),
                )
                .route(
                    "/v1/sorafs/audit/repair/complete",
                    post(handler_post_sorafs_repair_complete),
                )
                .route(
                    "/v1/sorafs/audit/repair/fail",
                    post(handler_post_sorafs_repair_fail),
                )
                .route(
                    "/v1/sorafs/audit/repair/status",
                    get(handler_get_sorafs_repair_status_all),
                )
                .route(
                    "/v1/sorafs/audit/repair/status/{manifest_hex}",
                    get(handler_get_sorafs_repair_status),
                );
            #[cfg(not(feature = "app_api"))]
            let group = group;

            let group = group
                // VK registry lifecycle (app API convenience): register, update
                .route("/v1/zk/vk/register", post(handler_post_vk_register))
                .route("/v1/zk/vk/update", post(handler_post_vk_update))
                .route(
                    "/v1/zk/vk/{backend}/{name}",
                    get(handler_get_vk_by_backend_name),
                )
                .route("/v1/zk/vk", get(handler_list_vk))
                .route("/v1/zk/proofs", get(handler_list_proofs))
                .route("/v1/zk/proofs/count", get(handler_count_proofs))
                .route(
                    "/v1/zk/proof/{backend}/{hash}",
                    get(handler_get_proof_by_backend_hash),
                )
                .route(
                    "/v1/confidential/derive-keyset",
                    post(handler_confidential_derive_keyset_route),
                )
                .route(
                    "/v1/contracts/code/{code_hash}",
                    get(handler_get_contract_code),
                );
            router.merge(group)
        });
    }

    /// P2P fallback, event SSE, subscription WS, and block stream WS
    #[allow(clippy::unused_self)]
    fn add_network_stream_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            let router = {
                #[cfg(feature = "p2p_ws")]
                let router = router.route("/p2p", get(handler_p2p_ws));
                #[cfg(not(feature = "p2p_ws"))]
                let router = router.route(
                    "/p2p",
                    get(|| async move { (axum::http::StatusCode::NOT_FOUND, "p2p_ws disabled") }),
                );
                router
            };

            let streams = {
                #[cfg(feature = "app_api")]
                {
                    Router::new()
                        .route("/v1/events/sse", get(handler_events_sse))
                        .route(uri::SUBSCRIPTION, get(handler_subscription_ws))
                        .route(uri::BLOCKS_STREAM, get(handler_blocks_stream_ws))
                }
                #[cfg(not(feature = "app_api"))]
                {
                    Router::new()
                }
            };

            router.merge(streams)
        });
    }

    /// Policy and pipeline recovery routes
    fn add_policy_and_pipeline_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router
                .route(
                    "/v1/pipeline/transactions/status",
                    get(handler_pipeline_transaction_status),
                )
                .route(
                    "/v1/pipeline/recovery/{height}",
                    get(handler_pipeline_recovery),
                )
                .route("/v1/policy", get(handler_policy))
        });
    }

    /// Signed Norito query endpoint
    fn add_query_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            let group = Router::new().route(uri::QUERY, post(handler_signed_query));
            router.merge(group)
        });
    }

    /// Proof record read route
    fn add_proof_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            router
                .route("/v1/proofs/{id}", get(handler_proof_record_get))
                .route(
                    iroha_torii_shared::uri::PROOF_RETENTION_STATUS,
                    get(handler_proof_retention_status),
                )
        });
    }

    /// Native MCP capability and JSON-RPC bridge routes.
    fn add_mcp_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| {
            if !self.mcp.enabled {
                return router;
            }

            let mcp_router = Router::new().route(
                "/v1/mcp",
                get(handler_mcp_capabilities).post(handler_mcp_jsonrpc),
            );
            router.merge(mcp_router)
        });
    }

    /// Iroha Connect routes (feature-gated)
    #[cfg(feature = "connect")]
    fn add_connect_routes(&self, builder: &mut RouterBuilder) {
        builder.apply(|router| {
            if !self.connect_enabled {
                return router;
            }
            let connect_router = Router::new()
                .route("/v1/connect/session", post(handler_connect_session))
                .route(
                    "/v1/connect/session/{sid}",
                    axum::routing::delete(handler_connect_session_delete),
                )
                .route("/v1/connect/ws", get(handler_connect_ws))
                .route("/v1/connect/status", get(handler_connect_status));
            router.merge(connect_router)
        });
    }

    #[cfg(not(feature = "connect"))]
    fn add_connect_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| router);
    }

    /// App-facing JSON endpoints (filters, webhooks, attachments, zk vote tally)
    /// App-facing JSON endpoints (filters, webhooks, attachments, zk vote tally)
    #[cfg(feature = "app_api")]
    fn add_app_api_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            // App-facing endpoints
            let aa_group = Router::new()
                .route(
                    "/v1/accounts/{account_id}/transactions/query",
                    post(handler_account_transactions_query),
                )
                .route(
                    "/v1/accounts/{account_id}/assets",
                    get(handler_account_assets),
                )
                .route(
                    "/v1/accounts/{account_id}/assets/query",
                    post(handler_account_assets_query),
                )
                .route(
                    "/v1/accounts/{account_id}/permissions",
                    get(handler_account_permissions),
                )
                .route(
                    "/v1/accounts/{account_id}/transactions",
                    get(handler_account_transactions_get),
                );
            let router = router
                .merge(aa_group)
                .route("/v1/proofs/query", post(handler_proofs_query))
                // Debug: fetch ZK1 tags for a proof id (feature-gated)
                .route(
                    "/v1/zk/proof-tags/{backend}/{hash}",
                    get(handler_proof_tags),
                )
                // App-facing: verify a batch of KZG proofs (BN254) carried in ZK1 envelopes
                // under `app_api` feature. Useful for integration checks.
                // Domains listing (GET) and JSON-DSL (POST)
                .route("/v1/domains", get(handler_domains_list))
                .route("/v1/domains/query", post(handler_domains_query))
                // Accounts listing
                .route("/v1/accounts", get(handler_accounts_list))
                .route("/v1/accounts/query", post(handler_accounts_query))
                .route("/v1/accounts/resolve", post(handler_accounts_resolve))
                .route("/v1/accounts/onboard", post(handler_accounts_onboard))
                .route(
                    "/v1/accounts/{uaid}/portfolio",
                    get(handler_accounts_portfolio),
                )
                .route(
                    "/v1/nexus/public_lanes/{lane_id}/validators",
                    get(handler_nexus_public_lane_validators),
                )
                .route(
                    "/v1/nexus/public_lanes/{lane_id}/stake",
                    get(handler_nexus_public_lane_stake),
                )
                .route(
                    "/v1/nexus/public_lanes/{lane_id}/rewards/pending",
                    get(handler_nexus_public_lane_rewards),
                )
                .route(
                    "/v1/nexus/dataspaces/accounts/{literal}/summary",
                    get(handler_nexus_dataspaces_account_summary),
                )
                .route(
                    "/v1/space-directory/uaids/{uaid}",
                    get(handler_space_directory_bindings),
                )
                .route(
                    "/v1/space-directory/uaids/{uaid}/manifests",
                    get(handler_space_directory_manifests),
                )
                .route(
                    "/v1/space-directory/manifests",
                    post(handler_space_directory_manifest_publish),
                )
                .route(
                    "/v1/space-directory/manifests/revoke",
                    post(handler_space_directory_manifest_revoke),
                )
                // Repo agreements listing
                .route("/v1/repo/agreements", get(handler_repo_agreements))
                .route(
                    "/v1/repo/agreements/query",
                    post(handler_repo_agreements_query),
                )
                .route(
                    "/v1/offline/allowances",
                    get(handler_offline_allowances_list).post(handler_offline_allowances_issue),
                )
                .route(
                    "/v1/offline/allowances/{certificate_id_hex}",
                    get(handler_offline_allowance_get),
                )
                .route(
                    "/v1/offline/allowances/{certificate_id_hex}/renew",
                    post(handler_offline_allowances_renew),
                )
                .route(
                    "/v1/offline/certificates",
                    get(handler_offline_allowances_list).post(handler_offline_allowances_issue),
                )
                .route(
                    "/v1/offline/certificates/issue",
                    post(handler_offline_certificates_issue),
                )
                .route(
                    "/v1/offline/certificates/{certificate_id_hex}",
                    get(handler_offline_allowance_get),
                )
                .route(
                    "/v1/offline/certificates/{certificate_id_hex}/renew",
                    post(handler_offline_allowances_renew),
                )
                .route(
                    "/v1/offline/certificates/{certificate_id_hex}/renew/issue",
                    post(handler_offline_certificates_renew_issue),
                )
                .route(
                    "/v1/offline/certificates/revoke",
                    post(handler_offline_certificates_revoke),
                )
                .route("/v1/offline/receipts", get(handler_offline_receipts_list))
                .route(
                    "/v1/offline/receipts/query",
                    post(handler_offline_receipts_query),
                )
                .route(
                    "/v1/offline/allowances/query",
                    post(handler_offline_allowances_query),
                )
                .route(
                    "/v1/offline/certificates/query",
                    post(handler_offline_certificates_query),
                )
                .route(
                    "/v1/offline/revocations",
                    get(handler_offline_revocations_list),
                )
                .route(
                    "/v1/offline/revocations/query",
                    post(handler_offline_revocations_query),
                )
                .route("/v1/offline/summaries", get(handler_offline_summaries_list))
                .route(
                    "/v1/offline/summaries/query",
                    post(handler_offline_summaries_query),
                )
                .route("/v1/offline/transfers", get(handler_offline_transfers_list))
                .route(
                    "/v1/offline/transfers/{bundle_id_hex}",
                    get(handler_offline_transfer_get),
                )
                .route(
                    "/v1/offline/transfers/query",
                    post(handler_offline_transfers_query),
                )
                .route(
                    "/v1/offline/settlements",
                    get(handler_offline_transfers_list).post(handler_offline_settlements_submit),
                )
                .route(
                    "/v1/offline/settlements/{bundle_id_hex}",
                    get(handler_offline_transfer_get),
                )
                .route(
                    "/v1/offline/settlements/query",
                    post(handler_offline_transfers_query),
                )
                .route(
                    "/v1/offline/spend-receipts",
                    post(handler_offline_spend_receipts_submit),
                )
                .route("/v1/offline/state", get(handler_offline_state))
                .route(
                    "/v1/offline/transfers/proof",
                    post(handler_offline_transfer_proof),
                )
                .route(
                    "/v1/offline/bundle/proof_status",
                    get(handler_offline_bundle_proof_status),
                );
            #[cfg(feature = "push")]
            let router = router.route("/v1/notify/devices", post(handler_push_register_device));
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            let router = router.route("/v1/offline/rejections", get(handler_offline_rejections));
            #[cfg(not(all(feature = "app_api", feature = "telemetry")))]
            let router = router;
            let router = router
                // SNS registrar scaffolding
                .route("/v1/sns/registrations", post(sns::handle_register))
                .route(
                    "/v1/sns/registrations/{selector}",
                    get(sns::handle_get_registration),
                )
                .route(
                    "/v1/sns/registrations/{selector}/renew",
                    post(sns::handle_renew_registration),
                )
                .route(
                    "/v1/sns/registrations/{selector}/transfer",
                    post(sns::handle_transfer_registration),
                )
                .route(
                    "/v1/sns/registrations/{selector}/controllers",
                    post(sns::handle_update_controllers),
                )
                .route(
                    "/v1/sns/registrations/{selector}/freeze",
                    post(sns::handle_freeze_registration),
                )
                .route(
                    "/v1/sns/registrations/{selector}/freeze",
                    delete(sns::handle_unfreeze_registration),
                )
                .route("/v1/sns/policies/{suffix_id}", get(sns::handle_get_policy))
                .route("/v1/sns/governance/cases", post(sns::handle_post_case))
                .route("/v1/sns/governance/cases", get(sns::handle_get_cases))
                .route("/v1/soracloud/deploy", post(soracloud::handle_deploy))
                .route("/v1/soracloud/upgrade", post(soracloud::handle_upgrade))
                .route("/v1/soracloud/rollback", post(soracloud::handle_rollback))
                .route("/v1/soracloud/rollout", post(soracloud::handle_rollout))
                .route(
                    "/v1/soracloud/state/mutate",
                    post(soracloud::handle_state_mutation),
                )
                .route(
                    "/v1/soracloud/fhe/job/run",
                    post(soracloud::handle_fhe_job_run),
                )
                .route(
                    "/v1/soracloud/decrypt/request",
                    post(soracloud::handle_decryption_request),
                )
                .route(
                    "/v1/soracloud/health/access/request",
                    post(soracloud::handle_health_access_request),
                )
                .route(
                    "/v1/soracloud/health/compliance/report",
                    get(soracloud::handle_health_compliance_report),
                )
                .route(
                    "/v1/soracloud/ciphertext/query",
                    post(soracloud::handle_ciphertext_query),
                )
                .route(
                    "/v1/soracloud/training/job/start",
                    post(soracloud::handle_training_job_start),
                )
                .route(
                    "/v1/soracloud/training/job/checkpoint",
                    post(soracloud::handle_training_job_checkpoint),
                )
                .route(
                    "/v1/soracloud/training/job/retry",
                    post(soracloud::handle_training_job_retry),
                )
                .route(
                    "/v1/soracloud/training/job/status",
                    get(soracloud::handle_training_job_status),
                )
                .route(
                    "/v1/soracloud/model/weight/register",
                    post(soracloud::handle_model_weight_register),
                )
                .route(
                    "/v1/soracloud/model/weight/promote",
                    post(soracloud::handle_model_weight_promote),
                )
                .route(
                    "/v1/soracloud/model/weight/rollback",
                    post(soracloud::handle_model_weight_rollback),
                )
                .route(
                    "/v1/soracloud/model/weight/status",
                    get(soracloud::handle_model_weight_status),
                )
                .route(
                    "/v1/soracloud/model/artifact/register",
                    post(soracloud::handle_model_artifact_register),
                )
                .route(
                    "/v1/soracloud/model/artifact/status",
                    get(soracloud::handle_model_artifact_status),
                )
                .route(
                    "/v1/soracloud/registry",
                    get(soracloud::handle_registry_status),
                )
                .route(
                    "/v1/soracloud/agent/deploy",
                    post(soracloud::handle_agent_deploy),
                )
                .route(
                    "/v1/soracloud/agent/lease/renew",
                    post(soracloud::handle_agent_lease_renew),
                )
                .route(
                    "/v1/soracloud/agent/restart",
                    post(soracloud::handle_agent_restart),
                )
                .route(
                    "/v1/soracloud/agent/status",
                    get(soracloud::handle_agent_status),
                )
                .route(
                    "/v1/soracloud/agent/wallet/spend",
                    post(soracloud::handle_agent_wallet_spend),
                )
                .route(
                    "/v1/soracloud/agent/wallet/approve",
                    post(soracloud::handle_agent_wallet_approve),
                )
                .route(
                    "/v1/soracloud/agent/policy/revoke",
                    post(soracloud::handle_agent_policy_revoke),
                )
                .route(
                    "/v1/soracloud/agent/message/send",
                    post(soracloud::handle_agent_message_send),
                )
                .route(
                    "/v1/soracloud/agent/message/ack",
                    post(soracloud::handle_agent_message_ack),
                )
                .route(
                    "/v1/soracloud/agent/mailbox/status",
                    get(soracloud::handle_agent_mailbox_status),
                )
                .route(
                    "/v1/soracloud/agent/autonomy/allow",
                    post(soracloud::handle_agent_autonomy_allow),
                )
                .route(
                    "/v1/soracloud/agent/autonomy/run",
                    post(soracloud::handle_agent_autonomy_run),
                )
                .route(
                    "/v1/soracloud/agent/autonomy/status",
                    get(soracloud::handle_agent_autonomy_status),
                )
                // Asset Definitions listing
                .route(
                    "/v1/assets/definitions",
                    get(handler_assets_definitions_list),
                )
                .route(
                    "/v1/assets/definitions/query",
                    post(handler_assets_definitions_query),
                )
                .route(
                    "/v1/confidential/assets/{definition_id}/transitions",
                    get(handler_confidential_asset_transitions),
                )
                // NFTs listing
                .route("/v1/nfts", get(handler_nfts_list))
                .route("/v1/nfts/query", post(handler_nfts_query))
                // Subscriptions
                .route(
                    "/v1/subscriptions/plans",
                    get(handler_subscription_plans_list).post(handler_subscription_plans_create),
                )
                .route(
                    "/v1/subscriptions",
                    get(handler_subscriptions_list).post(handler_subscriptions_create),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}",
                    get(handler_subscription_get),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}/pause",
                    post(handler_subscription_pause),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}/resume",
                    post(handler_subscription_resume),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}/cancel",
                    post(handler_subscription_cancel),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}/keep",
                    post(handler_subscription_keep),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}/usage",
                    post(handler_subscription_usage),
                )
                .route(
                    "/v1/subscriptions/{subscription_id}/charge-now",
                    post(handler_subscription_charge_now),
                )
                .route("/v1/parameters", get(handler_parameters))
                // Explorer endpoints
                .route("/v1/explorer/accounts", get(handler_explorer_accounts_list))
                .route("/v1/explorer/domains", get(handler_explorer_domains_list))
                .route(
                    "/v1/explorer/asset-definitions",
                    get(handler_explorer_asset_definitions_list),
                )
                .route("/v1/explorer/assets", get(handler_explorer_assets_list))
                .route("/v1/explorer/nfts", get(handler_explorer_nfts_list))
                .route("/v1/explorer/blocks", get(handler_explorer_blocks_list))
                .route(
                    "/v1/explorer/blocks/stream",
                    get(handler_explorer_blocks_stream),
                )
                .route(
                    "/v1/explorer/transactions",
                    get(handler_explorer_transactions_list),
                )
                .route(
                    "/v1/explorer/transactions/stream",
                    get(handler_explorer_transactions_stream),
                )
                .route(
                    "/v1/explorer/instructions",
                    get(handler_explorer_instructions_list),
                )
                .merge({
                    #[cfg(all(feature = "app_api", feature = "telemetry"))]
                    {
                        axum::Router::new().route(
                            "/v1/explorer/metrics",
                            axum::routing::get(handler_explorer_metrics),
                        )
                    }
                    #[cfg(not(all(feature = "app_api", feature = "telemetry")))]
                    {
                        axum::Router::new()
                    }
                })
                .route(
                    "/v1/explorer/instructions/stream",
                    get(handler_explorer_instructions_stream),
                );
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            let router = {
                router
                    .route(
                        "/v1/telemetry/peers-info",
                        get(handler_telemetry_peers_info),
                    )
                    .route("/v1/telemetry/live", get(handler_telemetry_live))
            };
            #[cfg(not(all(feature = "app_api", feature = "telemetry")))]
            let router = router;
            let router = router
                .route(
                    "/v1/explorer/accounts/{account_id}",
                    get(handler_explorer_account_detail),
                )
                .route(
                    "/v1/explorer/accounts/{account_id}/qr",
                    get(handler_explorer_account_qr),
                )
                .route(
                    "/v1/explorer/domains/{domain_id}",
                    get(handler_explorer_domain_detail),
                )
                .route(
                    "/v1/explorer/asset-definitions/{definition_id}",
                    get(handler_explorer_asset_definition_detail),
                )
                .route(
                    "/v1/explorer/asset-definitions/{definition_id}/econometrics",
                    get(handler_explorer_asset_definition_econometrics),
                )
                .route(
                    "/v1/explorer/asset-definitions/{definition_id}/snapshot",
                    get(handler_explorer_asset_definition_snapshot),
                )
                .route(
                    "/v1/explorer/assets/{asset_id}",
                    get(handler_explorer_asset_detail),
                )
                .route(
                    "/v1/explorer/nfts/{nft_id}",
                    get(handler_explorer_nft_detail),
                )
                .route(
                    "/v1/explorer/blocks/{identifier}",
                    get(handler_explorer_block_detail),
                )
                .route(
                    "/v1/explorer/transactions/{hash}",
                    get(handler_explorer_transaction_detail),
                )
                .route(
                    "/v1/explorer/instructions/{hash}/{index}",
                    get(handler_explorer_instruction_detail),
                );

            #[cfg(feature = "telemetry")]
            let router = router
                .route("/v1/kaigi/relays", get(handler_kaigi_relays))
                .route(
                    "/v1/kaigi/relays/{relay_id}",
                    get(handler_kaigi_relay_detail),
                )
                .route("/v1/kaigi/relays/health", get(handler_kaigi_relays_health))
                .route("/v1/kaigi/relays/events", get(handler_kaigi_relays_sse));

            #[cfg(not(feature = "telemetry"))]
            let router = router;

            router
                // Webhooks grouped sub-router
                .merge({
                    Router::new()
                        .route(
                            "/v1/webhooks",
                            post(handler_webhooks_create).get(handler_webhooks_list),
                        )
                        .route("/v1/webhooks/{id}", delete(handler_webhooks_delete))
                })
        });
    }

    #[cfg(feature = "app_api")]
    fn add_sorafs_routes(&self, builder: &mut RouterBuilder) {
        let discovery_enabled = self.sorafs_cache.is_some();
        let capacity_enabled = self.sorafs_node.is_enabled();

        if !discovery_enabled && !capacity_enabled {
            return;
        }

        if discovery_enabled {
            builder.apply(|router| {
                router
                    .route(
                        "/v1/sorafs/providers",
                        axum::routing::get(sorafs::api::handle_get_sorafs_providers),
                    )
                    .route(
                        "/v1/sorafs/providers/advert",
                        axum::routing::post(sorafs::api::handle_post_sorafs_provider_advert),
                    )
            });
        }

        if capacity_enabled {
            builder.apply(|router| {
                router
                    .route(
                        "/v1/sorafs/capacity/state",
                        axum::routing::get(sorafs::api::handle_get_sorafs_capacity_state),
                    )
                    .route(
                        "/v1/sorafs/pin",
                        axum::routing::get(sorafs::api::handle_get_sorafs_pin_registry),
                    )
                    .route(
                        "/v1/sorafs/pin/{digest_hex}",
                        axum::routing::get(sorafs::api::handle_get_sorafs_pin_manifest),
                    )
                    .route(
                        "/v1/sorafs/aliases",
                        axum::routing::get(sorafs::api::handle_get_sorafs_aliases),
                    )
                    .route(
                        "/v1/sorafs/replication",
                        axum::routing::get(sorafs::api::handle_get_sorafs_replication_orders),
                    )
                    .route(
                        "/v1/sorafs/storage/state",
                        axum::routing::get(sorafs::api::handle_get_sorafs_storage_state),
                    )
                    .route(
                        "/v1/sorafs/storage/manifest/{manifest_id}",
                        axum::routing::get(sorafs::api::handle_get_sorafs_storage_manifest),
                    )
                    .route(
                        "/v1/sorafs/storage/plan/{manifest_id}",
                        axum::routing::get(sorafs::api::handle_get_sorafs_storage_plan),
                    )
                    .route(
                        "/v1/sorafs/storage/pin",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_pin),
                    )
                    .route(
                        "/v1/sorafs/storage/fetch",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_fetch),
                    )
                    .route(
                        "/v1/sorafs/storage/token",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_token),
                    )
                    .route(
                        "/v1/sorafs/storage/car/{manifest_id}",
                        axum::routing::get(sorafs::api::handle_get_sorafs_storage_car_range),
                    )
                    .route(
                        "/v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}",
                        axum::routing::get(sorafs::api::handle_get_sorafs_storage_chunk),
                    )
                    .route(
                        "/v1/sorafs/storage/por-sample",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_por_sample),
                    )
                    .route(
                        "/v1/sorafs/proof/stream",
                        axum::routing::post(sorafs::api::handle_post_sorafs_proof_stream),
                    )
                    .route(
                        "/v1/sorafs/storage/por-challenge",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_por_challenge),
                    )
                    .route(
                        "/v1/sorafs/storage/por-proof",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_por_proof),
                    )
                    .route(
                        "/v1/sorafs/storage/por-verdict",
                        axum::routing::post(sorafs::api::handle_post_sorafs_storage_por_verdict),
                    )
                    .route(
                        "/v1/sorafs/deal/usage",
                        axum::routing::post(handler_post_sorafs_deal_usage),
                    )
                    .route(
                        "/v1/sorafs/deal/settle",
                        axum::routing::post(handler_post_sorafs_deal_settle),
                    )
            });
        }
    }

    #[cfg(feature = "app_api")]
    fn add_content_routes(builder: &mut RouterBuilder) {
        builder.apply(|router| {
            router.route(
                "/v1/content/{bundle}/{path..}",
                axum::routing::get(content::handle_get_content),
            )
        });
    }

    #[cfg(feature = "app_api")]
    fn add_soradns_routes(builder: &mut RouterBuilder) {
        builder.apply(|router| {
            router
                .route(
                    "/v1/soradns/directory/latest",
                    axum::routing::get(handler_soradns_directory_latest),
                )
                .route(
                    "/v1/soradns/directory/events",
                    axum::routing::get(handler_soradns_directory_events),
                )
        });
    }

    fn add_runtime_governance_routes(&self, builder: &mut RouterBuilder) {
        let _ = self;
        builder.apply(|router| {
            let attachments_methods = {
                let route = post(handler_zk_attachments_create);
                #[cfg(feature = "app_api")]
                let route = route.get(handler_zk_attachments_filtered);
                #[cfg(not(feature = "app_api"))]
                let route = route.get(handler_zk_attachments_list);
                route
            };

            let mut zk_router = Router::new()
                .route("/v1/zk/roots", post(handler_zk_roots))
                .route("/v1/zk/verify", post(handler_zk_verify))
                .route("/v1/zk/submit-proof", post(handler_zk_submit_proof))
                .route("/v1/zk/ivm/derive", post(handler_zk_ivm_derive))
                .route("/v1/zk/ivm/prove", post(handler_zk_ivm_prove))
                .route(
                    "/v1/zk/ivm/prove/{job_id}",
                    get(handler_zk_ivm_prove_get).delete(handler_zk_ivm_prove_delete),
                )
                .route("/v1/zk/attachments", attachments_methods)
                .route(
                    "/v1/zk/attachments/{id}",
                    get(handler_zk_attachment_get).delete(handler_zk_attachment_delete),
                )
                .route("/v1/zk/vote/tally", post(handler_zk_vote_tally));

            #[cfg(feature = "app_api")]
            {
                zk_router = zk_router.route(
                    "/v1/zk/attachments/count",
                    get(handler_zk_attachments_count),
                );
            }

            let mut router = router
                // Contracts: instances listing by namespace
                .route(
                    "/v1/contracts/instances/{ns}",
                    get(handler_contracts_instances_by_ns),
                )
                // Runtime ABI/upgrade endpoints
                .route(
                    iroha_torii_shared::uri::RUNTIME_ABI_ACTIVE,
                    get(handler_runtime_abi_active),
                )
                .route(
                    iroha_torii_shared::uri::RUNTIME_ABI_HASH,
                    get(handler_runtime_abi_hash),
                )
                .route(
                    iroha_torii_shared::uri::RUNTIME_METRICS,
                    get(handler_runtime_metrics),
                )
                .route(
                    iroha_torii_shared::uri::NODE_CAPABILITIES,
                    get(handler_node_capabilities),
                )
                // ZK and attachments grouped sub-router
                .merge(zk_router)
                .route(
                    iroha_torii_shared::uri::RUNTIME_UPGRADES_LIST,
                    get(handler_runtime_upgrades_list),
                )
                .route(
                    iroha_torii_shared::uri::RUNTIME_UPGRADES_PROPOSE,
                    post(handler_runtime_propose_upgrade),
                )
                .route(
                    iroha_torii_shared::uri::RUNTIME_UPGRADES_ACTIVATE,
                    post(handler_runtime_activate_upgrade),
                )
                .route(
                    iroha_torii_shared::uri::RUNTIME_UPGRADES_CANCEL,
                    post(handler_runtime_cancel_upgrade),
                );

            // Governance endpoints (convert to closures)
            router = router
                // Governance endpoints (AppState handlers + one closure)
                .route(
                    iroha_torii_shared::uri::GOV_PROPOSE_DEPLOY,
                    post(handler_gov_propose_deploy),
                )
                // Read endpoints: proposal/referendum/locks/tally
                .route(
                    iroha_torii_shared::uri::GOV_PROPOSAL_GET,
                    get(handler_gov_proposal_get),
                )
                .route(
                    iroha_torii_shared::uri::GOV_LOCKS_GET,
                    get(handler_gov_locks_get),
                )
                .route(
                    iroha_torii_shared::uri::GOV_REFERENDUM_GET,
                    get(handler_gov_referendum_get),
                )
                .route(
                    iroha_torii_shared::uri::GOV_TALLY_GET,
                    get(handler_gov_tally_get),
                )
                .route(
                    iroha_torii_shared::uri::GOV_BALLOT_ZK,
                    post(handler_gov_ballot_zk),
                )
                .route("/v1/gov/ballots/zk-v1", post(handler_gov_ballot_zk_v1))
                .route(
                    "/v1/gov/ballots/zk-v1/ballot-proof",
                    post(handler_gov_ballot_zk_v1_ballot_proof),
                )
                .route(
                    iroha_torii_shared::uri::GOV_BALLOT_PLAIN,
                    post(handler_gov_ballot_plain),
                )
                .route(
                    iroha_torii_shared::uri::GOV_FINALIZE,
                    post(handler_gov_finalize),
                )
                .route(
                    iroha_torii_shared::uri::GOV_PROTECTED_SET,
                    post(handler_gov_protected_set),
                )
                .route(
                    iroha_torii_shared::uri::GOV_PROTECTED_SET,
                    get(handler_gov_protected_get),
                )
                .route("/v1/gov/stream", get(handler_gov_stream))
                .route("/v1/gov/unlocks/stats", get(handler_gov_unlock_stats))
                .route(
                    iroha_torii_shared::uri::GOV_INSTANCES_BY_NS,
                    get(handler_gov_instances_ns),
                )
                .route(iroha_torii_shared::uri::GOV_ENACT, post(handler_gov_enact))
                .route(
                    iroha_torii_shared::uri::GOV_COUNCIL_CURRENT,
                    get(handler_gov_council_current),
                );
            // Persist council (app API; feature gov_vrf)
            {
                #[cfg(feature = "gov_vrf")]
                {
                    router = router.route(
                        iroha_torii_shared::uri::GOV_COUNCIL_PERSIST,
                        post(handler_gov_council_persist),
                    );
                    router = router.route(
                        iroha_torii_shared::uri::GOV_COUNCIL_REPLACE,
                        post(handler_gov_council_replace),
                    );
                }
                #[cfg(not(feature = "gov_vrf"))]
                {}
            }
            // Audit seed/epoch
            router = router.route(
                iroha_torii_shared::uri::GOV_COUNCIL_AUDIT,
                get(handler_gov_council_audit),
            );

            router
        });
    }
    /// Construct `Torii` using the classic telemetry arguments (`Telemetry` + enabled flag).
    #[allow(clippy::too_many_arguments)]
    #[cfg(feature = "telemetry")]
    /// Construct `Torii` with runtime telemetry configuration.
    pub fn new(
        chain_id: ChainId,
        kiso: KisoHandle,
        config: Config,
        queue: Arc<Queue>,
        events: EventsSender,
        query_service: LiveQueryStoreHandle,
        kura: Arc<Kura>,
        state: Arc<CoreState>,
        da_receipt_signer: KeyPair,
        online_peers: OnlinePeersProvider,
        telemetry: Telemetry,
        telemetry_enabled: bool,
    ) -> Self {
        let profile = if telemetry_enabled {
            TelemetryProfile::Operator
        } else {
            TelemetryProfile::Disabled
        };
        let handle = if telemetry_enabled {
            routing::MaybeTelemetry::from_profile(Some(telemetry), profile)
        } else {
            routing::MaybeTelemetry::from_profile(None, profile)
        };
        Self::new_with_handle(
            chain_id,
            kiso,
            config,
            queue,
            events,
            query_service,
            kura,
            state,
            da_receipt_signer,
            online_peers,
            None,
            handle,
        )
    }

    /// Construct `Torii` when the telemetry feature is disabled.
    #[allow(clippy::too_many_arguments)]
    #[cfg(not(feature = "telemetry"))]
    /// Construct `Torii` when telemetry support is disabled.
    pub fn new(
        chain_id: ChainId,
        kiso: KisoHandle,
        config: Config,
        queue: Arc<Queue>,
        events: EventsSender,
        query_service: LiveQueryStoreHandle,
        kura: Arc<Kura>,
        state: Arc<CoreState>,
        da_receipt_signer: KeyPair,
        online_peers: OnlinePeersProvider,
    ) -> Self {
        Self::new_with_handle(
            chain_id,
            kiso,
            config,
            queue,
            events,
            query_service,
            kura,
            state,
            da_receipt_signer,
            online_peers,
            None,
            routing::MaybeTelemetry::disabled(),
        )
    }

    /// Construct `Torii`.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_handle(
        chain_id: ChainId,
        kiso: KisoHandle,
        config: Config,
        queue: Arc<Queue>,
        events: EventsSender,
        query_service: LiveQueryStoreHandle,
        kura: Arc<Kura>,
        state: Arc<CoreState>,
        da_receipt_signer: KeyPair,
        online_peers: OnlinePeersProvider,
        sumeragi: Option<iroha_core::sumeragi::SumeragiHandle>,
        telemetry: routing::MaybeTelemetry,
    ) -> Self {
        routing::debug_match_flag::set_from_config(config.debug_match_filters);
        routing::set_app_query_limits(routing::AppQueryLimits::new(
            config.app_api.default_list_limit.get().into(),
            config.app_api.max_list_limit.get().into(),
            config.app_api.max_fetch_size.get().into(),
            config.app_api.rate_limit_cost_per_row.get().into(),
        ));
        install_account_resolvers(&state);
        crate::data_dir::set_base_dir(config.data_dir.clone());
        #[cfg(feature = "push")]
        let (push_bridge, push_rate_limiter) = {
            let bridge = if config.push.enabled {
                Some(push::PushBridge::new(config.push.clone()))
            } else {
                None
            };
            let per_sec = config.push.rate_per_minute.map(|value| {
                let per_min = value.get().max(1);
                per_min.saturating_add(59) / 60
            });
            let burst = config.push.burst.map(std::num::NonZeroU32::get);
            (bridge, limits::RateLimiter::new(per_sec, burst))
        };
        // Configure app API subsystems (attachments) from Torii config
        #[cfg(feature = "app_api")]
        {
            crate::webhook::set_webhook_policy(crate::webhook::WebhookPolicy {
                queue_capacity: config.webhook.queue_capacity,
                max_attempts: config.webhook.max_attempts,
                backoff_initial: config.webhook.backoff_initial,
                backoff_max: config.webhook.backoff_max,
                connect_timeout: config.webhook.connect_timeout,
                write_timeout: config.webhook.write_timeout,
                read_timeout: config.webhook.read_timeout,
            });
            crate::webhook::set_webhook_security_policy(crate::webhook::WebhookSecurityPolicy {
                enabled: config.webhook_security.enabled,
                allow_nets: limits::parse_cidrs(&config.webhook_security.allow_cidrs),
            });
            crate::zk_attachments::configure(
                config.attachments_ttl_secs,
                config.attachments_max_bytes,
                config.attachments_per_tenant_max_count,
                config.attachments_per_tenant_max_bytes,
                config.attachments_allowed_mime_types.clone(),
                config.attachments_max_expanded_bytes,
                config.attachments_max_archive_depth,
                config.attachments_sanitizer_mode,
                config.attachments_sanitize_timeout_ms,
                None,
                telemetry.clone(),
            );
            // Non-consensus background prover hook (disabled by default)
            crate::zk_prover::configure(
                config.zk_prover_enabled,
                config.zk_prover_scan_period_secs,
                config.zk_prover_reports_ttl_secs,
                config.zk_prover_max_inflight,
                config.zk_prover_max_scan_bytes,
                config.zk_prover_max_scan_millis,
                config.zk_prover_keys_dir.clone(),
                config.zk_prover_allowed_backends.clone(),
                config.zk_prover_allowed_circuits.clone(),
                Some(state.clone()),
                telemetry.clone(),
            );
            crate::zk_prover::start_worker();
        }
        if config.require_api_token && config.api_tokens.is_empty() {
            assert!(
                !(config.require_api_token && config.api_tokens.is_empty()),
                "torii.require_api_token is true but no api_tokens were configured"
            );
        }
        let api_versions = api_version::ApiVersionPolicy::from_labels(
            config.api_versions.clone(),
            config.api_version_default.clone(),
            config.api_min_proof_version.clone(),
            config.api_version_sunset_unix,
        )
        .unwrap_or_else(|err| panic!("invalid Torii API version config: {err}"));
        let rl = limits::RateLimiter::new(
            config
                .query_rate_per_authority_per_sec
                .map(std::num::NonZeroU32::get),
            config
                .query_burst_per_authority
                .map(std::num::NonZeroU32::get),
        );
        let tx_rl = limits::RateLimiter::new(
            config
                .tx_rate_per_authority_per_sec
                .map(std::num::NonZeroU32::get),
            config.tx_burst_per_authority.map(std::num::NonZeroU32::get),
        );
        let deploy_rl = limits::RateLimiter::new(
            config
                .deploy_rate_per_origin_per_sec
                .map(std::num::NonZeroU32::get),
            config
                .deploy_burst_per_origin
                .map(std::num::NonZeroU32::get),
        );
        let mcp_rate_per_sec = config.mcp.rate_per_minute.map(|rate| {
            let per_minute = rate.get();
            let per_sec = per_minute.div_ceil(60);
            per_sec.max(1)
        });
        let mcp_burst = config.mcp.burst.map(std::num::NonZeroU32::get);
        let mcp_rate_limiter = limits::RateLimiter::new(mcp_rate_per_sec, mcp_burst);
        let proof_rate_per_sec = config.proof_api.rate_per_minute.map(|rate| {
            let per_minute = rate.get();
            let per_sec = per_minute.div_ceil(60);
            per_sec.max(1)
        });
        let proof_burst = config.proof_api.burst.map(std::num::NonZeroU32::get);
        let proof_rate_limiter = limits::RateLimiter::new(proof_rate_per_sec, proof_burst);
        let proof_egress_limiter = limits::RateLimiter::new_u64(
            config
                .proof_api
                .egress_bytes_per_sec
                .map(std::num::NonZeroU64::get),
            config
                .proof_api
                .egress_burst_bytes
                .map(std::num::NonZeroU64::get),
        );
        let proof_limits = routing::ProofApiLimits::new(
            config.proof_api.max_list_limit.get(),
            config.proof_api.request_timeout,
            config.proof_api.cache_max_age,
            config.proof_api.retry_after,
            config.proof_api.max_body_bytes.get(),
        );
        let content_snapshot = state.content_snapshot();
        let content_limits_snapshot = content_snapshot.limits;
        let content_request_limiter = limits::RateLimiter::new(
            Some(content_limits_snapshot.max_requests_per_second.get()),
            Some(content_limits_snapshot.request_burst.get()),
        );
        let content_egress_limiter = limits::RateLimiter::new_u64(
            Some(content_limits_snapshot.max_egress_bytes_per_second.get()),
            Some(content_limits_snapshot.egress_burst_bytes.get()),
        );
        let allow_nets = limits::parse_cidrs(&config.api_allow_cidrs);
        let soranet_privacy_allow_nets =
            limits::parse_cidrs(&config.soranet_privacy_ingest.allow_cidrs);
        let soranet_privacy_tokens: HashSet<String> = config
            .soranet_privacy_ingest
            .tokens
            .iter()
            .cloned()
            .collect();
        let soranet_privacy_rate_limiter = limits::RateLimiter::new(
            config
                .soranet_privacy_ingest
                .rate_per_sec
                .map(std::num::NonZeroU32::get),
            config
                .soranet_privacy_ingest
                .burst
                .map(std::num::NonZeroU32::get),
        );
        let nofile_soft = limits::nofile_soft_limit();
        let configured_max_total = config
            .preauth_max_connections
            .map(std::num::NonZeroUsize::get);
        let max_total = limits::clamp_preauth_max_total(configured_max_total, nofile_soft);
        if let (Some(configured), Some(clamped)) = (configured_max_total, max_total) {
            if clamped < configured {
                iroha_logger::warn!(
                    configured,
                    clamped,
                    ?nofile_soft,
                    "clamping torii pre-auth global connection cap to fit RLIMIT_NOFILE"
                );
            }
        }
        let configured_max_per_ip = config
            .preauth_max_connections_per_ip
            .map(std::num::NonZeroUsize::get);
        let max_per_ip = limits::clamp_preauth_max_per_ip(configured_max_per_ip, max_total);
        if let (Some(configured), Some(clamped)) = (configured_max_per_ip, max_per_ip) {
            if clamped < configured {
                iroha_logger::warn!(
                    configured,
                    clamped,
                    "clamping torii pre-auth per-ip connection cap to global limit"
                );
            }
        }
        let preauth_gate = Arc::new(limits::PreAuthGate::new(limits::PreAuthConfig {
            max_total,
            max_per_ip,
            rate_per_ip: config
                .preauth_rate_per_ip_per_sec
                .map(std::num::NonZeroU32::get),
            burst_per_ip: config.preauth_burst_per_ip.map(std::num::NonZeroU32::get),
            ban_duration: config.preauth_temp_ban,
            allow_nets: limits::parse_cidrs(&config.preauth_allow_cidrs),
            scheme_limits: config
                .preauth_scheme_limits
                .iter()
                .map(|limit| limits::SchemeLimit {
                    name: limit.scheme.clone(),
                    max_connections: limit.max_connections.get(),
                })
                .collect(),
        }));
        let iso_bridge_runtime = Iso20022BridgeRuntime::from_config(&config.iso_bridge)
            .unwrap_or_else(|err| panic!("invalid ISO 20022 bridge configuration: {err:?}"))
            .map(Arc::new);
        let alias_service = alias_service_from_iso_config(&config.iso_bridge);
        let fee_policy = match (
            config.api_fee_asset_id.clone(),
            config.api_fee_amount,
            config.api_fee_receiver.clone(),
        ) {
            (Some(asset_id), Some(amount), Some(receiver)) => FeePolicy::Manual {
                asset_id,
                amount,
                receiver,
            },
            _ => FeePolicy::Disabled,
        };
        // Default threshold if not provided
        let default_high_load_tx_threshold =
            std::cmp::max(1, queue.current_backpressure().capacity().get() / 2);
        let high_load_tx_threshold = config
            .api_high_load_tx_threshold
            .unwrap_or(default_high_load_tx_threshold);
        // Default higher threshold for streaming endpoints if not provided
        let high_load_stream_tx_threshold = config.api_high_load_stream_threshold.unwrap_or(16_384);
        // Subscription WS may use its own threshold; default to stream threshold
        let high_load_subscription_tx_threshold = config
            .api_high_load_subscription_threshold
            .unwrap_or(high_load_stream_tx_threshold);

        let telemetry_profile = telemetry.profile();

        let api_tokens_set: Arc<HashSet<String>> =
            Arc::new(config.api_tokens.iter().cloned().collect());
        let operator_auth = Arc::new(
            operator_auth::OperatorAuth::new(
                config.operator_auth.clone(),
                api_tokens_set.clone(),
                config.data_dir.clone(),
                telemetry.clone(),
            )
            .unwrap_or_else(|err| panic!("invalid torii.operator_auth configuration: {err}")),
        );
        let operator_signatures = Arc::new(operator_signatures::OperatorSignatures::new(
            config.operator_signatures.clone(),
            da_receipt_signer.public_key().clone(),
            config.max_content_len.get(),
            telemetry.clone(),
        ));

        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        let peer_telemetry_urls = config
            .peer_telemetry_urls
            .iter()
            .cloned()
            .map(telemetry::peers::ToriiUrl::from)
            .collect::<Vec<_>>();
        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        let peer_geo = telemetry::peers::GeoLookupConfig::from(&config.peer_geo);

        #[cfg(all(feature = "telemetry", feature = "app_api"))]
        telemetry.with_metrics(|tel| {
            let metadata = sorafs_gateway_fixture_telemetry();
            if !metadata.fixtures_digest.is_empty() {
                tel.set_sorafs_gateway_fixture_metadata(
                    metadata.version.as_str(),
                    metadata.profile_version.as_str(),
                    metadata.fixtures_digest.as_str(),
                    metadata.released_at_unix,
                );
            }
        });

        #[cfg(feature = "app_api")]
        let sorafs_admission = load_sorafs_admission(&config);
        #[cfg(feature = "app_api")]
        let sorafs_cache = build_sorafs_cache(&config, sorafs_admission.clone());
        #[cfg(feature = "app_api")]
        let sorafs_node = sorafs_node::NodeHandle::new_with_policies(
            sorafs_node::config::StorageConfig::from(&config.sorafs_storage),
            sorafs_node::config::RepairConfig::from_repair_and_policy(
                &config.sorafs_repair,
                &state.gov.sorafs_repair_escalation,
            ),
            sorafs_node::config::GcConfig::from(&config.sorafs_gc),
        );
        #[cfg(feature = "app_api")]
        let sorafs_limits = Arc::new(sorafs::SorafsQuotaEnforcer::from_config(
            &build_sorafs_quota_config(&config.sorafs_quota),
        ));
        #[cfg(feature = "app_api")]
        let sorafs_alias_cache = sorafs::policy_from_config(&config.sorafs_alias_cache);
        #[cfg(feature = "app_api")]
        let sorafs_alias_enforcement = sorafs::enforcement_from_config(&config.sorafs_alias_cache);
        #[cfg(feature = "app_api")]
        let sorafs_gateway = config.sorafs_gateway.clone();
        #[cfg(feature = "app_api")]
        let sorafs_gateway_security = if sorafs_node.is_enabled() {
            Some(build_sorafs_gateway_security(
                &config.sorafs_gateway,
                sorafs_admission.clone(),
            ))
        } else {
            None
        };
        #[cfg(feature = "app_api")]
        let sorafs_pin_policy =
            sorafs::PinSubmissionPolicy::from_config(&config.sorafs_storage.pin)
                .unwrap_or_else(|err| panic!("invalid SoraFS storage pin policy: {err}"));
        #[cfg(feature = "app_api")]
        let stream_token_issuer =
            match sorafs::StreamTokenIssuer::from_config(&config.sorafs_storage.stream_tokens) {
                Ok(Some(issuer)) => Some(Arc::new(issuer)),
                Ok(None) => None,
                Err(err) => panic!("invalid SoraFS stream token configuration: {err}"),
            };
        #[cfg(feature = "app_api")]
        let (por_coordinator, por_runtime) = build_por_components(&config, &sorafs_node);
        #[cfg(feature = "app_api")]
        let repair_runtime = build_repair_runtime(&sorafs_node);
        #[cfg(feature = "app_api")]
        let gc_runtime = build_gc_runtime(&sorafs_node);
        #[cfg(feature = "app_api")]
        let uaid_onboarding = config
            .onboarding
            .as_ref()
            .map(|cfg| AccountOnboardingSigner {
                authority: cfg.authority.clone(),
                private_key: cfg.private_key.clone(),
                allowed_domain: cfg.allowed_domain.clone(),
            });
        #[cfg(feature = "app_api")]
        let offline_issuer = config.offline_issuer.as_ref().map(|cfg| {
            let operator_keypair = KeyPair::from_private_key(cfg.operator_private_key.0.clone())
                .unwrap_or_else(|err| {
                    panic!("invalid torii.offline_issuer.operator_private_key: {err}")
                });
            OfflineIssuerSigner {
                operator_keypair,
                allowed_controllers: cfg.allowed_controllers.clone(),
            }
        });
        let pipeline_status_cache = Arc::new(PipelineStatusCache::new());

        Self {
            chain_id: Arc::new(chain_id),
            kiso,
            queue,
            pipeline_status_cache,
            events,
            query_service,
            kura,
            state,
            online_peers,
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            peer_telemetry_urls,
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            peer_geo,
            sumeragi,
            telemetry,
            telemetry_profile,
            api_versions,
            content_config: content_snapshot,
            address: config.address,
            transaction_max_content_len: config.max_content_len,
            ws_message_timeout: config.ws_message_timeout,
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            p2p: None,
            query_rate_per_authority_per_sec: config.query_rate_per_authority_per_sec,
            query_burst_per_authority: config.query_burst_per_authority,
            tx_rate_per_authority_per_sec: config.tx_rate_per_authority_per_sec,
            tx_burst_per_authority: config.tx_burst_per_authority,
            deploy_rate_per_origin_per_sec: config.deploy_rate_per_origin_per_sec,
            deploy_burst_per_origin: config.deploy_burst_per_origin,
            rate_limiter: rl,
            tx_rate_limiter: tx_rl,
            deploy_rate_limiter: deploy_rl,
            proof_rate_limiter,
            proof_egress_limiter,
            content_request_limiter,
            content_egress_limiter,
            proof_limits,
            zk_prover_keys_dir: config.zk_prover_keys_dir.clone(),
            zk_ivm_prove_max_inflight: config.zk_ivm_prove_max_inflight,
            zk_ivm_prove_max_queue: config.zk_ivm_prove_max_queue,
            zk_ivm_prove_job_ttl_ms: config.zk_ivm_prove_job_ttl_secs.saturating_mul(1_000),
            zk_ivm_prove_job_max_entries: config.zk_ivm_prove_job_max_entries,
            preauth_gate,
            fee_policy,
            norito_rpc: config.transport.norito_rpc.clone(),
            mcp: config.mcp,
            mcp_rate_limiter,
            require_api_token: config.require_api_token,
            api_tokens_set: api_tokens_set.clone(),
            operator_auth,
            operator_signatures,
            soranet_privacy_ingest: config.soranet_privacy_ingest.clone(),
            soranet_privacy_tokens: std::sync::Arc::new(soranet_privacy_tokens),
            soranet_privacy_allow_nets: std::sync::Arc::new(soranet_privacy_allow_nets),
            soranet_privacy_rate_limiter,
            allow_nets: std::sync::Arc::new(allow_nets),
            high_load_tx_threshold,
            high_load_stream_tx_threshold,
            high_load_subscription_tx_threshold,
            iso_bridge: iso_bridge_runtime,
            alias_service,
            rbc_store_dir: None,
            rbc_sampling: config.rbc_sampling.clone(),
            da_receipt_signer,
            da_ingest: config.da_ingest.clone(),
            #[cfg(feature = "connect")]
            connect_bus: connect::Bus::from_config(&config.connect),
            #[cfg(feature = "connect")]
            connect_enabled: config.connect.enabled,
            #[cfg(feature = "push")]
            push: push_bridge,
            #[cfg(feature = "push")]
            push_rate_limiter,
            #[cfg(feature = "app_api")]
            sorafs_cache,
            #[cfg(feature = "app_api")]
            sorafs_node,
            #[cfg(feature = "app_api")]
            sorafs_limits,
            #[cfg(feature = "app_api")]
            por_coordinator,
            #[cfg(feature = "app_api")]
            por_runtime,
            #[cfg(feature = "app_api")]
            repair_runtime,
            #[cfg(feature = "app_api")]
            gc_runtime,
            #[cfg(feature = "app_api")]
            sorafs_alias_cache,
            #[cfg(feature = "app_api")]
            sorafs_alias_enforcement,
            #[cfg(feature = "app_api")]
            sorafs_gateway,
            #[cfg(feature = "app_api")]
            sorafs_gateway_security,
            #[cfg(feature = "app_api")]
            sorafs_pin_policy,
            #[cfg(feature = "app_api")]
            sorafs_admission,
            #[cfg(feature = "app_api")]
            stream_token_issuer,
            #[cfg(feature = "app_api")]
            offline_issuer,
            #[cfg(feature = "app_api")]
            uaid_onboarding,
        }
    }

    /// Wire a P2P handle for WebSocket fallback (feature `p2p_ws`).
    #[cfg(any(feature = "p2p_ws", feature = "connect"))]
    pub fn with_p2p(mut self, p2p: iroha_core::IrohaNetwork) -> Self {
        self.p2p = Some(p2p);
        #[cfg(feature = "connect")]
        {
            // Attach network to the Connect Bus for P2P relay and subscription.
            if let Some(net) = &self.p2p {
                self.connect_bus.attach_network(net.clone());
            }
        }
        self
    }
    /// No-op when both `p2p_ws` and `connect` features are disabled.
    #[cfg(not(any(feature = "p2p_ws", feature = "connect")))]
    pub fn with_p2p(self, _p2p: iroha_core::IrohaNetwork) -> Self {
        self
    }

    /// Attach the RBC persistence directory so sampling endpoints can read disk state.
    pub fn with_rbc_store_dir(mut self, dir: PathBuf) -> Self {
        self.rbc_store_dir = Some(dir);
        self
    }

    /// Helper function to create router. This router can be tested without starting up an HTTP server
    #[allow(clippy::too_many_lines)]
    fn create_api_router(&self) -> axum::Router {
        // Ensure the erased iterable-query registry is initialized before any
        // request decoding happens (the Norito extractor deserializes queries).
        #[allow(let_underscore_drop)]
        {
            use iroha_data_model as dm;
            use iroha_data_model::query as dm_query;
            dm_query::set_query_registry(dm::query_registry![
                dm_query::ErasedIterQuery<dm::domain::Domain>,
                dm_query::ErasedIterQuery<dm::account::Account>,
                dm_query::ErasedIterQuery<dm::asset::value::Asset>,
                dm_query::ErasedIterQuery<dm::asset::definition::AssetDefinition>,
                dm_query::ErasedIterQuery<dm::nft::Nft>,
                dm_query::ErasedIterQuery<dm::role::Role>,
                dm_query::ErasedIterQuery<dm::role::RoleId>,
                dm_query::ErasedIterQuery<dm::peer::PeerId>,
                dm_query::ErasedIterQuery<dm::trigger::TriggerId>,
                dm_query::ErasedIterQuery<dm::trigger::Trigger>,
                dm_query::ErasedIterQuery<dm_query::CommittedTransaction>,
                dm_query::ErasedIterQuery<dm::block::SignedBlock>,
                dm_query::ErasedIterQuery<dm::block::BlockHeader>,
            ]);
        }

        let rbc_chain_hash = Hash::new(self.chain_id.as_str().as_bytes());
        let sampling_cfg = &self.rbc_sampling;
        let sampling_enabled = sampling_cfg.enabled && self.rbc_store_dir.is_some();
        let sampling_rate_per_sec = sampling_cfg.rate_per_minute.map(|rate| {
            let per_minute = rate.get();
            let per_sec = per_minute.div_ceil(60);
            per_sec.max(1)
        });
        let sampling_burst = sampling_cfg.rate_per_minute.map(|rate| rate.get().max(1));
        let rbc_sampling_limiter = limits::RateLimiter::new(sampling_rate_per_sec, sampling_burst);
        let rbc_sampling_budget = DashMap::new();
        let rbc_sampling_manifest = SoftwareManifest::current();

        #[cfg(feature = "app_api")]
        let gateway_components = self.sorafs_gateway_security.clone();

        let replay_store_dir = self.da_ingest.replay_cache_store_dir.clone();
        let replay_cursor_store = match da::ReplayCursorStore::open(replay_store_dir.clone()) {
            Ok(store) => store,
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    dir = ?replay_store_dir,
                    "failed to load DA replay cursor snapshot; starting with empty cache"
                );
                match da::ReplayCursorStore::empty(replay_store_dir.clone()) {
                    Ok(store) => store,
                    Err(io_err) => {
                        iroha_logger::error!(
                            ?io_err,
                            dir = ?replay_store_dir,
                            "failed to prepare DA replay cursor directory; DA replay persistence disabled"
                        );
                        da::ReplayCursorStore::in_memory()
                    }
                }
            }
        };
        let replay_cache_config = iroha_core::da::ReplayCacheConfig::new()
            .with_max_entries_per_lane(self.da_ingest.replay_cache_capacity)
            .with_ttl(self.da_ingest.replay_cache_ttl)
            .with_max_sequence_lag(self.da_ingest.replay_cache_max_sequence_lag);
        let da_replay_cache = Arc::new(iroha_core::da::ReplayCache::new(replay_cache_config));
        for (lane_epoch, highest) in replay_cursor_store.highest_sequences() {
            da_replay_cache.prime_lane_epoch(lane_epoch, highest);
        }
        let da_replay_store = Arc::new(replay_cursor_store);
        let da_receipt_log = match da::DaReceiptLog::open(
            self.da_ingest.manifest_store_dir.clone(),
            Arc::clone(&da_replay_store),
            self.da_receipt_signer.public_key().clone(),
        ) {
            Ok(log) => Arc::new(log),
            Err(err) => {
                iroha_logger::error!(
                    ?err,
                    dir = ?self.da_ingest.manifest_store_dir,
                    "failed to open DA receipt log; falling back to in-memory log"
                );
                Arc::new(da::DaReceiptLog::in_memory(
                    Arc::clone(&da_replay_store),
                    self.da_receipt_signer.public_key().clone(),
                ))
            }
        };
        #[cfg(feature = "telemetry")]
        self.telemetry.with_metrics(|metrics| {
            for (lane_epoch, highest) in da_replay_store.highest_sequences() {
                metrics.set_da_receipt_cursor(
                    lane_epoch.lane_id.as_u32(),
                    lane_epoch.epoch,
                    highest,
                );
            }
        });

        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        let peer_telemetry = telemetry::peers::PeerTelemetryService::new(
            collect_peer_urls(&self.online_peers, &self.peer_telemetry_urls),
            self.peer_geo.clone(),
        );

        let zk_ivm_prove_jobs = Arc::new(DashMap::new());
        let zk_ivm_prove_max_inflight = self.zk_ivm_prove_max_inflight.max(1);
        let zk_ivm_prove_slots_total =
            zk_ivm_prove_max_inflight.saturating_add(self.zk_ivm_prove_max_queue);
        let zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(zk_ivm_prove_slots_total));
        let zk_ivm_prove_inflight =
            Arc::new(tokio::sync::Semaphore::new(zk_ivm_prove_max_inflight));
        let zk_ivm_prove_inflight_total = zk_ivm_prove_max_inflight;
        let mcp_tools = Arc::new(if self.mcp.enabled {
            mcp::build_tool_specs(&self.mcp)
        } else {
            Vec::new()
        });

        let app_state: SharedAppState = Arc::new(AppState {
            events: self.events.clone(),
            kura: self.kura.clone(),
            chain_id: self.chain_id.clone(),
            state: self.state.clone(),
            kiso: self.kiso.clone(),
            query_service: self.query_service.clone(),
            rate_limiter: self.rate_limiter.clone(),
            tx_rate_limiter: self.tx_rate_limiter.clone(),
            deploy_rate_limiter: self.deploy_rate_limiter.clone(),
            proof_rate_limiter: self.proof_rate_limiter.clone(),
            proof_egress_limiter: self.proof_egress_limiter.clone(),
            content_request_limiter: self.content_request_limiter.clone(),
            content_egress_limiter: self.content_egress_limiter.clone(),
            proof_limits: self.proof_limits,
            content_config: self.content_config.clone(),
            ws_message_timeout: self.ws_message_timeout,
            require_api_token: self.require_api_token,
            api_tokens_set: self.api_tokens_set.clone(),
            operator_auth: self.operator_auth.clone(),
            operator_signatures: self.operator_signatures.clone(),
            soranet_privacy_ingest: self.soranet_privacy_ingest.clone(),
            soranet_privacy_tokens: self.soranet_privacy_tokens.clone(),
            soranet_privacy_allow_nets: self.soranet_privacy_allow_nets.clone(),
            soranet_privacy_rate_limiter: self.soranet_privacy_rate_limiter.clone(),
            allow_nets: self.allow_nets.clone(),
            preauth_gate: self.preauth_gate.clone(),
            queue: self.queue.clone(),
            pipeline_status_cache: self.pipeline_status_cache.clone(),
            mcp: self.mcp,
            mcp_rate_limiter: self.mcp_rate_limiter.clone(),
            mcp_tools: mcp_tools.clone(),
            mcp_dispatch_router: std::sync::RwLock::new(None),
            fee_policy: self.fee_policy.clone(),
            norito_rpc: self.norito_rpc.clone(),
            high_load_tx_threshold: self.high_load_tx_threshold,
            high_load_stream_tx_threshold: self.high_load_stream_tx_threshold,
            high_load_subscription_tx_threshold: self.high_load_subscription_tx_threshold,
            online_peers: self.online_peers.clone(),
            iso_bridge: self.iso_bridge.clone(),
            alias_service: self.alias_service.clone(),
            telemetry: self.telemetry.clone(),
            telemetry_profile: self.telemetry_profile,
            api_versions: self.api_versions.clone(),
            zk_prover_keys_dir: self.zk_prover_keys_dir.clone(),
            zk_ivm_prove_jobs,
            zk_ivm_prove_inflight,
            zk_ivm_prove_slots,
            zk_ivm_prove_slots_total,
            zk_ivm_prove_inflight_total,
            zk_ivm_prove_job_ttl_ms: self.zk_ivm_prove_job_ttl_ms,
            zk_ivm_prove_job_max_entries: self.zk_ivm_prove_job_max_entries,
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            peer_telemetry,
            rbc_sampling_enabled: sampling_enabled,
            rbc_sampling_store_dir: self.rbc_store_dir.clone(),
            rbc_sampling_max_samples: sampling_cfg.max_samples_per_request,
            rbc_sampling_max_bytes: sampling_cfg.max_bytes_per_request,
            rbc_sampling_daily_budget: sampling_cfg.daily_byte_budget,
            rbc_sampling_limiter,
            rbc_sampling_budget,
            rbc_sampling_manifest,
            rbc_chain_hash,
            da_replay_cache,
            da_replay_store,
            da_receipt_log,
            da_receipt_signer: self.da_receipt_signer.clone(),
            da_ingest: self.da_ingest.clone(),
            sumeragi: self.sumeragi.clone(),
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            p2p: self.p2p.clone(),
            #[cfg(feature = "connect")]
            connect_bus: self.connect_bus.clone(),
            #[cfg(feature = "connect")]
            connect_enabled: self.connect_enabled,
            #[cfg(feature = "push")]
            push: self.push.clone(),
            #[cfg(feature = "push")]
            push_rate_limiter: self.push_rate_limiter.clone(),
            #[cfg(feature = "app_api")]
            sorafs_cache: self.sorafs_cache.clone(),
            #[cfg(feature = "app_api")]
            sorafs_node: self.sorafs_node.clone(),
            #[cfg(feature = "app_api")]
            sorafs_limits: self.sorafs_limits.clone(),
            #[cfg(feature = "app_api")]
            por_coordinator: self.por_coordinator.clone(),
            #[cfg(feature = "app_api")]
            sorafs_alias_cache_policy: self.sorafs_alias_cache.clone(),
            #[cfg(feature = "app_api")]
            sorafs_alias_enforcement: self.sorafs_alias_enforcement,
            #[cfg(feature = "app_api")]
            sorafs_admission: self.sorafs_admission.clone(),
            #[cfg(feature = "app_api")]
            sorafs_gateway_config: self.sorafs_gateway.clone(),
            #[cfg(feature = "app_api")]
            sorafs_gateway_policy: gateway_components
                .as_ref()
                .map(|components| Arc::clone(&components.policy)),
            #[cfg(feature = "app_api")]
            sorafs_gateway_denylist: gateway_components
                .as_ref()
                .map(|components| Arc::clone(&components.denylist)),
            #[cfg(feature = "app_api")]
            sorafs_gateway_tls_state: gateway_components
                .as_ref()
                .map(|components| Arc::clone(&components.tls_state)),
            #[cfg(feature = "app_api")]
            sorafs_pin_policy: self.sorafs_pin_policy.clone(),
            #[cfg(feature = "app_api")]
            sorafs_blinded_resolver: gateway_components
                .as_ref()
                .and_then(|components| components.blinded_resolver.clone()),
            #[cfg(feature = "app_api")]
            stream_token_issuer: self.stream_token_issuer.clone(),
            #[cfg(feature = "app_api")]
            stream_token_concurrency: sorafs::StreamTokenConcurrencyTracker::default(),
            #[cfg(feature = "app_api")]
            stream_token_quota: sorafs::StreamTokenQuotaTracker::default(),
            #[cfg(feature = "app_api")]
            sorafs_chunk_range_overrides: DashMap::new(),
            #[cfg(feature = "app_api")]
            offline_issuer: self.offline_issuer.clone(),
            #[cfg(feature = "app_api")]
            uaid_onboarding: self.uaid_onboarding.clone(),
            #[cfg(feature = "app_api")]
            sns_registry: Arc::new(sns::Registry::bootstrap_default()),
            #[cfg(feature = "app_api")]
            soracloud_registry: Arc::new({
                #[cfg(test)]
                {
                    soracloud::Registry::default()
                }
                #[cfg(not(test))]
                {
                    soracloud::Registry::with_default_persistence()
                }
            }),
        });

        #[cfg(feature = "app_api")]
        if gateway_components.is_some() {
            let version = sorafs_manifest::gateway_fixture::SORAFS_GATEWAY_FIXTURE_VERSION;
            app_state.telemetry.with_metrics(|metrics| {
                metrics.set_sorafs_gateway_fixture_version(version);
            });
        }

        // Touch certain fields to avoid dead-code warnings under feature combinations
        let _ = (&app_state.kiso, &app_state.online_peers);
        #[cfg(any(feature = "p2p_ws", feature = "connect"))]
        let _ = &app_state.p2p;
        let _ = (
            &app_state.mcp,
            &app_state.mcp_rate_limiter,
            &app_state.mcp_tools,
            &app_state.mcp_dispatch_router,
        );
        #[cfg(feature = "app_api")]
        let _ = (
            &app_state.sorafs_node,
            &app_state.sorafs_limits,
            &app_state.sorafs_alias_cache_policy,
            &app_state.sorafs_alias_enforcement,
            &app_state.sorafs_admission,
            &app_state.sorafs_gateway_config,
            &app_state.sorafs_gateway_policy,
            &app_state.sorafs_gateway_denylist,
            &app_state.sorafs_gateway_tls_state,
            &app_state.sorafs_blinded_resolver,
            &app_state.stream_token_issuer,
            &app_state.stream_token_quota,
            &app_state.stream_token_concurrency,
            &app_state.sorafs_chunk_range_overrides,
            &app_state.offline_issuer,
            &app_state.uaid_onboarding,
            &app_state.sns_registry,
            &app_state.soracloud_registry,
        );

        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        if let Some(tls_state) = &app_state.sorafs_gateway_tls_state {
            if let Ok(snapshot) = tls_state.try_read() {
                let now = SystemTime::now();
                app_state
                    .telemetry
                    .with_metrics(|metrics| snapshot.apply_metrics(metrics, now));
            }
        }

        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        {
            let metadata = sorafs_gateway_fixture_telemetry();
            app_state.telemetry.with_metrics(|telemetry| {
                telemetry.set_sorafs_gateway_fixture_metadata(
                    metadata.version.as_str(),
                    metadata.profile_version.as_str(),
                    metadata.fixtures_digest.as_str(),
                    metadata.released_at_unix,
                )
            });
        }

        let base_router: Router<SharedAppState> = Router::new().without_v07_checks();
        let mut builder = RouterBuilder::from_router(base_router, app_state.clone());

        self.add_sumeragi_routes(&mut builder);
        // VRF governance helper route(s)
        self.add_gov_vrf_routes(&mut builder);
        // Core info and introspection
        self.add_telemetry_routes(&mut builder);
        self.add_core_info_routes(&mut builder);
        self.add_operator_auth_routes(&mut builder);
        self.add_alias_routes(&mut builder);
        self.add_time_routes(&mut builder);
        self.add_schema_routes(&mut builder);
        self.add_openapi_routes(&mut builder);
        self.add_profiling_routes(&mut builder);
        // Runtime/Governance routes that require state
        self.add_runtime_governance_routes(&mut builder);
        // Transaction, Contracts, VK
        self.add_transaction_routes(&mut builder);
        self.add_da_routes(&mut builder);
        self.add_contracts_and_vk_routes(&mut builder);
        // Signed Norito query and proof endpoints
        self.add_query_routes(&mut builder);
        self.add_proof_routes(&mut builder);
        self.add_mcp_routes(&mut builder);
        // Streams and P2P websocket fallback
        self.add_network_stream_routes(&mut builder);
        // Policy and pipeline helpers
        self.add_policy_and_pipeline_routes(&mut builder);
        #[cfg(feature = "app_api")]
        self.add_sorafs_routes(&mut builder);
        #[cfg(feature = "app_api")]
        Self::add_content_routes(&mut builder);
        #[cfg(feature = "app_api")]
        Self::add_soradns_routes(&mut builder);
        // Iroha Connect (feature-gated)
        #[cfg(feature = "connect")]
        self.add_connect_routes(&mut builder);
        // App-facing JSON API
        #[cfg(feature = "app_api")]
        self.add_app_api_routes(&mut builder);

        let router = builder
            .finish()
            .layer((
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::default().include_headers(true)),
                TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    SERVER_SHUTDOWN_TIMEOUT,
                ),
            ))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                enforce_preauth,
            ))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                enforce_api_token,
            ))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                enforce_api_version,
            ))
            .layer(axum::middleware::from_fn(inject_remote_addr_header))
            .layer(axum::middleware::from_fn_with_state(
                app_state.clone(),
                record_http_metrics,
            ));

        {
            let mut guard = app_state
                .mcp_dispatch_router
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *guard = Some(router.clone());
        }

        router.with_state(app_state)
    }

    /// Public helper to get the API router for integration tests without starting an HTTP server.
    ///
    /// This keeps test code decoupled from server start and avoids binding to a TCP port.
    pub fn api_router_for_tests(&self) -> axum::Router {
        self.create_api_router()
    }

    /// Expose the push bridge to tests so registration side effects can be inspected.
    #[cfg(feature = "push")]
    pub fn push_bridge_for_tests(&self) -> Option<push::PushBridge> {
        self.push.clone()
    }

    /// To handle incoming requests `Torii` should be started first.
    ///
    /// # Errors
    /// Can fail due to listening to network or if http server fails
    // #[iroha_futures::telemetry_future]
    #[cfg(not(test))]
    pub async fn start(
        self,
        shutdown_signal: ShutdownSignal,
    ) -> core::result::Result<(), Report<Error>> {
        let torii_address = self.address.value().clone();
        iroha_logger::info!(addr = %torii_address, "starting Torii HTTP server");

        // Initialize optional app-facing subsystems.
        #[cfg(feature = "app_api")]
        {
            // Best-effort load/persist registry and spawn delivery worker.
            // If the data dir isn't writable, errors are logged and the in-memory
            // registry still functions.
            crate::webhook::init_persistence();
            crate::webhook::start_delivery_worker();
            // Initialize attachments store and background GC
            crate::zk_attachments::init_persistence();
            crate::zk_attachments::start_gc_worker();
            // Spawn a single event enqueuer that subscribes to core events and
            // pushes JSON payloads into the webhook delivery queue. This is separate
            // from SSE/WS consumers to avoid duplicate deliveries per connection.
            let mut rx = self.events.subscribe();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            crate::webhook::enqueue_event_for_matching_webhooks(
                                &event,
                                "application/json",
                            );
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Skip ahead on lag; webhook delivery is best-effort
                        }
                    }
                }
            });
        }

        if let Some(runtime) = self.iso_bridge.clone() {
            let mut rx = self.events.subscribe();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(EventBox::Pipeline(PipelineEventBox::Transaction(event))) => {
                            let tx_hash = event.hash().to_string();
                            match *event.status() {
                                TransactionStatus::Approved => {
                                    runtime.mark_transaction_applied(&tx_hash, SystemTime::now());
                                }
                                TransactionStatus::Rejected(ref reason) => {
                                    runtime.mark_transaction_rejected(&tx_hash, Some(reason));
                                }
                                TransactionStatus::Expired => {
                                    runtime.mark_transaction_expired(&tx_hash);
                                }
                                TransactionStatus::Queued => {}
                            }
                        }
                        Ok(EventBox::PipelineBatch(events)) => {
                            for event in events {
                                if let PipelineEventBox::Transaction(event) = event {
                                    let tx_hash = event.hash().to_string();
                                    match *event.status() {
                                        TransactionStatus::Approved => {
                                            runtime.mark_transaction_applied(
                                                &tx_hash,
                                                SystemTime::now(),
                                            );
                                        }
                                        TransactionStatus::Rejected(ref reason) => {
                                            runtime
                                                .mark_transaction_rejected(&tx_hash, Some(reason));
                                        }
                                        TransactionStatus::Expired => {
                                            runtime.mark_transaction_expired(&tx_hash);
                                        }
                                        TransactionStatus::Queued => {}
                                    }
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Skip on lag to catch up with the latest events
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        {
            let mut rx = self.events.subscribe();
            let cache = self.pipeline_status_cache.clone();
            let kura = self.kura.clone();
            tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(EventBox::Pipeline(PipelineEventBox::Transaction(event))) => {
                            cache.record_transaction_event(&event);
                        }
                        Ok(EventBox::Pipeline(PipelineEventBox::Block(event))) => {
                            cache.record_block_event(&event, &kura);
                        }
                        Ok(EventBox::PipelineBatch(events)) => {
                            for event in events {
                                match event {
                                    PipelineEventBox::Transaction(event) => {
                                        cache.record_transaction_event(&event);
                                    }
                                    PipelineEventBox::Block(event) => {
                                        cache.record_block_event(&event, &kura);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // Skip on lag to catch up with the latest events
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        if let Some(anchor_cfg) = self.da_ingest.taikai_anchor.clone() {
            crate::da::spawn_anchor_worker(
                self.da_ingest.manifest_store_dir.clone(),
                anchor_cfg,
                shutdown_signal.clone(),
            );
        }

        #[cfg(feature = "app_api")]
        self.spawn_pin_registry_metrics_worker(shutdown_signal.clone());

        #[cfg(feature = "app_api")]
        self.spawn_por_ingestion_metrics_worker(shutdown_signal.clone());

        #[cfg(feature = "app_api")]
        if let Some(runtime) = &self.por_runtime {
            runtime.clone().spawn(shutdown_signal.clone());
        }

        #[cfg(feature = "app_api")]
        if let Some(runtime) = &self.repair_runtime {
            runtime.clone().spawn(shutdown_signal.clone());
        }

        #[cfg(feature = "app_api")]
        if let Some(runtime) = &self.gc_runtime {
            runtime.clone().spawn(shutdown_signal.clone());
        }

        #[cfg(feature = "app_api")]
        if let Some(components) = &self.sorafs_gateway_security {
            if let Some(automation) = &components.tls_automation {
                automation.spawn(self.telemetry.clone(), shutdown_signal.clone());
            }
        }

        let listener = match torii_address.clone() {
            SocketAddr::Ipv4(v) => TcpListener::bind(std::net::SocketAddr::V4(v.into())).await,
            SocketAddr::Ipv6(v) => TcpListener::bind(std::net::SocketAddr::V6(v.into())).await,
            SocketAddr::Host(v) => TcpListener::bind((v.host.as_ref(), v.port)).await,
        }
        .change_context(Error::StartServer)
        .attach("failed to bind to the specified address")
        .attach_with(|| self.address.clone().into_attachment())?;

        let api_router = self.create_api_router();
        let make = api_router.into_make_service_with_connect_info::<std::net::SocketAddr>();

        iroha_logger::info!(addr = %torii_address, "Torii bound and listening");

        axum::serve(listener, make)
            .with_graceful_shutdown(async move { shutdown_signal.receive().await })
            .await
            .map_err(Report::from)
            .change_context(Error::FailedExit)
    }
}

/// GET /openapi(.json) — expose OpenAPI descriptor subject to Torii access policy.
async fn handler_openapi(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, Error> {
    let remote_ip = remote.ip();
    if !limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.allow_nets) {
        check_access(&app, &headers, Some(remote_ip), "openapi").await?;
    }

    Ok(routing::handler_openapi_spec(axum::extract::State(app)).await)
}

/// GET /v1/mcp — expose MCP capabilities and tool-count metadata.
async fn handler_mcp_capabilities(
    State(app): State<SharedAppState>,
) -> (StatusCode, JsonBody<norito::json::Value>) {
    (
        StatusCode::OK,
        JsonBody(mcp::capabilities_payload(app.mcp_tools.len())),
    )
}

/// POST /v1/mcp — execute native MCP JSON-RPC requests.
async fn handler_mcp_jsonrpc(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    request: axum::http::Request<Body>,
) -> (StatusCode, JsonBody<norito::json::Value>) {
    let rate_key = limits::key_from_headers(&headers, None, Some("mcp"), app.require_api_token);
    if !app.mcp_rate_limiter.allow(&rate_key).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            JsonBody(mcp::jsonrpc_rate_limited()),
        );
    }

    let request_bytes =
        match axum::body::to_bytes(request.into_body(), app.mcp.max_request_bytes).await {
            Ok(bytes) => bytes,
            Err(_) => {
                let (status, payload) = mcp::oversized_payload_response(app.mcp.max_request_bytes);
                return (status, JsonBody(payload));
            }
        };

    let payload = match norito::json::from_slice::<norito::json::Value>(&request_bytes) {
        Ok(payload) => payload,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                JsonBody(mcp::invalid_json_payload(&err)),
            );
        }
    };

    let response_payload = if let Some(batch) = payload.as_array() {
        if batch.is_empty() {
            mcp::jsonrpc_invalid_request("batch request must not be empty")
        } else {
            let mut responses = Vec::with_capacity(batch.len());
            for request_value in batch {
                let response =
                    mcp::handle_jsonrpc_request(app.clone(), &headers, request_value.clone()).await;
                responses.push(response);
            }
            norito::json::Value::Array(responses)
        }
    } else {
        mcp::handle_jsonrpc_request(app, &headers, payload).await
    };

    (StatusCode::OK, JsonBody(response_payload))
}

#[cfg(feature = "app_api")]
fn build_sorafs_cache(
    config: &iroha_config::parameters::actual::Torii,
    admission: Option<Arc<sorafs::AdmissionRegistry>>,
) -> Option<Arc<RwLock<sorafs::ProviderAdvertCache>>> {
    if !config.sorafs_discovery.discovery_enabled {
        return None;
    }

    let Some(admission_registry) = admission else {
        iroha_logger::warn!("SoraFS discovery cache disabled: admission registry not available");
        return None;
    };

    let mut capabilities = Vec::new();
    for name in &config.sorafs_discovery.known_capabilities {
        match sorafs::parse_capability_name(name) {
            Some(capability) => capabilities.push(capability),
            None => {
                panic!("unknown SoraFS capability `{name}` in torii.sorafs.known_capabilities");
            }
        }
    }

    assert!(
        !capabilities.is_empty(),
        "torii.sorafs.known_capabilities must include at least one capability"
    );

    iroha_logger::info!(
        known = capabilities
            .iter()
            .map(|cap| sorafs::capability_name(*cap))
            .collect::<Vec<_>>()
            .join(", "),
        "SoraFS discovery API enabled"
    );

    sorafs::api::init_cache(true, capabilities, admission_registry)
}

#[cfg(feature = "app_api")]
fn load_sorafs_admission(
    config: &iroha_config::parameters::actual::Torii,
) -> Option<Arc<sorafs::AdmissionRegistry>> {
    let Some(admission_cfg) = config.sorafs_discovery.admission.as_ref() else {
        if config.sorafs_gateway.enforce_admission {
            iroha_logger::warn!(
                "torii.sorafs.admission_envelopes_dir not configured; admission enforcement will operate in allow-all mode"
            );
        }
        return None;
    };

    match sorafs::AdmissionRegistry::load_from_dir(&admission_cfg.envelopes_dir) {
        Ok(registry) => Some(Arc::new(registry)),
        Err(err) => {
            iroha_logger::error!(
                ?err,
                dir = ?admission_cfg.envelopes_dir,
                "failed to load SoraFS provider admission registry"
            );
            None
        }
    }
}

#[cfg(feature = "app_api")]
fn load_cdn_policy(
    path: Option<&PathBuf>,
) -> Option<iroha_data_model::sorafs::gar::GarCdnPolicyV1> {
    let policy_path = path?;
    match fs::read(policy_path) {
        Ok(bytes) => match norito::json::from_slice(&bytes) {
            Ok(policy) => Some(policy),
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    path = ?policy_path,
                    "failed to parse SoraFS CDN policy payload"
                );
                None
            }
        },
        Err(err) => {
            iroha_logger::warn!(
                ?err,
                path = ?policy_path,
                "failed to read SoraFS CDN policy payload"
            );
            None
        }
    }
}

#[cfg(feature = "app_api")]
#[derive(Clone)]
struct GatewaySecurityComponents {
    policy: Arc<sorafs::gateway::GatewayPolicy>,
    denylist: Arc<sorafs::gateway::GatewayDenylist>,
    tls_state: Arc<RwLock<sorafs::gateway::TlsStateSnapshot>>,
    tls_automation: Option<Arc<sorafs::gateway::TlsAutomationHandle>>,
    blinded_resolver: Option<Arc<sorafs::BlindedCidResolver>>,
}

#[cfg(feature = "app_api")]
fn build_sorafs_gateway_security(
    config: &iroha_config::parameters::actual::SorafsGateway,
    admission: Option<Arc<sorafs::AdmissionRegistry>>,
) -> GatewaySecurityComponents {
    use sorafs::gateway::{
        AcmeConfig, ChallengeProfile, GatewayDenylist, GatewayPolicy, GatewayPolicyConfig,
        GatewayRateLimitConfig, GatewayRateLimiter, TlsAutomationHandle, TlsStateSnapshot,
    };

    let denylist = Arc::new(GatewayDenylist::new());
    populate_gateway_denylist(&denylist, &config.denylist);
    let rate_limit = GatewayRateLimitConfig {
        max_requests: config.rate_limit.max_requests.map(NonZeroU32::get),
        window: config.rate_limit.window,
        ban_duration: config.rate_limit.ban,
    };
    let cdn_policy = load_cdn_policy(config.cdn_policy_path.as_ref());
    let policy_config = GatewayPolicyConfig {
        require_manifest_envelope: config.require_manifest_envelope,
        enforce_admission: config.enforce_admission,
        rate_limit: rate_limit.clone(),
        cdn_policy,
    };
    let rate_limiter = GatewayRateLimiter::new(rate_limit);
    let policy = Arc::new(GatewayPolicy::new(
        policy_config,
        admission,
        denylist.clone(),
        rate_limiter,
    ));
    let tls_state = Arc::new(RwLock::new(TlsStateSnapshot::new(config.acme.ech_enabled)));

    let tls_automation = if config.acme.enabled {
        if config.acme.hostnames.is_empty() {
            iroha_logger::warn!(
                "SoraFS TLS automation enabled but no hostnames configured; automation skipped"
            );
            None
        } else {
            let automation_config = AcmeConfig {
                enabled: config.acme.enabled,
                account_email: config.acme.account_email.clone(),
                directory_url: config.acme.directory_url.clone(),
                hostnames: config.acme.hostnames.clone(),
                dns_provider_id: config.acme.dns_provider_id.clone(),
                renewal_window: config.acme.renewal_window,
                retry_backoff: config.acme.retry_backoff,
                retry_jitter: config.acme.retry_jitter,
                challenge: ChallengeProfile {
                    dns01: config.acme.challenges.dns01,
                    tls_alpn_01: config.acme.challenges.tls_alpn_01,
                },
            };

            Some(Arc::new(TlsAutomationHandle::new(
                automation_config,
                Arc::clone(&tls_state),
            )))
        }
    } else {
        None
    };

    let blinded_resolver = config.salt_schedule_dir.as_ref().map_or_else(
        || None,
        |dir| match sorafs::SaltSchedule::load_from_dir(dir) {
            Ok(schedule) => {
                let schedule = Arc::new(schedule);
                Some(Arc::new(sorafs::BlindedCidResolver::new(schedule)))
            }
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    "failed to load SoraNet salt schedule; blinded CID support disabled"
                );
                None
            }
        },
    );

    GatewaySecurityComponents {
        policy,
        denylist,
        tls_state,
        tls_automation,
        blinded_resolver,
    }
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, JsonDeserialize)]
struct GatewayDenylistFileEntry {
    kind: String,
    provider_id_hex: Option<String>,
    manifest_digest_hex: Option<String>,
    cid_b64: Option<String>,
    cid_hex: Option<String>,
    cid_utf8: Option<String>,
    url: Option<String>,
    account_id: Option<String>,
    account_alias: Option<String>,
    jurisdiction: Option<String>,
    reason: Option<String>,
    issued_at: Option<String>,
    expires_at: Option<String>,
    policy_tier: Option<String>,
    emergency_canon: Option<String>,
    governance_reference: Option<String>,
}

#[cfg(feature = "app_api")]
struct GatewayDenylistPolicyMeta {
    tier: sorafs::gateway::DenylistPolicyTier,
    canon: Option<String>,
    governance_reference: Option<String>,
}

#[cfg(feature = "app_api")]
fn populate_gateway_denylist(
    denylist: &Arc<sorafs::gateway::GatewayDenylist>,
    config: &iroha_config::parameters::actual::SorafsGatewayDenylist,
) {
    let Some(path) = &config.path else {
        return;
    };

    let policy = gateway_denylist_policy_from_config(config);

    let data = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            iroha_logger::warn!(?err, path = ?path, "failed to read gateway denylist file");
            return;
        }
    };

    let entries: Vec<GatewayDenylistFileEntry> = match norito::json::from_slice(&data) {
        Ok(entries) => entries,
        Err(err) => {
            iroha_logger::warn!(?err, path = ?path, "failed to parse gateway denylist file");
            return;
        }
    };

    let mut loaded = 0usize;
    for entry in entries {
        let result = match entry.kind.as_str() {
            "provider" => build_provider_denylist_entry(&entry, &policy),
            "manifest_digest" => build_manifest_digest_denylist_entry(&entry, &policy),
            "cid" => build_cid_denylist_entry(&entry, &policy),
            "url" => build_url_denylist_entry(&entry, &policy),
            "account_id" => build_account_id_denylist_entry(&entry, &policy),
            "account_alias" => build_account_alias_denylist_entry(&entry, &policy),
            other => {
                iroha_logger::warn!(
                    kind = other,
                    path = ?path,
                    "unsupported gateway denylist entry kind"
                );
                continue;
            }
        };

        match result {
            Ok((kind, metadata)) => {
                denylist.upsert(kind, metadata);
                loaded += 1;
            }
            Err(err) => {
                iroha_logger::warn!(
                    error = %err,
                    kind = %entry.kind,
                    path = ?path,
                    "failed to load gateway denylist entry"
                );
            }
        }
    }

    iroha_logger::info!(count = loaded, path = ?path, "loaded gateway denylist entries");
}

#[cfg(feature = "app_api")]
fn build_denylist_entry_metadata(
    entry: &GatewayDenylistFileEntry,
) -> Result<
    (
        sorafs::gateway::DenylistEntryBuilder,
        GatewayDenylistPolicyMeta,
    ),
    String,
> {
    let mut builder = sorafs::gateway::DenylistEntryBuilder::default();
    let mut issued_set = false;
    if let Some(jurisdiction) = &entry.jurisdiction {
        builder = builder.jurisdiction(jurisdiction.clone());
    }
    if let Some(reason) = &entry.reason {
        builder = builder.reason(reason.clone());
    }
    if let Some(issued_at) = parse_optional_timestamp(entry.issued_at.as_deref())? {
        builder = builder.issued_at(issued_at);
        issued_set = true;
    }
    if let Some(expires_at) = parse_optional_timestamp(entry.expires_at.as_deref())? {
        builder = builder.expires_at(expires_at);
    }
    if !issued_set {
        builder = builder.issued_at(SystemTime::now());
    }
    let tier = parse_policy_tier(entry.policy_tier.as_deref())?;
    let canon = entry
        .emergency_canon
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(std::borrow::ToOwned::to_owned);
    let governance_reference = entry
        .governance_reference
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(std::borrow::ToOwned::to_owned);
    Ok((
        builder,
        GatewayDenylistPolicyMeta {
            tier,
            canon,
            governance_reference,
        },
    ))
}

#[cfg(feature = "app_api")]
fn build_provider_denylist_entry(
    entry: &GatewayDenylistFileEntry,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<
    (
        sorafs::gateway::DenylistKind,
        sorafs::gateway::DenylistEntry,
    ),
    String,
> {
    let provider_hex = entry
        .provider_id_hex
        .as_ref()
        .ok_or_else(|| "provider_id_hex is required for provider denylist entries".to_string())?;
    let bytes = hex::decode(provider_hex)
        .map_err(|err| format!("invalid provider_id_hex `{provider_hex}`: {err}"))?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "provider_id_hex must be 32 bytes".to_string())?;
    let (builder, meta) = build_denylist_entry_metadata(entry)?;
    let metadata = finalize_gateway_denylist_entry(builder, meta, policy)?;

    Ok((sorafs::gateway::DenylistKind::Provider(array), metadata))
}

#[cfg(feature = "app_api")]
fn build_manifest_digest_denylist_entry(
    entry: &GatewayDenylistFileEntry,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<
    (
        sorafs::gateway::DenylistKind,
        sorafs::gateway::DenylistEntry,
    ),
    String,
> {
    let digest_hex = entry.manifest_digest_hex.as_ref().ok_or_else(|| {
        "manifest_digest_hex is required for manifest_digest denylist entries".to_string()
    })?;
    let bytes = hex::decode(digest_hex)
        .map_err(|err| format!("invalid manifest_digest_hex `{digest_hex}`: {err}"))?;
    let array: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "manifest_digest_hex must be 32 bytes".to_string())?;
    let (builder, meta) = build_denylist_entry_metadata(entry)?;
    let metadata = finalize_gateway_denylist_entry(builder, meta, policy)?;

    Ok((
        sorafs::gateway::DenylistKind::ManifestDigest(array),
        metadata,
    ))
}

#[cfg(feature = "app_api")]
fn build_cid_denylist_entry(
    entry: &GatewayDenylistFileEntry,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<
    (
        sorafs::gateway::DenylistKind,
        sorafs::gateway::DenylistEntry,
    ),
    String,
> {
    let cid_bytes = if let Some(encoded) = entry.cid_b64.as_ref() {
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|err| format!("invalid cid_b64 `{encoded}`: {err}"))?
    } else if let Some(hex_value) = entry.cid_hex.as_ref() {
        hex::decode(hex_value).map_err(|err| format!("invalid cid_hex `{hex_value}`: {err}"))?
    } else if let Some(text) = entry.cid_utf8.as_ref() {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err("cid_utf8 must not be empty".to_string());
        }
        trimmed.as_bytes().to_vec()
    } else {
        return Err(
            "cid denylist entries require `cid_b64`, `cid_hex`, or `cid_utf8` field".to_string(),
        );
    };

    if cid_bytes.is_empty() {
        return Err("CID payload must not be empty".to_string());
    }

    let (builder, meta) = build_denylist_entry_metadata(entry)?;
    let metadata = finalize_gateway_denylist_entry(builder, meta, policy)?;
    Ok((sorafs::gateway::DenylistKind::Cid(cid_bytes), metadata))
}

#[cfg(feature = "app_api")]
fn build_url_denylist_entry(
    entry: &GatewayDenylistFileEntry,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<
    (
        sorafs::gateway::DenylistKind,
        sorafs::gateway::DenylistEntry,
    ),
    String,
> {
    let url = entry
        .url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "url is required for url denylist entries".to_string())?;
    let (builder, meta) = build_denylist_entry_metadata(entry)?;
    let metadata = finalize_gateway_denylist_entry(builder, meta, policy)?;
    Ok((sorafs::gateway::DenylistKind::Url(url.to_owned()), metadata))
}

#[cfg(feature = "app_api")]
fn build_account_id_denylist_entry(
    entry: &GatewayDenylistFileEntry,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<
    (
        sorafs::gateway::DenylistKind,
        sorafs::gateway::DenylistEntry,
    ),
    String,
> {
    let account_id_raw = entry
        .account_id
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "account_id is required for account_id denylist entries".to_string())?;
    let (account_address, format) = AccountAddress::parse_any(account_id_raw, None)
        .map_err(|err| format!("invalid account_id `{account_id_raw}`: {err}"))?;
    if !matches!(format, AccountAddressFormat::CanonicalHex) {
        iroha_logger::debug!(
            literal = %account_id_raw,
            ?format,
            "gateway denylist normalized account address literal"
        );
    }
    let canonical = account_address
        .canonical_hex()
        .map_err(|err| format!("failed to encode canonical account address: {err}"))?;

    let (builder, meta) = build_denylist_entry_metadata(entry)?;
    let mut builder = builder;
    if let Some(alias) = entry.account_alias.as_ref().map(|value| value.trim()) {
        if alias.is_empty() {
            return Err("account_alias must not be empty when provided".to_string());
        }
        builder = builder.alias(alias.to_owned());
    }

    let metadata = finalize_gateway_denylist_entry(builder, meta, policy)?;

    Ok((
        sorafs::gateway::DenylistKind::AccountId(canonical),
        metadata,
    ))
}

#[cfg(feature = "app_api")]
fn build_account_alias_denylist_entry(
    entry: &GatewayDenylistFileEntry,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<
    (
        sorafs::gateway::DenylistKind,
        sorafs::gateway::DenylistEntry,
    ),
    String,
> {
    let alias = entry
        .account_alias
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "account_alias is required for account_alias denylist entries".to_string()
        })?;

    let (builder, meta) = build_denylist_entry_metadata(entry)?;
    let builder = builder.alias(alias.to_owned());
    let metadata = finalize_gateway_denylist_entry(builder, meta, policy)?;

    Ok((
        sorafs::gateway::DenylistKind::AccountAlias(alias.to_owned()),
        metadata,
    ))
}

#[cfg(all(test, feature = "app_api"))]
mod gateway_denylist_loader_tests {
    use iroha_crypto::PublicKey;
    use iroha_data_model::{account::AccountId, domain::DomainId};

    use super::*;

    fn sample_policy() -> sorafs::gateway::DenylistPolicy {
        sorafs::gateway::DenylistPolicy::new(
            Duration::from_secs(600),
            Duration::from_secs(60),
            Duration::from_secs(30),
            true,
        )
    }

    fn sample_account_literals() -> (String, String, String) {
        let domain: DomainId = "wonderland".parse().expect("domain parses");
        let public_key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("public key parses");
        let account = AccountId::new(domain, public_key);
        let address = AccountAddress::from_account_id(&account).expect("address from account_id");
        let canonical = address.canonical_hex().expect("canonical hex");
        let ih58 = address.to_ih58(42).expect("ih58 encoding");
        let compressed = address.to_compressed_sora().expect("compressed encoding");
        (canonical, ih58, compressed)
    }

    fn base_account_entry() -> GatewayDenylistFileEntry {
        GatewayDenylistFileEntry {
            kind: "account_id".to_string(),
            provider_id_hex: None,
            manifest_digest_hex: None,
            cid_b64: None,
            cid_hex: None,
            cid_utf8: None,
            url: None,
            account_id: None,
            account_alias: None,
            jurisdiction: None,
            reason: None,
            issued_at: None,
            expires_at: None,
            policy_tier: None,
            emergency_canon: None,
            governance_reference: None,
        }
    }

    #[test]
    fn account_id_entries_normalise_to_canonical_hex() {
        let (sample_canonical, _, _) = sample_account_literals();
        let mut entry = base_account_entry();
        entry.account_id = Some(sample_canonical.clone());
        entry.account_alias = Some("routing@sora".to_string());
        entry.jurisdiction = Some("US".to_string());
        entry.reason = Some("test".to_string());
        entry.issued_at = Some("2025-01-01T00:00:00Z".to_string());

        let policy = sample_policy();
        let (kind, metadata) = build_account_id_denylist_entry(&entry, &policy).expect("entry");

        match kind {
            sorafs::gateway::DenylistKind::AccountId(ref canonical) => {
                assert_eq!(canonical, &sample_canonical);
            }
            other => panic!("unexpected denylist kind: {other:?}"),
        }

        assert_eq!(metadata.alias(), Some("routing@sora"));
        assert_eq!(metadata.jurisdiction(), Some("US"));
        assert_eq!(metadata.reason(), Some("test"));
    }

    #[test]
    fn account_alias_entries_require_alias_field() {
        let entry = GatewayDenylistFileEntry {
            kind: "account_alias".to_string(),
            provider_id_hex: None,
            manifest_digest_hex: None,
            cid_b64: None,
            cid_hex: None,
            cid_utf8: None,
            url: None,
            account_id: None,
            account_alias: Some("alias@sora".to_string()),
            jurisdiction: None,
            reason: Some("alias".to_string()),
            issued_at: Some("2025-04-15T00:00:00Z".to_string()),
            expires_at: Some("2025-04-15T00:05:00Z".to_string()),
            policy_tier: None,
            emergency_canon: None,
            governance_reference: None,
        };

        let policy = sample_policy();
        let (kind, metadata) =
            build_account_alias_denylist_entry(&entry, &policy).expect("alias entry");

        match kind {
            sorafs::gateway::DenylistKind::AccountAlias(ref alias) => {
                assert_eq!(alias, "alias@sora");
            }
            other => panic!("unexpected denylist kind: {other:?}"),
        }

        assert_eq!(metadata.alias(), Some("alias@sora"));
        assert_eq!(metadata.reason(), Some("alias"));
    }

    #[test]
    fn account_id_entries_accept_ih58_literals() {
        let (canonical, ih58, _) = sample_account_literals();
        let mut entry = base_account_entry();
        entry.account_id = Some(ih58);
        entry.issued_at = Some("2025-01-01T00:00:00Z".to_string());

        let policy = sample_policy();
        let (kind, _) = build_account_id_denylist_entry(&entry, &policy).expect("ih58 entry");

        match kind {
            sorafs::gateway::DenylistKind::AccountId(ref value) => assert_eq!(value, &canonical),
            other => panic!("unexpected denylist kind: {other:?}"),
        }
    }

    #[test]
    fn account_id_entries_accept_compressed_literals() {
        let (canonical, _, compressed) = sample_account_literals();
        let mut entry = base_account_entry();
        entry.account_id = Some(compressed);
        entry.issued_at = Some("2025-01-01T00:00:00Z".to_string());

        let policy = sample_policy();
        let (kind, _) = build_account_id_denylist_entry(&entry, &policy).expect("compressed entry");

        match kind {
            sorafs::gateway::DenylistKind::AccountId(ref value) => assert_eq!(value, &canonical),
            other => panic!("unexpected denylist kind: {other:?}"),
        }
    }
}

#[cfg(feature = "app_api")]
fn parse_optional_timestamp(value: Option<&str>) -> Result<Option<SystemTime>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };

    let datetime = OffsetDateTime::parse(raw, &Rfc3339)
        .map_err(|err| format!("invalid timestamp `{raw}`: {err}"))?;
    let duration = datetime - OffsetDateTime::UNIX_EPOCH;
    if duration.is_negative() {
        return Err(format!("timestamp `{raw}` is before UNIX epoch"));
    }
    let std_duration: Duration = duration
        .try_into()
        .map_err(|err| format!("timestamp `{raw}` out of range: {err}"))?;
    Ok(Some(SystemTime::UNIX_EPOCH + std_duration))
}

#[cfg(feature = "app_api")]
fn build_sorafs_quota_config(
    config: &iroha_config::parameters::actual::SorafsQuota,
) -> sorafs::SorafsQuotaConfig {
    fn convert_window(
        window: &iroha_config::parameters::actual::SorafsQuotaWindow,
    ) -> sorafs::SorafsQuotaWindow {
        sorafs::SorafsQuotaWindow {
            max_events: window.max_events.map(std::num::NonZeroU32::get),
            window: window.window,
        }
    }

    sorafs::SorafsQuotaConfig {
        capacity_declaration: convert_window(&config.capacity_declaration),
        capacity_telemetry: convert_window(&config.capacity_telemetry),
        deal_telemetry: convert_window(&config.deal_telemetry),
        capacity_dispute: convert_window(&config.capacity_dispute),
        storage_pin: convert_window(&config.storage_pin),
        por_submission: convert_window(&config.por_submission),
    }
}

#[cfg(feature = "app_api")]
fn build_por_components(
    config: &Config,
    sorafs_node: &sorafs_node::NodeHandle,
) -> (
    Arc<sorafs::PorCoordinator>,
    Option<Arc<sorafs::PorCoordinatorRuntime>>,
) {
    let por_cfg = &config.sorafs_por;
    let snapshot_path = por_cfg
        .governance_dag_dir
        .join("por_coordinator_snapshot.norito");
    let coordinator = match sorafs::PorCoordinator::with_persistence(&snapshot_path) {
        Ok(coord) => Arc::new(coord),
        Err(err) => {
            iroha_logger::warn!(
                ?err,
                path = ?snapshot_path,
                "failed to load PoR coordinator snapshot; continuing with in-memory history"
            );
            Arc::new(sorafs::PorCoordinator::new())
        }
    };

    if !por_cfg.enabled {
        return (coordinator, None);
    }
    if !sorafs_node.is_enabled() {
        iroha_logger::warn!(
            "PoR coordinator runtime disabled: SoraFS storage is not enabled for this node"
        );
        return (coordinator, None);
    }

    if let Err(err) = fs::create_dir_all(&por_cfg.governance_dag_dir) {
        iroha_logger::error!(
            ?err,
            path = ?por_cfg.governance_dag_dir,
            "failed to prepare governance DAG directory for PoR automation"
        );
        return (coordinator, None);
    }

    let publisher = Arc::new(sorafs::FilesystemGovernancePublisher::new(
        por_cfg.governance_dag_dir.clone(),
    ));
    let randomness = Arc::new(sorafs::DeterministicRandomnessProvider::new(
        por_cfg.randomness_seed,
    ));
    let vrf_provider: Arc<dyn sorafs::VrfProvider> = Arc::new(sorafs::EmptyVrfProvider);
    let storage: Arc<dyn sorafs::PorStorage> = Arc::new(sorafs_node.clone());

    let runtime = Arc::new(sorafs::PorCoordinatorRuntime::new(
        storage,
        coordinator.clone(),
        randomness,
        vrf_provider,
        publisher,
        por_cfg.epoch_interval_secs,
        por_cfg.response_window_secs,
    ));

    iroha_logger::info!(
        epoch_interval_secs = por_cfg.epoch_interval_secs,
        response_window_secs = por_cfg.response_window_secs,
        path = ?por_cfg.governance_dag_dir,
        "PoR coordinator runtime initialised"
    );

    (coordinator, Some(runtime))
}

#[cfg(feature = "app_api")]
fn build_repair_runtime(
    sorafs_node: &sorafs_node::NodeHandle,
) -> Option<Arc<sorafs::RepairWorkerRuntime>> {
    let config = sorafs_node.repair_config();
    if !config.enabled() {
        return None;
    }
    if !sorafs_node.is_enabled() {
        iroha_logger::warn!("repair runtime disabled: SoraFS storage is not enabled for this node");
        return None;
    }

    let runtime = Arc::new(sorafs::RepairWorkerRuntime::new(
        sorafs_node.clone(),
        config,
    ));

    iroha_logger::info!(
        heartbeat_interval_secs = config.heartbeat_interval_secs(),
        workers = config.worker_concurrency(),
        "SoraFS repair runtime initialised"
    );

    Some(runtime)
}

#[cfg(feature = "app_api")]
fn build_gc_runtime(
    sorafs_node: &sorafs_node::NodeHandle,
) -> Option<Arc<sorafs::GcSweeperRuntime>> {
    let config = sorafs_node.gc_config();
    if !config.enabled() {
        return None;
    }
    if !sorafs_node.is_enabled() {
        iroha_logger::warn!("GC runtime disabled: SoraFS storage is not enabled for this node");
        return None;
    }

    let runtime = Arc::new(sorafs::GcSweeperRuntime::new(sorafs_node.clone(), config));

    iroha_logger::info!(
        interval_secs = config.interval_secs(),
        max_deletions = config.max_deletions_per_run(),
        retention_grace_secs = config.retention_grace_secs(),
        "SoraFS GC runtime initialised"
    );

    Some(runtime)
}

#[cfg(feature = "app_api")]
async fn handler_confidential_derive_keyset_route(
    State(app): State<SharedAppState>,
    payload: crate::NoritoJson<routing::ConfidentialKeyRequest>,
) -> Result<impl IntoResponse> {
    routing::handle_post_confidential_derive_keyset(
        app.chain_id.clone(),
        app.queue.clone(),
        app.state.clone(),
        app.telemetry.clone(),
        payload,
    )
    .await
}

/// Torii errors.
#[derive(thiserror::Error, displaydoc::Display, pretty_error_debug::Debug)]
pub enum Error {
    /// Failed to process query
    Query(#[from] iroha_data_model::ValidationFail),
    /// Validation error for app-facing query parameters `{code}`: {message}
    AppQueryValidation {
        /// Stable machine-readable code.
        code: &'static str,
        /// Human-readable error message.
        message: String,
    },
    /// Proof endpoint `{endpoint}` throttled the request; retry after {retry_after_secs}s
    ProofRateLimited {
        /// Logical endpoint label.
        endpoint: &'static str,
        /// Retry hint in whole seconds.
        retry_after_secs: u64,
    },
    /// API version negotiation or gating failed.
    ApiVersion(api_version::ApiVersionError),
    /// Failed to accept transaction
    AcceptTransaction(#[from] iroha_core::tx::AcceptTransactionFail),
    /// Failed to get or set configuration
    Config(#[source] eyre::Report),
    /// Failed to serialize response payload for `{context}`: {source}
    SerializationFailure {
        /// Logical context for the serialization failure.
        context: &'static str,
        /// Underlying Norito JSON error.
        #[source]
        source: norito::json::Error,
    },
    /// Failed to apply Nexus lane lifecycle plan: {reason}
    LaneLifecycle {
        /// Human-readable reason for the failure.
        reason: String,
    },
    /// Failed to push into queue ({source}; backpressure={backpressure:?})
    PushIntoQueue {
        /// Root cause from the core queue implementation.
        #[source]
        source: Box<queue::Error>,
        /// Snapshot of queue load when the error happened.
        backpressure: queue::BackpressureState,
    },
    #[cfg(feature = "telemetry")]
    /// Failed to get Prometheus metrics
    Prometheus(#[source] eyre::Report),
    #[cfg(feature = "profiling")]
    /// Failed to get pprof profile
    Pprof(#[source] eyre::Report),
    #[cfg(feature = "telemetry")]
    /// Failed to get status
    StatusFailure(#[source] eyre::Report),
    #[cfg(feature = "telemetry")]
    /// Telemetry endpoint {endpoint} disabled by active profile {profile:?}
    TelemetryProfileRestricted {
        /// Logical endpoint or capability name
        endpoint: &'static str,
        /// Active telemetry profile when denied
        profile: TelemetryProfile,
    },
    /// Failure caused by configuration subsystem
    ConfigurationFailure(#[from] KisoError),
    /// Failed to find status segment by provided path
    StatusSegmentNotFound(#[source] eyre::Report),
    /// Failed to start Torii
    StartServer,
    /// Torii server terminated with an error
    FailedExit,
}

fn offline_reject_code_from_message(message: &str) -> Option<&str> {
    use iroha_data_model::offline::OFFLINE_REJECTION_REASON_PREFIX;

    let start = message.find(OFFLINE_REJECTION_REASON_PREFIX)?;
    let rest = &message[start + OFFLINE_REJECTION_REASON_PREFIX.len()..];
    let (label, _) = rest.split_once(':')?;
    if label.is_empty() { None } else { Some(label) }
}

fn offline_reject_code_from_query_fail(
    fail: &iroha_data_model::query::error::QueryExecutionFail,
) -> Option<&str> {
    use iroha_data_model::query::error::QueryExecutionFail as Q;

    match fail {
        Q::Conversion(message) => offline_reject_code_from_message(message),
        _ => None,
    }
}

fn offline_reject_code_from_instruction_fail(
    fail: &iroha_data_model::isi::error::InstructionExecutionError,
) -> Option<&str> {
    use iroha_data_model::isi::error::InstructionExecutionError as I;

    match fail {
        I::Conversion(message) => offline_reject_code_from_message(message),
        I::InvariantViolation(message) => offline_reject_code_from_message(message.as_ref()),
        I::Query(fail) => offline_reject_code_from_query_fail(fail),
        _ => None,
    }
}

fn offline_reject_code_from_validation_fail(
    fail: &iroha_data_model::ValidationFail,
) -> Option<&str> {
    use iroha_data_model::ValidationFail as V;

    match fail {
        V::NotPermitted(message) => offline_reject_code_from_message(message),
        V::QueryFailed(fail) => offline_reject_code_from_query_fail(fail),
        V::InstructionFailed(fail) => offline_reject_code_from_instruction_fail(fail),
        _ => None,
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::Query(err) => {
                let status = Self::query_status_code(&err);
                let offline_reason =
                    offline_reject_code_from_validation_fail(&err).map(str::to_owned);
                let axt = match &err {
                    iroha_data_model::ValidationFail::AxtReject(ctx) => Some(ctx.clone()),
                    _ => None,
                };
                let mut response = (status, utils::NoritoBody(err)).into_response();
                let headers = response.headers_mut();
                if let Some(ctx) = axt {
                    headers.insert(
                        HeaderName::from_static("x-iroha-axt-code"),
                        HeaderValue::from_static(ctx.reason.code()),
                    );
                    headers.insert(
                        HeaderName::from_static("x-iroha-axt-reason"),
                        HeaderValue::from_static(ctx.reason.label()),
                    );
                    if ctx.snapshot_version > 0 {
                        if let Ok(value) = HeaderValue::from_str(&ctx.snapshot_version.to_string())
                        {
                            headers.insert(
                                HeaderName::from_static("x-iroha-axt-snapshot-version"),
                                value,
                            );
                        }
                    }
                    if let Some(dsid) = ctx.dataspace {
                        if let Ok(value) = HeaderValue::from_str(&dsid.as_u64().to_string()) {
                            headers.insert(HeaderName::from_static("x-iroha-axt-dataspace"), value);
                        }
                    }
                    if let Some(lane) = ctx.lane {
                        if let Ok(value) = HeaderValue::from_str(&lane.as_u32().to_string()) {
                            headers.insert(HeaderName::from_static("x-iroha-axt-lane"), value);
                        }
                    }
                    if let Some(hint) = ctx.next_min_handle_era {
                        if let Ok(value) = HeaderValue::from_str(&hint.to_string()) {
                            headers.insert(
                                HeaderName::from_static("x-iroha-axt-next-handle-era"),
                                value,
                            );
                        }
                    }
                    if let Some(hint) = ctx.next_min_sub_nonce {
                        if let Ok(value) = HeaderValue::from_str(&hint.to_string()) {
                            headers.insert(
                                HeaderName::from_static("x-iroha-axt-next-sub-nonce"),
                                value,
                            );
                        }
                    }
                }
                if headers.get("x-iroha-reject-code").is_none() {
                    if let Some(code) = offline_reason {
                        if let Ok(header) = HeaderValue::from_str(&code) {
                            headers.insert(HeaderName::from_static("x-iroha-reject-code"), header);
                        }
                    }
                }
                response
            }
            Self::AppQueryValidation { code, message } => {
                let payload = ErrorEnvelope::new(code, message);
                let status = if code == "type_mismatch" {
                    StatusCode::UNPROCESSABLE_ENTITY
                } else {
                    StatusCode::BAD_REQUEST
                };
                (status, utils::NoritoBody(payload)).into_response()
            }
            Self::ProofRateLimited {
                endpoint,
                retry_after_secs,
            } => {
                let payload = crate::json_object(vec![
                    crate::json_entry("error", "rate_limited"),
                    crate::json_entry("endpoint", endpoint),
                    crate::json_entry("retry_after_secs", retry_after_secs),
                ]);
                let mut resp = utils::JsonBody(payload).into_response();
                *resp.status_mut() = StatusCode::TOO_MANY_REQUESTS;
                if let Ok(header) = HeaderValue::from_str(&retry_after_secs.to_string()) {
                    resp.headers_mut()
                        .insert(axum::http::header::RETRY_AFTER, header);
                }
                resp
            }
            Self::LaneLifecycle { reason } => (
                StatusCode::BAD_REQUEST,
                utils::NoritoBody(iroha_data_model::ValidationFail::NotPermitted(reason)),
            )
                .into_response(),
            Self::PushIntoQueue {
                source,
                backpressure,
            } => {
                let status = Self::status_code_for_queue_error(source.as_ref());
                let envelope = Self::queue_error_envelope(source.as_ref(), backpressure);
                let mut response = (status, utils::NoritoBody(envelope)).into_response();
                let headers = response.headers_mut();
                let (reject_code, _detail) = queue_rejection_metadata(source.as_ref());
                if let Ok(header) = HeaderValue::from_str(reject_code) {
                    headers.insert(HeaderName::from_static("x-iroha-reject-code"), header);
                }

                if let Ok(depth) = HeaderValue::from_str(&backpressure.queued().to_string()) {
                    headers.insert("X-Iroha-Queue-Depth", depth);
                }
                if let Ok(capacity) =
                    HeaderValue::from_str(&backpressure.capacity().get().to_string())
                {
                    headers.insert("X-Iroha-Queue-Capacity", capacity);
                }
                headers.insert(
                    "X-Iroha-Queue-State",
                    if backpressure.is_saturated() {
                        HeaderValue::from_static("saturated")
                    } else {
                        HeaderValue::from_static("healthy")
                    },
                );

                if matches!(
                    source.as_ref(),
                    queue::Error::Full | queue::Error::MaximumTransactionsPerUser
                ) {
                    headers.insert("Retry-After", HeaderValue::from_static("1"));
                }

                response
            }
            Self::AcceptTransaction(err) => {
                let (code, detail) = accept_transaction_metadata(&err);
                let mut response = (
                    StatusCode::BAD_REQUEST,
                    utils::NoritoBody(ErrorEnvelope::new(code, detail)),
                )
                    .into_response();
                if let Ok(header) = HeaderValue::from_str(code) {
                    response
                        .headers_mut()
                        .insert(HeaderName::from_static("x-iroha-reject-code"), header);
                }
                response
            }
            other => {
                let status = other.status_code();
                let payload = other.into_envelope();
                (status, utils::NoritoBody(payload)).into_response()
            }
        }
    }
}

#[allow(dead_code)]
fn _assert_torii_types_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Error>();
    assert_send_sync::<AppState>();
}

#[cfg(test)]
pub(crate) mod tests_runtime_handlers {
    use std::{
        collections::HashSet,
        net::SocketAddr,
        num::{NonZeroU32, NonZeroU64, NonZeroUsize},
        sync::{Arc, LazyLock, Mutex, MutexGuard},
        time::{Duration, Instant},
    };

    use axum::{
        extract::{Extension, State},
        http::{HeaderMap, HeaderValue, StatusCode},
        response::IntoResponse,
    };
    use base64::Engine as _;
    use futures::executor;
    use iroha_config::{
        client_api::ConfigGetDTO,
        parameters::{
            actual::{NoritoRpcStage, NoritoRpcTransport, TelemetryProfile},
            defaults,
        },
    };
    use iroha_core::{
        kiso::KisoHandle,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::Queue,
        state::{State as IrohaState, World},
        sumeragi::{
            consensus::{PERMISSIONED_TAG, Phase, Vote, vote_preimage},
            status::record_commit_qc,
        },
        tx::AcceptedTransaction,
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId, ValidationFail,
        account::{Account, AccountId, AccountLabel, OpaqueAccountId},
        block::{BlockSignature, SignedBlock},
        consensus::{Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
        domain::{Domain, DomainId},
        events::pipeline::{BlockEvent, BlockStatus, TransactionEvent, TransactionStatus},
        isi::Log,
        level::Level,
        name::Name,
        nexus::{AxtPolicySnapshot, AxtRejectReason, DataSpaceId, LaneId, UniversalAccountId},
        peer::{Peer, PeerId},
        soranet::privacy_metrics::{
            SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1,
            SoranetPrivacyEventV1, SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1,
        },
        transaction::{
            error::TransactionRejectionReason,
            signed::{TransactionBuilder, TransactionResultInner},
        },
        trigger::DataTriggerSequence,
    };

    use super::*;
    #[cfg(feature = "telemetry")]
    use crate::{RecordSoranetPrivacyEventDto, RecordSoranetPrivacyShareDto};
    use crate::{
        routing::{
            ActivateInstanceDto, DeployContractDto, RegisterContractCodeDto,
            handle_v1_sumeragi_commit_qcs,
        },
        utils::extractors::NoritoJson,
    };

    pub fn negotiated(app: &SharedAppState) -> Extension<api_version::NegotiatedVersion> {
        Extension(api_version::NegotiatedVersion {
            version: app.api_versions.default,
            inferred: true,
        })
    }

    #[cfg(feature = "app_api")]
    pub(crate) struct AccountResolverGuard {
        _lock: MutexGuard<'static, ()>,
    }

    #[cfg(feature = "app_api")]
    impl Drop for AccountResolverGuard {
        fn drop(&mut self) {
            iroha_data_model::account::clear_account_alias_resolver();
            iroha_data_model::account::clear_account_domain_selector_resolver();
            iroha_data_model::account::clear_account_opaque_resolver();
            iroha_data_model::account::clear_account_uaid_resolver();
        }
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn guard_account_resolvers(app: &SharedAppState) -> AccountResolverGuard {
        static RESOLVER_GUARD: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        let lock = RESOLVER_GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        install_account_resolvers(&app.state);
        AccountResolverGuard { _lock: lock }
    }

    pub fn mk_app_state_for_tests() -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(World::default(), None, None, None, None)
    }

    pub fn mk_app_state_for_tests_with_iso_bridge(
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
    ) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(World::default(), iso, None, None, None)
    }

    pub fn mk_app_state_for_tests_with_options(
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
        deploy_limit: Option<(u32, u32)>,
        norito_rpc: Option<iroha_config::parameters::actual::NoritoRpcTransport>,
        push: Option<iroha_config::parameters::actual::Push>,
    ) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(
            World::default(),
            iso,
            deploy_limit,
            norito_rpc,
            push,
        )
    }

    pub fn mk_app_state_for_tests_with_world(world: World) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(world, None, None, None, None)
    }

    fn mk_app_state_for_tests_with_world_and_options(
        world: World,
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
        deploy_limit: Option<(u32, u32)>,
        norito_rpc: Option<iroha_config::parameters::actual::NoritoRpcTransport>,
        push: Option<iroha_config::parameters::actual::Push>,
    ) -> SharedAppState {
        // Minimal core state
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state_inner =
            iroha_core::state::State::new_for_testing(world, kura.clone(), query_handle.clone());
        {
            let mut topo_block = state_inner.commit_topology.block();
            topo_block.clear();
            let peer_id = iroha_data_model::peer::PeerId::from(
                iroha_crypto::KeyPair::random().public_key().clone(),
            );
            topo_block.push(peer_id);
            topo_block.commit();
        }
        let state = Arc::new(state_inner);

        // Minimal queue/events
        let events: EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue_cfg = iroha_config::parameters::actual::Queue::default();
        let queue = Arc::new(Queue::from_config(queue_cfg, events.clone()));
        let pipeline_status_cache = Arc::new(PipelineStatusCache::new());
        let chain_id: ChainId = "chain".parse().unwrap();
        // Minimal Kiso and peers provider (mocked to avoid spawning the full actor in tests)
        let cfg = crate::test_utils::mk_minimal_root_cfg();
        let kiso = KisoHandle::mock(&cfg);
        let (_tx, rx) =
            tokio::sync::watch::channel::<std::collections::HashSet<Peer>>(Default::default());
        let peers = OnlinePeersProvider::new(rx);

        #[cfg(feature = "connect")]
        let connect_cfg = iroha_config::parameters::actual::Connect {
            enabled: false,
            ws_max_sessions: iroha_config::parameters::defaults::connect::WS_MAX_SESSIONS,
            ws_per_ip_max_sessions:
                iroha_config::parameters::defaults::connect::WS_PER_IP_MAX_SESSIONS,
            ws_rate_per_ip_per_min:
                iroha_config::parameters::defaults::connect::WS_RATE_PER_IP_PER_MIN,
            session_ttl: iroha_config::parameters::defaults::connect::SESSION_TTL,
            frame_max_bytes: iroha_config::parameters::defaults::connect::FRAME_MAX_BYTES,
            session_buffer_max_bytes:
                iroha_config::parameters::defaults::connect::SESSION_BUFFER_MAX_BYTES,
            ping_interval: iroha_config::parameters::defaults::connect::PING_INTERVAL,
            ping_miss_tolerance: iroha_config::parameters::defaults::connect::PING_MISS_TOLERANCE,
            ping_min_interval: iroha_config::parameters::defaults::connect::PING_MIN_INTERVAL,
            dedupe_ttl: iroha_config::parameters::defaults::connect::DEDUPE_TTL,
            dedupe_cap: iroha_config::parameters::defaults::connect::DEDUPE_CAP,
            relay_enabled: false,
            relay_strategy: iroha_config::parameters::defaults::connect::RELAY_STRATEGY,
            p2p_ttl_hops: iroha_config::parameters::defaults::connect::P2P_TTL_HOPS,
        };
        #[cfg(feature = "push")]
        let push_cfg = push.unwrap_or_default();
        #[cfg(feature = "push")]
        let push_bridge = if push_cfg.enabled {
            Some(push::PushBridge::new(push_cfg.clone()))
        } else {
            None
        };
        #[cfg(feature = "app_api")]
        let sorafs_cache: Option<Arc<RwLock<sorafs::ProviderAdvertCache>>> = None;
        #[cfg(feature = "app_api")]
        let sorafs_node =
            sorafs_node::NodeHandle::new(sorafs_node::config::StorageConfig::default());
        #[cfg(feature = "app_api")]
        let sorafs_limits = Arc::new(sorafs::SorafsQuotaEnforcer::unlimited());
        #[cfg(feature = "app_api")]
        let sorafs_alias_cache = sorafs::policy_from_config(
            &iroha_config::parameters::actual::SorafsAliasCachePolicy::default(),
        );
        #[cfg(feature = "app_api")]
        let sorafs_alias_enforcement = sorafs::enforcement_from_config(
            &iroha_config::parameters::actual::SorafsAliasCachePolicy::default(),
        );
        #[cfg(feature = "app_api")]
        let stream_token_issuer: Option<Arc<sorafs::StreamTokenIssuer>> = None;
        #[cfg(feature = "app_api")]
        let sorafs_gateway_config = iroha_config::parameters::actual::SorafsGateway::default();
        #[cfg(feature = "app_api")]
        let sorafs_pin_policy = sorafs::PinSubmissionPolicy::from_config(
            &iroha_config::parameters::actual::SorafsStoragePin::default(),
        )
        .expect("default SoraFS pin policy should be valid");

        let telemetry = routing::MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
        let telemetry_profile = telemetry.profile();
        let rbc_sampling_manifest = SoftwareManifest::current();
        let rbc_chain_hash = Hash::new(chain_id.as_str().as_bytes());

        let iso_bridge = iso
            .as_ref()
            .and_then(|cfg| {
                crate::iso20022_bridge::Iso20022BridgeRuntime::from_config(cfg)
                    .expect("iso bridge config for tests should be valid")
            })
            .map(Arc::new);
        let alias_service = iso.as_ref().and_then(alias_service_from_iso_config);

        let deploy_rate_limiter = match deploy_limit {
            Some((rate, burst)) => limits::RateLimiter::new(Some(rate), Some(burst)),
            None => limits::RateLimiter::new(None, None),
        };
        let norito_rpc_cfg = norito_rpc.unwrap_or_default();
        let da_replay_cache = Arc::new(iroha_core::da::ReplayCache::new(
            iroha_core::da::ReplayCacheConfig::new(),
        ));
        let da_replay_store = Arc::new(da::ReplayCursorStore::in_memory());
        let da_ingest = iroha_config::parameters::actual::DaIngest::default();
        let da_receipt_signer = KeyPair::random();
        let da_receipt_log = Arc::new(da::DaReceiptLog::in_memory(
            Arc::clone(&da_replay_store),
            da_receipt_signer.public_key().clone(),
        ));

        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        let peer_telemetry = telemetry::peers::PeerTelemetryService::new(
            Vec::new(),
            telemetry::peers::GeoLookupConfig::disabled(),
        );

        let content_config_snapshot = state.content_snapshot();
        let soranet_privacy_ingest =
            iroha_config::parameters::actual::SoranetPrivacyIngest::default();
        let soranet_privacy_tokens: HashSet<String> =
            soranet_privacy_ingest.tokens.iter().cloned().collect();
        let soranet_privacy_allow_nets = limits::parse_cidrs(&soranet_privacy_ingest.allow_cidrs);
        let soranet_privacy_rate_limiter = limits::RateLimiter::new(
            soranet_privacy_ingest
                .rate_per_sec
                .map(std::num::NonZeroU32::get),
            soranet_privacy_ingest.burst.map(std::num::NonZeroU32::get),
        );
        let api_tokens_set: Arc<HashSet<String>> = Arc::new(Default::default());
        let operator_auth = Arc::new(
            operator_auth::OperatorAuth::new(
                iroha_config::parameters::actual::ToriiOperatorAuth::default(),
                api_tokens_set.clone(),
                defaults::torii::data_dir(),
                telemetry.clone(),
            )
            .expect("operator auth defaults should be valid"),
        );
        let operator_signatures = Arc::new(operator_signatures::OperatorSignatures::new(
            iroha_config::parameters::actual::ToriiOperatorSignatures::default(),
            da_receipt_signer.public_key().clone(),
            defaults::torii::MAX_CONTENT_LEN.get(),
            telemetry.clone(),
        ));

        let zk_ivm_prove_jobs = Arc::new(DashMap::new());
        let zk_ivm_prove_max_inflight = defaults::torii::ZK_IVM_PROVE_MAX_INFLIGHT.max(1);
        let zk_ivm_prove_slots_total =
            zk_ivm_prove_max_inflight.saturating_add(defaults::torii::ZK_IVM_PROVE_MAX_QUEUE);
        let zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(zk_ivm_prove_slots_total));
        let zk_ivm_prove_inflight =
            Arc::new(tokio::sync::Semaphore::new(zk_ivm_prove_max_inflight));
        let zk_ivm_prove_inflight_total = zk_ivm_prove_max_inflight;
        let mcp = iroha_config::parameters::actual::ToriiMcp::default();
        let mcp_rate_per_sec = mcp.rate_per_minute.map(|rate| {
            let per_minute = rate.get();
            let per_sec = per_minute.div_ceil(60);
            per_sec.max(1)
        });
        let mcp_burst = mcp.burst.map(std::num::NonZeroU32::get);
        let mcp_rate_limiter = limits::RateLimiter::new(mcp_rate_per_sec, mcp_burst);
        let mcp_tools = Arc::new(if mcp.enabled {
            mcp::build_tool_specs(&mcp)
        } else {
            Vec::new()
        });

        Arc::new(AppState {
            events,
            kura,
            chain_id: Arc::new(chain_id),
            state: state.clone(),
            kiso,
            query_service: query_handle,
            rate_limiter: limits::RateLimiter::new(None, None),
            tx_rate_limiter: limits::RateLimiter::new(None, None),
            deploy_rate_limiter,
            proof_rate_limiter: limits::RateLimiter::new(None, None),
            proof_egress_limiter: limits::RateLimiter::new_u64(None, None),
            content_request_limiter: limits::RateLimiter::new(None, None),
            content_egress_limiter: limits::RateLimiter::new_u64(None, None),
            proof_limits: routing::ProofApiLimits::default(),
            content_config: content_config_snapshot,
            ws_message_timeout: Duration::from_millis(defaults::torii::WS_MESSAGE_TIMEOUT_MS),
            require_api_token: false,
            api_tokens_set: api_tokens_set.clone(),
            operator_auth,
            operator_signatures,
            soranet_privacy_ingest,
            soranet_privacy_tokens: Arc::new(soranet_privacy_tokens),
            soranet_privacy_allow_nets: Arc::new(soranet_privacy_allow_nets),
            soranet_privacy_rate_limiter,
            allow_nets: Arc::new(vec![]),
            preauth_gate: Arc::new(limits::PreAuthGate::disabled()),
            queue,
            pipeline_status_cache,
            mcp,
            mcp_rate_limiter,
            mcp_tools,
            mcp_dispatch_router: std::sync::RwLock::new(None),
            fee_policy: FeePolicy::Disabled,
            norito_rpc: norito_rpc_cfg,
            high_load_tx_threshold: usize::MAX,
            high_load_stream_tx_threshold: usize::MAX,
            high_load_subscription_tx_threshold: usize::MAX,
            online_peers: peers,
            iso_bridge,
            alias_service,
            telemetry,
            telemetry_profile,
            api_versions: api_version::ApiVersionPolicy::default(),
            zk_prover_keys_dir: defaults::torii::zk_prover_keys_dir(),
            zk_ivm_prove_jobs,
            zk_ivm_prove_inflight,
            zk_ivm_prove_slots,
            zk_ivm_prove_slots_total,
            zk_ivm_prove_inflight_total,
            zk_ivm_prove_job_ttl_ms: defaults::torii::ZK_IVM_PROVE_JOB_TTL_SECS * 1_000,
            zk_ivm_prove_job_max_entries: defaults::torii::ZK_IVM_PROVE_JOB_MAX_ENTRIES,
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            peer_telemetry,
            da_replay_cache,
            da_replay_store,
            da_receipt_log,
            da_receipt_signer,
            da_ingest,
            #[cfg(feature = "app_api")]
            sorafs_cache,
            #[cfg(feature = "app_api")]
            sorafs_node,
            #[cfg(feature = "app_api")]
            sorafs_limits,
            #[cfg(feature = "app_api")]
            por_coordinator: Arc::new(sorafs::PorCoordinator::new()),
            #[cfg(feature = "app_api")]
            sorafs_alias_cache_policy: sorafs_alias_cache,
            #[cfg(feature = "app_api")]
            sorafs_alias_enforcement,
            #[cfg(feature = "app_api")]
            sorafs_admission: None,
            #[cfg(feature = "app_api")]
            sorafs_gateway_config,
            #[cfg(feature = "app_api")]
            sorafs_gateway_policy: None,
            #[cfg(feature = "app_api")]
            sorafs_gateway_denylist: None,
            #[cfg(feature = "app_api")]
            sorafs_gateway_tls_state: None,
            #[cfg(feature = "app_api")]
            sorafs_pin_policy: sorafs_pin_policy.clone(),
            #[cfg(feature = "app_api")]
            sorafs_blinded_resolver: None,
            #[cfg(feature = "app_api")]
            stream_token_issuer,
            #[cfg(feature = "app_api")]
            stream_token_concurrency: sorafs::StreamTokenConcurrencyTracker::default(),
            #[cfg(feature = "app_api")]
            stream_token_quota: sorafs::StreamTokenQuotaTracker::default(),
            #[cfg(feature = "app_api")]
            sorafs_chunk_range_overrides: DashMap::new(),
            #[cfg(feature = "app_api")]
            offline_issuer: None,
            #[cfg(feature = "app_api")]
            uaid_onboarding: None,
            #[cfg(feature = "app_api")]
            sns_registry: Arc::new(sns::Registry::bootstrap_default()),
            #[cfg(feature = "app_api")]
            soracloud_registry: Arc::new(soracloud::Registry::default()),
            rbc_sampling_enabled: false,
            rbc_sampling_store_dir: None,
            rbc_sampling_max_samples: 0,
            rbc_sampling_max_bytes: 0,
            rbc_sampling_daily_budget: 0,
            rbc_sampling_limiter: limits::RateLimiter::new(None, None),
            rbc_sampling_budget: DashMap::new(),
            rbc_sampling_manifest,
            rbc_chain_hash,
            sumeragi: None,
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            p2p: None,
            #[cfg(feature = "connect")]
            connect_bus: crate::connect::Bus::from_config(&connect_cfg),
            #[cfg(feature = "connect")]
            connect_enabled: connect_cfg.enabled,
            #[cfg(feature = "push")]
            push: push_bridge,
            #[cfg(feature = "push")]
            push_rate_limiter: limits::RateLimiter::new(
                push_cfg
                    .rate_per_minute
                    .map(|v| v.get().saturating_add(59) / 60),
                push_cfg.burst.map(std::num::NonZeroU32::get),
            ),
        })
    }

    #[cfg(feature = "telemetry")]
    pub async fn mk_norito_rpc_test_harness(
        cfg: NoritoRpcTransport,
    ) -> (SharedAppState, Arc<iroha_telemetry::metrics::Metrics>) {
        let app = mk_app_state_for_tests_with_options(None, None, Some(cfg), None);
        let metrics = iroha_telemetry::metrics::global_or_default();
        (app, metrics)
    }

    #[tokio::test]
    async fn runtime_handlers_ok_without_token_and_rate_limit() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        // Active ABI versions
        let resp = super::handler_runtime_abi_active(State(app.clone()), headers.clone(), None)
            .await
            .expect("ok");
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let active: crate::runtime::RuntimeAbiActiveResponse =
            norito::json::from_slice(&bytes).expect("decode json");
        assert!(active.active_versions.contains(&1));
        assert_eq!(
            active.default_compile_target,
            *active.active_versions.iter().max().unwrap()
        );

        // ABI hash
        let resp = super::handler_runtime_abi_hash(State(app), headers, None)
            .await
            .expect("ok");
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let hash: crate::runtime::RuntimeAbiHashResponse =
            norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(hash.policy, "V1");
        assert_eq!(hash.abi_hash_hex.len(), 64);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn debug_witness_returns_json_body() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let accept = Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static("application/json"),
        ));

        let resp = super::handler_debug_witness(State(app), headers, accept)
            .await
            .expect("debug witness response");

        assert_eq!(resp.status(), StatusCode::OK);
        let content_type = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .expect("content type header");
        assert_eq!(content_type, "application/json");

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("response body");
        let _parsed: norito::json::Value = norito::json::from_slice(&bytes).expect("valid json");
    }

    #[tokio::test]
    async fn torii_tx_rate_uses_config_and_queue_default() {
        let mut cfg = crate::test_utils::mk_minimal_root_cfg();
        cfg.torii.tx_rate_per_authority_per_sec =
            Some(NonZeroU32::new(123).expect("nonzero tx rate"));
        cfg.torii.tx_burst_per_authority = Some(NonZeroU32::new(456).expect("nonzero tx burst"));
        cfg.torii.api_high_load_tx_threshold = None;

        let (kiso, _child) = KisoHandle::start(cfg.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(IrohaState::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));
        let queue_cfg = iroha_config::parameters::actual::Queue {
            capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
            capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
            transaction_time_to_live: Duration::from_secs(60),
            ..Default::default()
        };
        let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
        let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
        let _ = peers_tx;
        let torii = Torii::new_with_handle(
            ChainId::from("tx-rate-test"),
            kiso,
            cfg.torii.clone(),
            queue,
            tokio::sync::broadcast::channel(1).0,
            LiveQueryStore::start_test(),
            kura,
            state,
            cfg.common.key_pair.clone(),
            OnlinePeersProvider::new(peers_rx),
            None,
            routing::MaybeTelemetry::disabled(),
        );

        assert_eq!(torii.tx_rate_per_authority_per_sec.unwrap().get(), 123);
        assert_eq!(torii.tx_burst_per_authority.unwrap().get(), 456);
        assert_eq!(torii.high_load_tx_threshold, 50);
    }

    #[tokio::test]
    async fn handler_post_transaction_uses_tx_rate_limiter() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            app_mut.high_load_tx_threshold = 0;
            app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        }

        let keypair = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            keypair.public_key().clone(),
        );
        let chain = (*app.chain_id).clone();
        let tx1 = TransactionBuilder::new(chain.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "rate-limit-1".to_string())])
            .sign(keypair.private_key());
        let tx2 = TransactionBuilder::new(chain, authority)
            .with_instructions([Log::new(Level::INFO, "rate-limit-2".to_string())])
            .sign(keypair.private_key());
        let headers = HeaderMap::new();

        let ok = super::handler_post_transaction(
            State(app.clone()),
            headers.clone(),
            NoritoVersioned(tx1),
        )
        .await
        .expect("accepted");
        assert_eq!(ok.into_response().status(), StatusCode::ACCEPTED);

        let err = match super::handler_post_transaction(State(app), headers, NoritoVersioned(tx2))
            .await
        {
            Ok(_) => panic!("expected rate limit"),
            Err(err) => err,
        };
        assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[cfg(feature = "telemetry")]
    fn sample_privacy_event_dto() -> RecordSoranetPrivacyEventDto {
        RecordSoranetPrivacyEventDto {
            event: SoranetPrivacyEventV1 {
                timestamp_unix: 1_720_000_123,
                mode: SoranetPrivacyModeV1::Entry,
                kind: SoranetPrivacyEventKindV1::HandshakeSuccess(
                    SoranetPrivacyEventHandshakeSuccessV1 {
                        rtt_ms: Some(12),
                        active_circuits_after: Some(3),
                    },
                ),
            },
            source: None,
        }
    }

    #[cfg(feature = "telemetry")]
    fn sample_privacy_share_dto() -> RecordSoranetPrivacyShareDto {
        let mut share = SoranetPrivacyPrioShareV1::new(1, 1_720_000_000, 60);
        share.mode = SoranetPrivacyModeV1::Entry;
        share.handshake_accept_share = 5;
        share.active_circuits_sum_share = 30;
        share.active_circuits_sample_share = 5;
        share.active_circuits_max_observed = Some(7);
        share.verified_bytes_share = 1_024;
        RecordSoranetPrivacyShareDto {
            share,
            forwarded_by: None,
        }
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn privacy_ingest_rejects_when_disabled() {
        let app = mk_app_state_for_tests();
        let dto = sample_privacy_event_dto();

        let response = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            HeaderMap::new(),
            axum::extract::ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))),
            NoritoJson(dto),
        )
        .await
        .expect("handler executes");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let metrics = app.telemetry.metrics().await;
        let disabled = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["event", "disabled"])
            .unwrap()
            .get();
        assert!(disabled >= 1, "disabled counter should increment");
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn privacy_ingest_denies_without_allowlist() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut =
                std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
            app_mut.soranet_privacy_ingest.enabled = true;
            app_mut.soranet_privacy_ingest.require_token = true;
            app_mut.soranet_privacy_ingest.tokens = vec!["secret-token".into()];
            app_mut.soranet_privacy_tokens =
                Arc::new(vec!["secret-token".to_string()].into_iter().collect());
            app_mut.soranet_privacy_allow_nets = Arc::new(Vec::new());
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            super::SORANET_PRIVACY_TOKEN_HEADER,
            HeaderValue::from_static("secret-token"),
        );

        let response = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            headers,
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(sample_privacy_event_dto()),
        )
        .await
        .expect("handler executes");

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let metrics = app.telemetry.metrics().await;
        let blocked = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["event", "namespace_blocked"])
            .unwrap()
            .get();
        assert!(blocked >= 1, "namespace block counter should increment");
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn privacy_ingest_enforces_token_namespace_and_rate() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut =
                std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
            app_mut.soranet_privacy_ingest.enabled = true;
            app_mut.soranet_privacy_ingest.require_token = true;
            app_mut.soranet_privacy_ingest.tokens = vec!["secret-token".into()];
            app_mut.soranet_privacy_ingest.allow_cidrs =
                vec!["127.0.0.1/32".to_string(), "::1/128".to_string()];
            app_mut.soranet_privacy_ingest.rate_per_sec =
                Some(std::num::NonZeroU32::new(1).expect("nonzero"));
            app_mut.soranet_privacy_ingest.burst =
                Some(std::num::NonZeroU32::new(1).expect("nonzero"));
            app_mut.soranet_privacy_tokens =
                Arc::new(vec!["secret-token".to_string()].into_iter().collect());
            app_mut.soranet_privacy_allow_nets = Arc::new(crate::limits::parse_cidrs(
                &app_mut.soranet_privacy_ingest.allow_cidrs,
            ));
            app_mut.soranet_privacy_rate_limiter = crate::limits::RateLimiter::new(
                app_mut
                    .soranet_privacy_ingest
                    .rate_per_sec
                    .map(std::num::NonZeroU32::get),
                app_mut
                    .soranet_privacy_ingest
                    .burst
                    .map(std::num::NonZeroU32::get),
            );
        }

        let dto = sample_privacy_event_dto();
        // Missing token
        let resp = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            HeaderMap::new(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(dto.clone()),
        )
        .await
        .expect("handler executes");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let metrics = app.telemetry.metrics().await;
        let missing = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["event", "missing_token"])
            .unwrap()
            .get();
        assert!(missing >= 1);

        // Wrong namespace
        let mut headers = HeaderMap::new();
        headers.insert(
            super::SORANET_PRIVACY_TOKEN_HEADER,
            HeaderValue::from_static("secret-token"),
        );
        let resp = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))),
            NoritoJson(dto.clone()),
        )
        .await
        .expect("handler executes");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Happy path
        let ok_resp = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(dto.clone()),
        )
        .await
        .expect("handler executes");
        assert_eq!(ok_resp.status(), StatusCode::ACCEPTED);

        // Rate limit on second immediate call
        let limited_resp = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            headers,
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(dto),
        )
        .await
        .expect("handler executes");
        assert_eq!(limited_resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn privacy_ingest_denies_without_namespace_allowlist() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut =
                std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
            app_mut.soranet_privacy_ingest.enabled = true;
            app_mut.soranet_privacy_ingest.require_token = false;
            app_mut.soranet_privacy_ingest.allow_cidrs.clear();
            app_mut.soranet_privacy_allow_nets = Arc::new(Vec::new());
        }

        let dto = sample_privacy_event_dto();
        let resp = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            HeaderMap::new(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(dto),
        )
        .await
        .expect("handler executes");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let metrics = app.telemetry.metrics().await;
        let blocked = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["event", "namespace_blocked"])
            .unwrap()
            .get();
        assert!(
            blocked >= 1,
            "namespace rejection counter should increment for missing allow-list"
        );
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn privacy_ingest_blocks_without_allowlist() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut =
                std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
            app_mut.soranet_privacy_ingest.enabled = true;
            app_mut.soranet_privacy_ingest.require_token = true;
            app_mut.soranet_privacy_ingest.tokens = vec!["secret-token".into()];
            app_mut.soranet_privacy_allow_nets = Arc::new(Vec::new());
            app_mut.soranet_privacy_tokens =
                Arc::new(vec!["secret-token".to_string()].into_iter().collect());
            app_mut.soranet_privacy_rate_limiter = crate::limits::RateLimiter::new(None, None);
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            super::SORANET_PRIVACY_TOKEN_HEADER,
            HeaderValue::from_static("secret-token"),
        );
        let resp = super::handler_post_soranet_privacy_event(
            State(app.clone()),
            headers,
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(sample_privacy_event_dto()),
        )
        .await
        .expect("handler executes");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        let metrics = app.telemetry.metrics().await;
        let namespace_blocked = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["event", "namespace_blocked"])
            .unwrap()
            .get();
        assert!(
            namespace_blocked >= 1,
            "namespace reject counter must increment"
        );
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn privacy_share_ingest_enforces_policy() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut =
                std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
            app_mut.soranet_privacy_ingest.enabled = true;
            app_mut.soranet_privacy_ingest.require_token = true;
            app_mut.soranet_privacy_ingest.tokens = vec!["secret-token".into()];
            app_mut.soranet_privacy_ingest.allow_cidrs =
                vec!["127.0.0.1/32".to_string(), "::1/128".to_string()];
            app_mut.soranet_privacy_ingest.rate_per_sec =
                Some(std::num::NonZeroU32::new(1).expect("nonzero"));
            app_mut.soranet_privacy_ingest.burst =
                Some(std::num::NonZeroU32::new(1).expect("nonzero"));
            app_mut.soranet_privacy_tokens =
                Arc::new(vec!["secret-token".to_string()].into_iter().collect());
            app_mut.soranet_privacy_allow_nets = Arc::new(crate::limits::parse_cidrs(
                &app_mut.soranet_privacy_ingest.allow_cidrs,
            ));
            app_mut.soranet_privacy_rate_limiter = crate::limits::RateLimiter::new(
                app_mut
                    .soranet_privacy_ingest
                    .rate_per_sec
                    .map(std::num::NonZeroU32::get),
                app_mut
                    .soranet_privacy_ingest
                    .burst
                    .map(std::num::NonZeroU32::get),
            );
        }

        let share_dto = sample_privacy_share_dto();

        // Missing token -> 401
        let resp = super::handler_post_soranet_privacy_share(
            State(app.clone()),
            HeaderMap::new(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(share_dto.clone()),
        )
        .await
        .expect("handler executes");
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Wrong namespace -> 403
        let mut headers = HeaderMap::new();
        headers.insert(
            super::SORANET_PRIVACY_TOKEN_HEADER,
            HeaderValue::from_static("secret-token"),
        );
        let resp = super::handler_post_soranet_privacy_share(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))),
            NoritoJson(share_dto.clone()),
        )
        .await
        .expect("handler executes");
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // Happy path -> 202
        let ok = super::handler_post_soranet_privacy_share(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(share_dto.clone()),
        )
        .await
        .expect("handler executes");
        assert_eq!(ok.status(), StatusCode::ACCEPTED);

        // Rate limit -> 429
        let limited = super::handler_post_soranet_privacy_share(
            State(app.clone()),
            headers,
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            NoritoJson(share_dto),
        )
        .await
        .expect("handler executes");
        assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);

        let metrics = app.telemetry.metrics().await;
        let missing = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["share", "missing_token"])
            .unwrap()
            .get();
        assert!(missing >= 1);
        let namespace = metrics
            .soranet_privacy_ingest_reject_total
            .get_metric_with_label_values(&["share", "namespace_blocked"])
            .unwrap()
            .get();
        assert!(namespace >= 1);
    }

    #[tokio::test]
    async fn runtime_metrics_and_node_capabilities_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        let metrics_resp =
            super::handler_runtime_metrics(State(app.clone()), headers.clone(), None)
                .await
                .expect("ok");
        assert_eq!(metrics_resp.status(), axum::http::StatusCode::OK);
        let metrics_bytes = axum::body::to_bytes(metrics_resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let metrics: crate::runtime::RuntimeMetricsResponse =
            norito::json::from_slice(&metrics_bytes).expect("decode json");
        assert_eq!(metrics.active_abi_versions_count, 1);

        let caps_resp = super::handler_node_capabilities(State(app), headers, None)
            .await
            .expect("ok");
        assert_eq!(caps_resp.status(), axum::http::StatusCode::OK);
        let caps_bytes = axum::body::to_bytes(caps_resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let caps: crate::runtime::NodeCapabilitiesResponse =
            norito::json::from_slice(&caps_bytes).expect("decode json");
        assert!(caps.supported_abi_versions.contains(&1));
        assert_eq!(caps.default_compile_target, 1);
        assert_eq!(
            caps.data_model_version,
            iroha_data_model::DATA_MODEL_VERSION
        );
        assert!(caps.crypto.sm.acceleration.scalar);
        assert!(
            !caps.crypto.sm.allowed_signing.is_empty(),
            "allowed_signing must advertise at least one algorithm"
        );
    }

    #[tokio::test]
    async fn core_info_handlers_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        // configuration
        let resp = super::handler_get_configuration(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let config_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("config body");
        let config: ConfigGetDTO =
            norito::json::from_slice(&config_bytes).expect("decode config payload");
        assert!(
            !config
                .network
                .soranet_handshake
                .descriptor_commit_hex
                .is_empty(),
            "handshake descriptor should be present in config payload"
        );
        assert!(
            config.network.soranet_handshake.pow.puzzle.is_some(),
            "puzzle gate should be advertised in configuration payload"
        );

        // peers
        let resp = super::handler_peers(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // health
        // For ConnectInfo we can pass a dummy loopback address by constructing the extractor arg manually is not possible here.
        // Instead, rely on non-allowlist path (headers don't carry the internal x-iroha-remote-addr), which doesn't need ConnectInfo IP.
        let resp = super::handler_health(
            State(app),
            headers,
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn time_handlers_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        let resp = super::handler_time_now(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let resp = super::handler_time_status(State(app), headers)
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[cfg(all(feature = "app_api", feature = "push"))]
    fn mk_push_request(token: &str) -> push::RegisterDeviceRequest {
        push::RegisterDeviceRequest {
            account_id: "alice@wonderland".to_string(),
            platform: "FCM".to_string(),
            token: token.to_string(),
            topics: Some(vec!["orders".into()]),
        }
    }

    #[cfg(all(feature = "app_api", feature = "push"))]
    async fn extract_error(resp: AxResponse) -> ErrorEnvelope {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("error body");
        norito::decode_from_bytes(&bytes).expect("decode error envelope")
    }

    #[cfg(all(feature = "app_api", feature = "push"))]
    #[tokio::test]
    async fn push_registration_rejected_when_disabled() {
        let app = mk_app_state_for_tests();

        let resp = super::handler_push_register_device(
            State(app.clone()),
            NoritoJson(mk_push_request("t-disabled")),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let err = extract_error(resp).await;
        assert_eq!(err.code(), "push_disabled");
        assert!(
            app.push.is_none(),
            "push bridge should be absent by default"
        );
    }

    #[cfg(all(feature = "app_api", feature = "push"))]
    #[tokio::test]
    async fn push_registration_requires_credentials() {
        let app = mk_app_state_for_tests_with_options(
            None,
            None,
            None,
            Some(iroha_config::parameters::actual::Push {
                enabled: true,
                ..Default::default()
            }),
        );

        let resp = super::handler_push_register_device(
            State(app.clone()),
            NoritoJson(mk_push_request("t-missing-creds")),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let err = extract_error(resp).await;
        assert_eq!(err.code(), "push_missing_credentials");
        let bridge = app.push.as_ref().expect("push bridge configured");
        assert_eq!(bridge.device_count(), 0);
    }

    #[cfg(all(feature = "app_api", feature = "push"))]
    #[tokio::test]
    async fn push_registration_succeeds_with_credentials() {
        let app = mk_app_state_for_tests_with_options(
            None,
            None,
            None,
            Some(iroha_config::parameters::actual::Push {
                enabled: true,
                fcm_api_key: Some("k".to_string()),
                ..Default::default()
            }),
        );

        let resp = super::handler_push_register_device(
            State(app.clone()),
            NoritoJson(mk_push_request("t-success")),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let bridge = app.push.as_ref().expect("push bridge configured");
        assert_eq!(bridge.device_count(), 1);
    }

    fn make_signed_block(
        height: u64,
        prev_hash: Option<HashOf<BlockHeader>>,
    ) -> (SignedBlock, HashOf<TransactionEntrypoint>) {
        let keypair = KeyPair::random();
        let chain: ChainId = "block-header-tests".parse().expect("chain id");
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            keypair.public_key().clone(),
        );
        let tx = TransactionBuilder::new(chain, authority).sign(keypair.private_key());
        let entry_hash = tx.hash_as_entrypoint();
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero height"),
            prev_hash,
            None,
            None,
            0,
            0,
        );
        let signature = BlockSignature::new(
            0,
            SignatureOf::from_hash(keypair.private_key(), header.hash()),
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        let entry_hashes = [entry_hash];
        block.set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        );
        (block, entry_hash)
    }

    fn store_block(app: &SharedAppState, block: SignedBlock) -> HashOf<BlockHeader> {
        let hash = block.hash();
        app.kura.store_block(Arc::new(block)).expect("store block");
        hash
    }

    #[test]
    fn pipeline_status_merge_prefers_higher_rank() {
        let now = Instant::now();
        let mut entry = PipelineStatusEntry::at_time(PipelineStatusKind::Applied, None, None, now);
        entry.merge_from_event(PipelineStatusEntry::at_time(
            PipelineStatusKind::Expired,
            None,
            None,
            now + Duration::from_secs(1),
        ));
        assert_eq!(entry.kind, PipelineStatusKind::Applied);

        let height = NonZeroU64::new(7).expect("height");
        let rejection = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
        entry.merge_from_event(PipelineStatusEntry::at_time(
            PipelineStatusKind::Rejected,
            Some(height),
            Some(rejection.clone()),
            now + Duration::from_secs(2),
        ));
        assert_eq!(entry.kind, PipelineStatusKind::Rejected);
        assert_eq!(entry.block_height, Some(height));
        assert_eq!(entry.rejection, Some(rejection));
    }

    #[test]
    fn pipeline_status_cache_records_transaction_event() {
        let cache = PipelineStatusCache::new();
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let height = NonZeroU64::new(2).expect("height");
        let event = TransactionEvent {
            hash: tx_hash,
            block_height: Some(height),
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            status: TransactionStatus::Approved,
        };
        cache.record_transaction_event(&event);
        let stored = cache.lookup(&tx_hash).expect("entry");
        assert_eq!(stored.kind, PipelineStatusKind::Approved);
        assert_eq!(stored.block_height, Some(height));
        assert!(stored.rejection.is_none());
    }

    #[tokio::test]
    async fn pipeline_status_cache_records_block_event() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        let tx = block.external_transactions().next().expect("tx");
        let tx_hash = tx.hash();
        store_block(&app, block);
        let event = BlockEvent {
            header,
            status: BlockStatus::Applied,
        };
        app.pipeline_status_cache
            .record_block_event(&event, &app.kura);
        let stored = app.pipeline_status_cache.lookup(&tx_hash).expect("entry");
        assert_eq!(stored.kind, PipelineStatusKind::Applied);
        let height = NonZeroU64::new(1).expect("height");
        assert_eq!(stored.block_height, Some(height));
    }

    #[tokio::test]
    async fn pipeline_status_cache_refreshes_pending_block() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let event = BlockEvent {
            header,
            status: BlockStatus::Committed,
        };
        app.pipeline_status_cache
            .record_block_event(&event, &app.kura);
        assert!(app.pipeline_status_cache.lookup(&tx_hash).is_none());
        store_block(&app, block);
        app.pipeline_status_cache.refresh_pending_blocks(&app.kura);
        let stored = app.pipeline_status_cache.lookup(&tx_hash).expect("entry");
        assert_eq!(stored.kind, PipelineStatusKind::Committed);
    }

    #[test]
    fn pipeline_status_cache_prunes_stale_entries() {
        let cache = PipelineStatusCache::with_limits(10, Duration::from_secs(1));
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        cache.record_entry(
            tx_hash,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
        );
        cache.prune(now);
        assert!(cache.lookup(&tx_hash).is_none());
    }

    #[test]
    fn pipeline_status_cache_eviction_respects_capacity() {
        let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(60));
        let (block_a, _) = make_signed_block(1, None);
        let (block_b, _) = make_signed_block(2, None);
        let hash_a = block_a.external_transactions().next().expect("tx").hash();
        let hash_b = block_b.external_transactions().next().expect("tx").hash();
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        cache.record_entry(
            hash_a,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
        );
        cache.record_entry(
            hash_b,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
        );
        cache.prune(now);
        assert!(cache.lookup(&hash_a).is_none());
        assert!(cache.lookup(&hash_b).is_some());
    }

    #[test]
    fn parse_signed_transaction_hash_rejects_invalid() {
        assert!(parse_signed_transaction_hash("not-a-hash").is_err());
    }

    #[tokio::test]
    async fn pipeline_status_handler_returns_queued() {
        let app = mk_app_state_for_tests();
        let keypair = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            keypair.public_key().clone(),
        );
        let tx = TransactionBuilder::new((*app.chain_id).clone(), authority)
            .with_instructions([Log {
                level: Level::INFO,
                msg: "queued".to_string(),
            }])
            .sign(keypair.private_key());
        let params = app.state.world.view().parameters().clone();
        let max_clock_drift = params.sumeragi().max_clock_drift();
        let tx_limits = params.transaction();
        let crypto_cfg = app.state.crypto();
        let accepted = AcceptedTransaction::accept(
            tx.clone(),
            app.chain_id.as_ref(),
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("accepted");
        app.queue
            .push(accepted, app.state.view())
            .expect("queue push");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            None,
            crate::NoritoQuery(PipelineStatusQuery {
                hash: Some(tx.hash().to_string()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
        let status_kind = payload
            .get("content")
            .and_then(|content| content.get("status"))
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind, Some("Queued"));

        let resp_entry = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            None,
            crate::NoritoQuery(PipelineStatusQuery {
                hash: Some(tx.hash_as_entrypoint().to_string()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp_entry.status(), StatusCode::OK);
        let bytes_entry = axum::body::to_bytes(resp_entry.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload_entry: norito::json::Value =
            norito::json::from_slice(&bytes_entry).expect("json");
        let status_kind_entry = payload_entry
            .get("content")
            .and_then(|content| content.get("status"))
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind_entry, Some("Queued"));
    }

    #[tokio::test]
    async fn pipeline_status_handler_returns_applied_from_state() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        let tx = block.external_transactions().next().expect("tx");
        let tx_hash = tx.hash();
        let tx_entry_hash = tx.hash_as_entrypoint();
        store_block(&app, block);

        let height = header.height();
        let height_usize = usize::try_from(height.get()).expect("height usize");
        let height_nz = NonZeroUsize::new(height_usize).expect("height");
        let mut state_block = app.state.block(header);
        let tx_hashes: HashSet<_> = [tx_hash].into_iter().collect();
        state_block.transactions.insert_block(tx_hashes, height_nz);
        state_block.commit().expect("commit");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            None,
            crate::NoritoQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
        let status_kind = payload
            .get("content")
            .and_then(|content| content.get("status"))
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind, Some("Applied"));

        let resp_entry = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            None,
            crate::NoritoQuery(PipelineStatusQuery {
                hash: Some(tx_entry_hash.to_string()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp_entry.status(), StatusCode::OK);
        let bytes_entry = axum::body::to_bytes(resp_entry.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload_entry: norito::json::Value =
            norito::json::from_slice(&bytes_entry).expect("json");
        let status_kind_entry = payload_entry
            .get("content")
            .and_then(|content| content.get("status"))
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind_entry, Some("Applied"));
    }

    #[tokio::test]
    async fn pipeline_status_handler_encodes_rejection_as_base64() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let reason = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
        app.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Rejected, None, Some(reason.clone())),
        );

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            None,
            crate::NoritoQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
        let rejection_payload = payload
            .get("content")
            .and_then(|content| content.get("status"))
            .and_then(|status| status.get("content"))
            .and_then(norito::json::Value::as_str)
            .expect("rejection content");
        let expected =
            base64::engine::general_purpose::STANDARD.encode(norito::to_bytes(&reason).unwrap());
        assert_eq!(rejection_payload, expected);
    }

    fn sample_commit_qc(
        block_hash: HashOf<BlockHeader>,
        post_state_root: iroha_crypto::Hash,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> Qc {
        let chain_id: ChainId = "chain".parse().expect("chain id");
        let parent_state_root = iroha_crypto::Hash::prehashed([0x11; 32]);
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let vote = Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = vote_preimage(&chain_id, PERMISSIONED_TAG, &vote);
        let signature = Signature::new(keypair.private_key(), &preimage);
        let sig_bytes = signature.payload().to_vec();
        let sig_refs = vec![sig_bytes.as_slice()];
        let aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate signatures");

        let peer_id = PeerId::from(keypair.public_key().clone());
        let validator_set = vec![peer_id];
        Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: aggregate_signature,
            },
        }
    }

    fn record_commit_cert(height: u64) -> Qc {
        let chain_id: ChainId = "chain".parse().expect("chain id");
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::from(keypair.public_key().clone());
        let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([height as u8; 32]));
        let parent_state_root = iroha_crypto::Hash::prehashed([0x22; 32]);
        let post_state_root = iroha_crypto::Hash::prehashed([0x33; 32]);
        let vote = Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view: 0,
            epoch: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = vote_preimage(&chain_id, PERMISSIONED_TAG, &vote);
        let signature = Signature::new(keypair.private_key(), &preimage);
        let cert = Qc {
            phase: Phase::Commit,
            height,
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            view: 0,
            epoch: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&vec![peer_id.clone()]),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: vec![peer_id],
            aggregate: QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: signature.payload().to_vec(),
            },
        };
        record_commit_qc(cert.clone());
        cert
    }

    #[tokio::test]
    async fn ledger_headers_respect_from_and_limit() {
        let app = mk_app_state_for_tests();
        let (block1, _) = make_signed_block(1, None);
        let first_hash = store_block(&app, block1);
        let (block2, _) = make_signed_block(2, Some(first_hash));
        store_block(&app, block2);

        let resp = super::handler_ledger_headers(
            State(app.clone()),
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(2),
                limit: Some(1),
            }),
            HeaderMap::new(),
        )
        .await
        .expect("ok");
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .map(HeaderValue::as_bytes),
            Some(b"application/json".as_slice())
        );
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        let headers: Vec<BlockHeader> = norito::json::from_slice(&bytes).expect("json decode");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0].height().get(), 2);

        let norito_resp = super::handler_ledger_headers(
            State(app),
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(2),
                limit: Some(2),
            }),
            {
                let mut headers = HeaderMap::new();
                headers.insert(
                    axum::http::header::ACCEPT,
                    HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
                );
                headers
            },
        )
        .await
        .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
        let archived = norito::from_bytes::<Vec<BlockHeader>>(&norito_bytes).expect("archive");
        let decoded: Vec<BlockHeader> = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].height().get(), 2);
        assert_eq!(decoded[1].height().get(), 1);
    }

    #[tokio::test]
    async fn commit_qc_window_clamped() {
        let high = 10_000;
        let latest = record_commit_cert(high + 1);
        let older = record_commit_cert(high);

        let resp = handle_v1_sumeragi_commit_qcs(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(1),
            }),
            None,
        )
        .await
        .expect("ok");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let certs: Vec<Qc> = norito::json::from_slice(&bytes).expect("decode certs json");
        assert_eq!(certs.len(), 1);
        assert_eq!(certs[0].height, latest.height);

        let norito_resp = handle_v1_sumeragi_commit_qcs(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(2),
            }),
            Some(HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE)),
        )
        .await
        .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let archived = norito::from_bytes::<Vec<Qc>>(&norito_bytes).expect("arch");
        let decoded: Vec<Qc> = norito::core::NoritoDeserialize::deserialize(archived);
        assert!(decoded.iter().any(|c| c.height == latest.height));
        assert!(decoded.iter().any(|c| c.height == older.height));
    }

    #[tokio::test]
    async fn validator_set_history_returns_snapshots() {
        let high = 5;
        let latest = record_commit_cert(high + 1);
        let older = record_commit_cert(high);

        let resp = routing::handle_v1_sumeragi_validator_sets(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(2),
            }),
            None,
        )
        .await
        .expect("ok");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("json");
        let sets: Vec<routing::ValidatorSetSnapshot> =
            norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].height, latest.height);
        assert_eq!(sets[1].height, older.height);

        let norito_resp = routing::handle_v1_sumeragi_validator_sets(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(1),
            }),
            Some(HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE)),
        )
        .await
        .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let archived =
            norito::from_bytes::<Vec<routing::ValidatorSetSnapshot>>(&norito_bytes).expect("arch");
        let decoded: Vec<routing::ValidatorSetSnapshot> =
            norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].height, latest.height);
        assert_eq!(decoded[0].validator_set_hash, latest.validator_set_hash);
    }

    #[tokio::test]
    async fn validator_set_by_height_returns_exact_match() {
        let high = 20;
        record_commit_cert(high);
        let wanted = record_commit_cert(high + 1);

        let resp = routing::handle_v1_sumeragi_validator_set_by_height(
            axum::extract::Path(high + 1),
            Some(HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE)),
        )
        .await
        .expect("ok");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
        let archived =
            norito::from_bytes::<routing::ValidatorSetSnapshot>(&bytes).expect("archive");
        let decoded: routing::ValidatorSetSnapshot =
            norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.height, wanted.height);
        assert_eq!(decoded.block_hash, wanted.subject_block_hash);
    }

    #[tokio::test]
    async fn ledger_state_root_uses_result_merkle_root_when_no_commit_qc() {
        let app = mk_app_state_for_tests();
        let (mut block, _) = make_signed_block(1, None);
        let entry_hashes = [block
            .payload()
            .transactions
            .first()
            .expect("tx")
            .hash_as_entrypoint()];
        block.set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        );
        let result_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("result root");
        let block_hash = block.hash();
        store_block(&app, block);

        let resp =
            handler_ledger_state_root(State(app.clone()), axum::extract::Path(1), HeaderMap::new())
                .await
                .expect("ok");
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .map(HeaderValue::as_bytes),
            Some(b"application/json".as_slice())
        );
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("json body");
        let payload: StateRootResponse = norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(payload.height, 1);
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.state_root, result_root);
        assert_eq!(payload.source, "result_merkle_root");

        let mut accept = HeaderMap::new();
        accept.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        );
        let norito_resp = handler_ledger_state_root(State(app), axum::extract::Path(1), accept)
            .await
            .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
        let archived = norito::from_bytes::<StateRootResponse>(&norito_bytes).expect("archive");
        let decoded: StateRootResponse = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.state_root, result_root);
        assert_eq!(decoded.source, "result_merkle_root");
    }

    #[tokio::test]
    async fn ledger_state_proof_returns_commit_qc() {
        let app = mk_app_state_for_tests();
        let (mut block, _) = make_signed_block(1, None);
        let entry_hashes = [block
            .payload()
            .transactions
            .first()
            .expect("tx")
            .hash_as_entrypoint()];
        block.set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        );
        let expected_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("result root");
        let block_hash = block.hash();
        store_block(&app, block);

        let qc = sample_commit_qc(block_hash, expected_root, 1, 2, 0);
        let mut app = app;
        let app_mut = Arc::get_mut(&mut app).expect("unique app state for test");
        Arc::get_mut(&mut app_mut.state)
            .expect("unique core state for test")
            .insert_commit_qc_for_testing(block_hash, qc.clone());

        let resp = handler_ledger_state_proof(
            State(app.clone()),
            axum::extract::Path(1),
            HeaderMap::new(),
        )
        .await
        .expect("ok");
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let proof: StateProofResponse = norito::json::from_slice(&body).expect("json decode");
        assert_eq!(proof.height, 1);
        assert_eq!(proof.block_hash, block_hash);
        assert_eq!(proof.state_root, expected_root);
        assert_eq!(proof.commit_qc.subject_block_hash, block_hash);
        assert_eq!(proof.commit_qc.post_state_root, expected_root);
        assert_eq!(
            proof.commit_qc.aggregate.signers_bitmap,
            qc.aggregate.signers_bitmap
        );
        assert_eq!(
            proof.commit_qc.aggregate.bls_aggregate_signature,
            qc.aggregate.bls_aggregate_signature
        );

        let mut accept = HeaderMap::new();
        accept.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        );
        let norito_resp = handler_ledger_state_proof(State(app), axum::extract::Path(1), accept)
            .await
            .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let archived = norito::from_bytes::<StateProofResponse>(&norito_bytes).expect("arch");
        let decoded: StateProofResponse = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.commit_qc.post_state_root, expected_root);
    }

    #[tokio::test]
    async fn state_proof_http_roundtrip_supports_json_and_norito() {
        use axum::{
            Router,
            body::Body,
            http::{Request, StatusCode},
            routing::get,
        };
        use http_body_util::BodyExt as _;
        use tower::ServiceExt as _;

        let app = mk_app_state_for_tests();
        let (mut block, _) = make_signed_block(1, None);
        let entry_hashes = [block
            .payload()
            .transactions
            .first()
            .expect("tx")
            .hash_as_entrypoint()];
        block.set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        );
        let expected_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("result root");
        let block_hash = block.hash();
        store_block(&app, block);

        let qc = sample_commit_qc(block_hash, expected_root, 1, 2, 0);
        let mut app = Arc::into_inner(app).unwrap_or_else(|| panic!("unique app state for test"));
        let mut state =
            Arc::into_inner(app.state).unwrap_or_else(|| panic!("unique core state for test"));
        state.insert_commit_qc_for_testing(block_hash, qc.clone());
        app.state = Arc::new(state);
        let app: SharedAppState = Arc::new(app);

        let router = Router::new()
            .route(uri::LEDGER_STATE_PROOF, get(handler_ledger_state_proof))
            .with_state(app.clone());

        let request = Request::builder()
            .uri("/v1/ledger/state-proof/1")
            .body(Body::empty())
            .expect("request");
        let response = router.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let proof: StateProofResponse = norito::json::from_slice(&bytes).expect("json decode");
        assert_eq!(proof.height, 1);
        assert_eq!(proof.block_hash, block_hash);
        assert_eq!(proof.state_root, expected_root);
        assert_eq!(
            proof.commit_qc.aggregate.signers_bitmap,
            qc.aggregate.signers_bitmap
        );
        assert_eq!(
            proof.commit_qc.aggregate.bls_aggregate_signature,
            qc.aggregate.bls_aggregate_signature
        );

        let request = Request::builder()
            .uri("/v1/ledger/state-proof/1")
            .header(axum::http::header::ACCEPT, crate::utils::NORITO_MIME_TYPE)
            .body(Body::empty())
            .expect("request");
        let response = router.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let archived =
            norito::from_bytes::<StateProofResponse>(&bytes).expect("archived state proof");
        let proof: StateProofResponse = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(proof.height, 1);
        assert_eq!(proof.block_hash, block_hash);
        assert_eq!(proof.state_root, expected_root);
        assert_eq!(proof.commit_qc.post_state_root, expected_root);
    }

    #[tokio::test]
    async fn block_proof_handler_emits_norito() {
        let app = mk_app_state_for_tests();
        let (block, entry_hash) = make_signed_block(1, None);
        store_block(&app, block);
        let entry_hex = hex::encode(entry_hash.as_ref());

        let resp = super::handler_block_proof(State(app), axum::extract::Path((1, entry_hex)))
            .await
            .expect("ok")
            .into_response();
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .map(HeaderValue::as_bytes),
            Some(crate::utils::NORITO_MIME_TYPE.as_bytes())
        );
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("norito payload");
        let archived = norito::from_bytes::<BlockProofs>(&bytes).expect("archive decode");
        let proofs: BlockProofs = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(proofs.block_height.get(), 1);
        assert_eq!(proofs.entry_hash, entry_hash);
    }

    fn empty_manifest() -> iroha_data_model::smart_contract::manifest::ContractManifest {
        iroha_data_model::smart_contract::manifest::ContractManifest {
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            kotoba: None,
            provenance: None,
        }
    }

    fn clone_private_key(
        src: &iroha_data_model::prelude::ExposedPrivateKey,
    ) -> iroha_data_model::prelude::ExposedPrivateKey {
        iroha_data_model::prelude::ExposedPrivateKey(src.0.clone())
    }

    #[tokio::test]
    async fn contract_code_rate_limit_throttles_after_burst() {
        let mut app = mk_app_state_for_tests_with_options(None, Some((1, 1)), None, None);
        let app_inner = Arc::get_mut(&mut app).expect("unique app state");
        app_inner.fee_policy = FeePolicy::Manual {
            asset_id: "xor#wonderland".to_string(),
            amount: 1,
            receiver: "admin@wonderland".to_string(),
        };

        let headers = HeaderMap::new();
        let creds = crate::test_utils::random_authority();

        let dto1 = RegisterContractCodeDto {
            authority: creds.account.clone(),
            private_key: clone_private_key(&creds.private_key),
            manifest: empty_manifest(),
        };
        let dto2 = RegisterContractCodeDto {
            authority: creds.account.clone(),
            private_key: clone_private_key(&creds.private_key),
            manifest: empty_manifest(),
        };

        let first = super::handler_post_contract_code(
            State(app.clone()),
            headers.clone(),
            NoritoJson(dto1),
        )
        .await;
        assert!(first.is_ok(), "first request should pass: {first:?}");

        let second =
            super::handler_post_contract_code(State(app.clone()), headers, NoritoJson(dto2)).await;
        assert!(matches!(
            second,
            Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
            )))
        ));
    }

    #[tokio::test]
    async fn contract_deploy_rate_limit_throttles_after_burst() {
        let app = mk_app_state_for_tests_with_options(None, Some((1, 1)), None, None);
        let headers = HeaderMap::new();
        let creds = crate::test_utils::random_authority();
        let program = crate::test_utils::minimal_ivm_program(1);
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&program);

        let dto1 = DeployContractDto {
            authority: creds.account.clone(),
            private_key: clone_private_key(&creds.private_key),
            code_b64: code_b64.clone(),
        };
        let dto2 = DeployContractDto {
            authority: creds.account.clone(),
            private_key: clone_private_key(&creds.private_key),
            code_b64,
        };

        let first = super::handler_post_contract_deploy(
            State(app.clone()),
            headers.clone(),
            NoritoJson(dto1),
        )
        .await;
        assert!(first.is_ok(), "first deploy should pass: {first:?}");

        let second =
            super::handler_post_contract_deploy(State(app.clone()), headers, NoritoJson(dto2))
                .await;
        assert!(matches!(
            second,
            Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
            )))
        ));
    }

    #[tokio::test]
    async fn contract_activate_rate_limit_throttles_after_burst() {
        let app = mk_app_state_for_tests_with_options(None, Some((1, 1)), None, None);
        let headers = HeaderMap::new();
        let creds = crate::test_utils::random_authority();
        let code_hash = "ab".repeat(32);

        let dto1 = ActivateInstanceDto {
            authority: creds.account.clone(),
            private_key: clone_private_key(&creds.private_key),
            namespace: "ns".into(),
            contract_id: "contract".into(),
            code_hash: code_hash.clone(),
        };
        let dto2 = ActivateInstanceDto {
            authority: creds.account.clone(),
            private_key: clone_private_key(&creds.private_key),
            namespace: "ns".into(),
            contract_id: "contract".into(),
            code_hash,
        };

        let first = super::handler_post_contract_instance_activate(
            State(app.clone()),
            headers.clone(),
            NoritoJson(dto1),
        )
        .await;
        assert!(first.is_ok(), "first activate should pass: {first:?}");

        let second = super::handler_post_contract_instance_activate(
            State(app.clone()),
            headers,
            NoritoJson(dto2),
        )
        .await;
        assert!(matches!(
            second,
            Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
            )))
        ));
    }

    #[tokio::test]
    async fn openapi_enforces_token_policy() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app state");
            state.require_api_token = true;
            let mut tokens = HashSet::new();
            tokens.insert("token-123".to_owned());
            state.api_tokens_set = Arc::new(tokens);
        }

        let headers = HeaderMap::new();
        let missing = super::handler_openapi(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await;
        assert!(matches!(
            missing,
            Err(Error::Query(
                iroha_data_model::ValidationFail::NotPermitted(_)
            ))
        ));

        let mut headers_with_token = HeaderMap::new();
        headers_with_token.insert("x-api-token", HeaderValue::from_static("token-123"));
        let ok = super::handler_openapi(
            State(app),
            headers_with_token,
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await
        .expect("token accepted");
        assert_eq!(ok.status(), axum::http::StatusCode::OK);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn soracloud_status_handler_returns_snapshot_sections() {
        let app = mk_app_state_for_tests();
        let response = super::handler_soracloud_status(State(app), HeaderMap::new(), None)
            .await
            .expect("soracloud status should succeed");
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("collect body");
        let payload: norito::json::Value =
            norito::json::from_slice(&body).expect("decode JSON response");

        assert_eq!(
            payload
                .get("schema_version")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert!(
            payload
                .get("service_health")
                .and_then(norito::json::Value::as_object)
                .is_some(),
            "service_health section should be present"
        );
        assert!(
            payload
                .get("routing")
                .and_then(norito::json::Value::as_object)
                .is_some(),
            "routing section should be present"
        );
        assert!(
            payload
                .get("resource_pressure")
                .and_then(norito::json::Value::as_object)
                .is_some(),
            "resource_pressure section should be present"
        );
        assert!(
            payload
                .get("failed_admissions")
                .and_then(norito::json::Value::as_object)
                .is_some(),
            "failed_admissions section should be present"
        );
        #[cfg(feature = "app_api")]
        assert!(
            payload
                .get("control_plane")
                .and_then(norito::json::Value::as_object)
                .is_some(),
            "control_plane section should be present"
        );
    }

    #[tokio::test]
    async fn proof_rate_limit_sets_retry_after_header() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app state");
            state.proof_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
            state.proof_limits = routing::ProofApiLimits::new(
                10,
                std::time::Duration::from_secs(1),
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(3),
                iroha_config::parameters::defaults::torii::PROOF_MAX_BODY_BYTES.get(),
            );
        }

        let headers = HeaderMap::new();
        let q = crate::NoritoQuery(routing::ProofListQuery {
            limit: Some(1),
            ..Default::default()
        });

        let first = super::handler_list_proofs(
            State(app.clone()),
            negotiated(&app),
            headers.clone(),
            q.clone(),
        )
        .await
        .expect("first request allowed")
        .into_response();
        assert_eq!(first.status(), axum::http::StatusCode::OK);

        let resp = match super::handler_list_proofs(
            State(app.clone()),
            negotiated(&app),
            headers,
            q,
        )
        .await
        {
            Ok(_) => panic!("second request should be throttled"),
            Err(err) => err.into_response(),
        };
        assert_eq!(resp.status(), axum::http::StatusCode::TOO_MANY_REQUESTS);
        let retry_after = resp.headers().get(axum::http::header::RETRY_AFTER);
        assert_eq!(retry_after.and_then(|h| h.to_str().ok()), Some("3"));
    }

    #[cfg(feature = "profiling")]
    #[tokio::test]
    async fn profiling_enforces_token_policy() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app state");
            state.require_api_token = true;
            let mut tokens = HashSet::new();
            tokens.insert("token-456".to_owned());
            state.api_tokens_set = Arc::new(tokens);
        }

        let params: routing::profiling::ProfileParams =
            norito::json::from_value(norito::json!({})).expect("defaults");

        let headers = HeaderMap::new();
        let missing = super::handler_profile(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            AxQuery(params),
        )
        .await;
        assert!(matches!(
            missing,
            Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
            )))
        ));

        let mut headers_with_token = HeaderMap::new();
        headers_with_token.insert("x-api-token", HeaderValue::from_static("token-456"));
        let ok = super::handler_profile(
            State(app),
            headers_with_token,
            axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
            AxQuery(norito::json::from_value(norito::json!({})).expect("defaults for second run")),
        )
        .await
        .expect("token accepted");
        assert!(!ok.is_empty());
    }

    #[cfg(feature = "schema")]
    #[tokio::test]
    async fn schema_handler_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let resp = super::handler_schema(
            State(app),
            headers,
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn telemetry_handlers_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        // Status root
        let resp = super::handler_status_root(
            State(app.clone()),
            headers.clone(),
            None,
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Status tail
        let resp = super::handler_status_tail(
            State(app.clone()),
            headers.clone(),
            None,
            axum::extract::Path("peers".to_string()),
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Metrics
        let text = super::handler_metrics(
            State(app.clone()),
            headers.clone(),
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
        )
        .await
        .expect("ok");
        assert!(!text.is_empty());

        // RBC endpoints (status, sessions, delivered)
        let resp = super::handler_rbc_status(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let resp = super::handler_rbc_sessions(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let resp = super::handler_rbc_delivered_height_view(
            State(app.clone()),
            headers.clone(),
            axum::extract::Path((0u64, 0u64)),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let resp = super::handler_sumeragi_phases(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // QC and leader endpoints
        let resp = super::handler_sumeragi_qc(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        app.state.metrics().set_axt_proof_cache_state(
            iroha_data_model::nexus::DataSpaceId::new(1),
            "miss",
            [0x11; 32],
            2,
            Some(5),
        );
        app.state.metrics().set_axt_reject_hint(
            DataSpaceId::new(1),
            LaneId::new(7),
            10,
            2,
            AxtRejectReason::HandleEra,
        );
        let resp = super::handler_debug_axt_cache(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body = String::from_utf8(body.to_vec()).expect("utf8 body");
        let body_json: norito::json::Value =
            norito::json::from_str(&body).expect("cache status json");
        let entries = body_json
            .get("entries")
            .and_then(norito::json::Value::as_array)
            .expect("entries array");
        assert!(
            entries.iter().any(|entry| {
                entry.get("status").and_then(norito::json::Value::as_str) == Some("miss")
            }),
            "cache status response should include miss entry"
        );
        assert!(
            body_json
                .get("snapshot_version")
                .and_then(norito::json::Value::as_u64)
                .is_some(),
            "cache status response should include snapshot version"
        );
        let hints = body_json
            .get("reject_hints")
            .and_then(norito::json::Value::as_array)
            .expect("reject_hints array");
        assert_eq!(hints.len(), 1, "reject hints should include dsid=1");
        let hint = &hints[0];
        assert_eq!(
            hint.get("dataspace").and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            hint.get("target_lane")
                .and_then(norito::json::Value::as_u64),
            Some(7)
        );
        assert_eq!(
            hint.get("next_min_handle_era")
                .and_then(norito::json::Value::as_u64),
            Some(10)
        );
        assert_eq!(
            hint.get("next_min_sub_nonce")
                .and_then(norito::json::Value::as_u64),
            Some(2)
        );
        assert_eq!(
            hint.get("reason").and_then(norito::json::Value::as_str),
            Some(AxtRejectReason::HandleEra.label())
        );

        let resp = super::handler_sumeragi_leader(State(app), headers)
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn telemetry_new_view_json_and_collectors_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        let resp = super::handler_new_view_json(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let resp = super::handler_sumeragi_collectors(State(app), headers)
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn telemetry_commit_qc_null_on_empty() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let sample_hash = format!("{}", iroha_crypto::Hash::new(b"torii-telemetry-test"));

        let resp = super::handler_commit_qc(
            State(app.clone()),
            headers,
            None,
            axum::extract::Path(sample_hash),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
        assert!(v.get("subject_block_hash").is_some());
        assert!(v.get("commit_qc").is_some());
        let resp = super::handler_commit_qc(
            State(app),
            HeaderMap::new(),
            Some(crate::utils::extractors::ExtractAccept(
                HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
            )),
            axum::extract::Path(format!(
                "{}",
                iroha_crypto::Hash::new(b"torii-telemetry-test")
            )),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        let decoded_opt: Option<Qc> =
            norito::decode_from_bytes(&bytes).expect("decode commit_qc norito");
        assert!(decoded_opt.is_none());
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn telemetry_params_and_bls_keys_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        let resp = super::handler_sumeragi_params(State(app.clone()), headers.clone())
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        let resp = super::handler_sumeragi_bls_keys(State(app), headers)
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn app_api_vk_and_proofs_lists_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        // List VKs
        let resp = super::handler_list_vk(
            State(app.clone()),
            headers.clone(),
            AxQuery(crate::routing::VkListQuery::default()),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // List proofs
        let resp = super::handler_list_proofs(
            State(app.clone()),
            negotiated(&app),
            headers.clone(),
            AxQuery(crate::routing::ProofListQuery::default()),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);

        // Count proofs
        let resp = super::handler_count_proofs(
            State(app.clone()),
            negotiated(&app),
            headers,
            AxQuery(crate::routing::ProofListQuery::default()),
        )
        .await
        .expect("ok")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[tokio::test]
    async fn app_api_get_by_id_not_found_returns_404() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        // Contract code by hash (non-existent)
        let resp = super::handler_get_contract_code(
            State(app.clone()),
            headers.clone(),
            axum::extract::Path(
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            ),
        )
        .await
        .expect("ok mapping")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);

        // VK by backend/name (non-existent)
        let resp = super::handler_get_vk_by_backend_name(
            State(app.clone()),
            headers.clone(),
            axum::extract::Path(("halo2/ipa".to_string(), "demo".to_string())),
        )
        .await
        .expect("ok mapping")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);

        // Proof by backend/hash (non-existent)
        let resp = super::handler_get_proof_by_backend_hash(
            State(app.clone()),
            negotiated(&app),
            headers,
            axum::extract::Path((
                "halo2/ipa".to_string(),
                "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            )),
        )
        .await
        .expect("ok mapping")
        .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn contracts_deploy_handler_ok() {
        // Helper to compose a minimal IVM program (.to bytes) with ABI v1
        fn minimal_ivm_program(abi_version: u8) -> Vec<u8> {
            let mut code = Vec::new();
            code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
            let meta = ivm::ProgramMetadata {
                version_major: 1,
                version_minor: 0,
                mode: 0,
                vector_length: 0,
                max_cycles: 0,
                abi_version,
            };
            let mut out = meta.encode();
            out.extend_from_slice(&code);
            out
        }

        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        let prog = minimal_ivm_program(1);
        let code_b64 = base64::engine::general_purpose::STANDARD.encode(&prog);
        let kp = iroha_crypto::KeyPair::random();
        let authority: iroha_data_model::account::AccountId =
            iroha_data_model::account::AccountId::of(
                "wonderland".parse().unwrap(),
                kp.public_key().clone(),
            );
        let dto = crate::routing::DeployContractDto {
            authority,
            private_key: iroha_data_model::prelude::ExposedPrivateKey(kp.private_key().clone()),
            code_b64,
        };
        let resp = super::handler_post_contract_deploy(State(app), headers, NoritoJson(dto))
            .await
            .expect("ok")
            .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let body = http_body_util::BodyExt::collect(resp.into_body())
            .await
            .unwrap()
            .to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&body).unwrap();
        assert_eq!(
            v.get("ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            v.get("code_hash_hex")
                .and_then(norito::json::Value::as_str)
                .map(str::len),
            Some(64)
        );
        assert_eq!(
            v.get("abi_hash_hex")
                .and_then(norito::json::Value::as_str)
                .map(str::len),
            Some(64)
        );
    }

    #[tokio::test]
    async fn contracts_instance_activate_handler_ok() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        let kp = iroha_crypto::KeyPair::random();
        let authority: iroha_data_model::account::AccountId =
            iroha_data_model::account::AccountId::of(
                "wonderland".parse().unwrap(),
                kp.public_key().clone(),
            );
        let dto = crate::routing::ActivateInstanceDto {
            authority,
            private_key: iroha_data_model::prelude::ExposedPrivateKey(kp.private_key().clone()),
            namespace: "ns.demo".to_string(),
            contract_id: "demo.contract".to_string(),
            code_hash: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
        };
        let resp =
            super::handler_post_contract_instance_activate(State(app), headers, NoritoJson(dto))
                .await
                .expect("ok")
                .into_response();
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
    }

    #[test]
    fn iso_error_mapping_returns_expected_variants() {
        use iroha_data_model::ValidationFail::{InternalError, NotPermitted};

        assert!(matches!(
            super::map_iso_error(MsgError::InvalidFormat),
            NotPermitted(_)
        ));
        assert!(matches!(
            super::map_iso_error(MsgError::Io(std::io::Error::from_raw_os_error(1))),
            InternalError(_)
        ));
    }

    #[test]
    fn push_into_queue_error_sets_backpressure_headers() {
        use nonzero_ext::nonzero;

        let err = super::Error::PushIntoQueue {
            source: Box::new(queue::Error::Full),
            backpressure: queue::BackpressureState::Saturated {
                queued: 5,
                capacity: nonzero!(5_usize),
            },
        };

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("X-Iroha-Queue-State")
                .map(|v| v.to_str().unwrap()),
            Some("saturated")
        );
        assert_eq!(
            headers.get("Retry-After").map(|v| v.to_str().unwrap()),
            Some("1")
        );
    }

    #[test]
    fn push_into_queue_error_sets_reject_code_header() {
        use nonzero_ext::nonzero;

        let err = super::Error::PushIntoQueue {
            source: Box::new(queue::Error::InBlockchain),
            backpressure: queue::BackpressureState::Healthy {
                queued: 0,
                capacity: nonzero!(1_usize),
            },
        };

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|v| v.to_str().ok()),
            Some("PRTRY:ALREADY_COMMITTED")
        );
    }

    #[tokio::test]
    async fn serialization_error_emits_redacted_norito_payload() {
        let err = super::Error::SerializationFailure {
            context: "unit_test",
            source: norito::json::Error::Message("boom".into()),
        };

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(|value| value.to_str().unwrap())
            .expect("content-type header present");
        assert_eq!(content_type, super::utils::NORITO_MIME_TYPE);

        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let payload = norito::decode_from_bytes::<super::ErrorEnvelope>(&body).unwrap();
        assert_eq!(payload.code(), "serialization_error");
        assert_eq!(payload.message(), "failed to serialize unit_test payload");
        assert!(
            !payload.message().contains("boom"),
            "redacted message leaked internal context"
        );
    }

    #[test]
    fn accept_transaction_signature_failure_sets_code_and_header() {
        let kp = iroha_crypto::KeyPair::random();
        let chain: ChainId = "chain".parse().unwrap();
        let authority: AccountId =
            AccountId::of("wonderland".parse().unwrap(), kp.public_key().clone());
        let tx = TransactionBuilder::new(chain, authority).sign(kp.private_key());

        let fail = iroha_core::tx::SignatureVerificationFail::new(
            tx.signature().clone(),
            iroha_core::tx::SignatureRejectionCode::UnsupportedAuthority,
            iroha_data_model::transaction::signed::MULTISIG_SIGNING_UNSUPPORTED_REASON,
        );
        let err = super::Error::AcceptTransaction(
            iroha_core::tx::AcceptTransactionFail::SignatureVerification(fail),
        );

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("x-iroha-reject-code")
                .and_then(|v| v.to_str().ok()),
            Some(iroha_core::tx::SignatureRejectionCode::UnsupportedAuthority.as_str())
        );

        let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
            .expect("collect body")
            .to_bytes();
        let envelope = norito::decode_from_bytes::<super::ErrorEnvelope>(&body)
            .expect("decode error envelope");
        assert_eq!(
            envelope.code(),
            iroha_core::tx::SignatureRejectionCode::UnsupportedAuthority.as_str()
        );
        assert!(envelope.message().contains("failed to accept transaction"));
    }

    #[test]
    fn accept_transaction_limit_failure_sets_header_code() {
        let err = super::Error::AcceptTransaction(
            iroha_core::tx::AcceptTransactionFail::TransactionLimit(
                iroha_data_model::transaction::error::TransactionLimitError {
                    reason: "too big".into(),
                },
            ),
        );

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("x-iroha-reject-code")
                .and_then(|v| v.to_str().ok()),
            Some("transaction_rejected")
        );

        let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
            .expect("collect body")
            .to_bytes();
        let envelope = norito::decode_from_bytes::<super::ErrorEnvelope>(&body)
            .expect("decode error envelope");
        assert_eq!(envelope.code(), "transaction_rejected");
        assert!(envelope.message().contains("too big"));
    }

    #[test]
    fn accept_transaction_nts_unhealthy_sets_header_code() {
        let err = super::Error::AcceptTransaction(
            iroha_core::tx::AcceptTransactionFail::NetworkTimeUnhealthy {
                reason: "fallback".to_owned(),
            },
        );

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("x-iroha-reject-code")
                .and_then(|v| v.to_str().ok()),
            Some("PRTRY:NTS_UNHEALTHY")
        );

        let body = executor::block_on(http_body_util::BodyExt::collect(response.into_body()))
            .expect("collect body")
            .to_bytes();
        let envelope = norito::decode_from_bytes::<super::ErrorEnvelope>(&body)
            .expect("decode error envelope");
        assert_eq!(envelope.code(), "PRTRY:NTS_UNHEALTHY");
        assert!(
            envelope
                .message()
                .contains("Network time service is unhealthy")
        );
    }

    #[test]
    fn offline_reason_query_error_sets_reject_code_header() {
        use iroha_data_model::{
            offline::OFFLINE_REJECTION_REASON_PREFIX, query::error::QueryExecutionFail,
        };

        let message =
            format!("{OFFLINE_REJECTION_REASON_PREFIX}certificate_expired:certificate expired");
        let err = super::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            QueryExecutionFail::Conversion(message),
        ));
        let response = err.into_response();
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|v| v.to_str().ok()),
            Some("certificate_expired")
        );
    }
}

#[cfg(test)]
mod tests_queue_metadata {
    use super::*;

    #[test]
    fn queue_errors_map_to_reason_codes() {
        let cases = [
            (
                queue::Error::Full,
                "PRTRY:QUEUE_FULL",
                "transaction queue is at capacity",
            ),
            (
                queue::Error::MaximumTransactionsPerUser,
                "PRTRY:QUEUE_RATE",
                "authority reached per-user queue capacity",
            ),
            (
                queue::Error::Expired,
                "ED07",
                "transaction expired before admission",
            ),
            (
                queue::Error::InBlockchain,
                "PRTRY:ALREADY_COMMITTED",
                "transaction already committed to the blockchain",
            ),
            (
                queue::Error::IsInQueue,
                "PRTRY:ALREADY_ENQUEUED",
                "transaction already present in the queue",
            ),
        ];

        for (error, expected_code, expected_detail) in cases {
            // array copy, pattern moves
            let (code, detail) = queue_rejection_metadata(&error);
            assert_eq!(code, expected_code);
            assert_eq!(detail, expected_detail);
        }
    }
}

#[cfg(all(test, feature = "app_api"))]
pub(crate) use tests_runtime_handlers::mk_app_state_for_tests;

impl Error {
    fn into_envelope(self) -> ErrorEnvelope {
        match self {
            Self::Query(err) => {
                iroha_logger::error!(
                    ?err,
                    "Query error reached envelope renderer; expected to be handled earlier"
                );
                ErrorEnvelope::new("query_error", err.to_string())
            }
            Self::ApiVersion(err) => ErrorEnvelope::new(err.code(), err.message()),
            Self::AcceptTransaction(err) => {
                iroha_logger::warn!(?err, "Transaction rejected during admission");
                let (code, detail) = accept_transaction_metadata(&err);
                ErrorEnvelope::new(code, detail)
            }
            Self::Config(err) => {
                iroha_logger::error!(?err, "Failed to process configuration request");
                ErrorEnvelope::new("config_error", "configuration request failed")
            }
            Self::SerializationFailure { context, source } => {
                iroha_logger::error!(
                    %context,
                    ?source,
                    "Failed to serialize Torii response payload"
                );
                ErrorEnvelope::new(
                    "serialization_error",
                    format!("failed to serialize {context} payload"),
                )
            }
            Self::LaneLifecycle { reason } => ErrorEnvelope::new("lane_lifecycle_error", reason),
            Self::PushIntoQueue { .. } => {
                iroha_logger::error!(
                    "Queue rejection reached envelope renderer; expected to be handled earlier"
                );
                ErrorEnvelope::new("queue_error", "queue request rejected")
            }
            Self::AppQueryValidation { code, message } => ErrorEnvelope::new(code, message),
            Self::ProofRateLimited {
                endpoint,
                retry_after_secs,
            } => ErrorEnvelope::new(
                "proof_rate_limited",
                format!("proof endpoint `{endpoint}` throttled; retry after {retry_after_secs}s"),
            ),
            #[cfg(feature = "telemetry")]
            Self::Prometheus(err) => {
                iroha_logger::error!(?err, "Failed to render Prometheus metrics");
                ErrorEnvelope::new("metrics_error", "failed to render metrics payload")
            }
            #[cfg(feature = "telemetry")]
            Self::StatusFailure(err) => {
                iroha_logger::error!(?err, "Failed to render status response");
                ErrorEnvelope::new("status_error", "failed to render status payload")
            }
            #[cfg(feature = "telemetry")]
            Self::TelemetryProfileRestricted { endpoint, profile } => ErrorEnvelope::new(
                "telemetry_profile_restricted",
                format!("telemetry endpoint `{endpoint}` disabled by profile `{profile:?}`"),
            ),
            #[cfg(feature = "profiling")]
            Self::Pprof(err) => {
                iroha_logger::error!(?err, "Failed to collect profiling data");
                ErrorEnvelope::new("profiling_error", "failed to collect profiling data")
            }
            Self::ConfigurationFailure(err) => {
                iroha_logger::error!(?err, "Configuration subsystem request failed");
                ErrorEnvelope::new("configuration_failure", err.to_string())
            }
            Self::StatusSegmentNotFound(err) => {
                iroha_logger::warn!(?err, "Requested status segment not found");
                ErrorEnvelope::new("status_segment_not_found", err.to_string())
            }
            Self::StartServer => {
                iroha_logger::error!(
                    "Attempted to map `Error::StartServer` to an HTTP response envelope"
                );
                ErrorEnvelope::new("torii_start_failure", "Torii server failed to start")
            }
            Self::FailedExit => {
                iroha_logger::error!(
                    "Attempted to map `Error::FailedExit` to an HTTP response envelope"
                );
                ErrorEnvelope::new(
                    "torii_shutdown_failure",
                    "Torii server terminated unexpectedly",
                )
            }
        }
    }

    fn status_code(&self) -> StatusCode {
        use Error::*;

        match self {
            Query(e) => Self::query_status_code(e),
            ApiVersion(err) => err.status(),
            AcceptTransaction(_) => StatusCode::BAD_REQUEST,
            AppQueryValidation { .. } => StatusCode::BAD_REQUEST,
            LaneLifecycle { .. } => StatusCode::BAD_REQUEST,
            Config(_) | StatusSegmentNotFound(_) => StatusCode::NOT_FOUND,
            SerializationFailure { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            PushIntoQueue { source, .. } => Self::status_code_for_queue_error(source.as_ref()),
            #[cfg(feature = "telemetry")]
            Prometheus(_) | StatusFailure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "telemetry")]
            TelemetryProfileRestricted { .. } => StatusCode::SERVICE_UNAVAILABLE,
            #[cfg(feature = "profiling")]
            Pprof(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ProofRateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            ConfigurationFailure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            StartServer => {
                iroha_logger::error!(
                    "Attempted to map `Error::StartServer` to an HTTP status code"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            }
            FailedExit => {
                iroha_logger::error!("Attempted to map `Error::FailedExit` to an HTTP status code");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    fn query_status_code(validation_error: &iroha_data_model::ValidationFail) -> StatusCode {
        use iroha_data_model::{
            ValidationFail::*, isi::error::InstructionExecutionError,
            query::error::QueryExecutionFail::*,
        };

        match validation_error {
            NotPermitted(_) => StatusCode::FORBIDDEN,
            IvmAdmission(_) => StatusCode::BAD_REQUEST,
            AxtReject(_) => StatusCode::BAD_REQUEST,
            QueryFailed(query_error)
            | InstructionFailed(InstructionExecutionError::Query(query_error)) => match query_error
            {
                Conversion(_)
                | CursorMismatch
                | CursorDone
                | FetchSizeTooBig
                | InvalidSingularParameters => StatusCode::BAD_REQUEST,
                Find(_) | NotFound => StatusCode::NOT_FOUND,
                Expired => StatusCode::GONE,
                AuthorityQuotaExceeded => StatusCode::TOO_MANY_REQUESTS,
                CapacityLimit => StatusCode::TOO_MANY_REQUESTS,
                GasBudgetExceeded => StatusCode::TOO_MANY_REQUESTS,
            },
            TooComplex => StatusCode::UNPROCESSABLE_ENTITY,
            InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            InstructionFailed(error) => {
                iroha_logger::error!(
                    ?error,
                    "Query validation failed with unexpected error. This means a bug inside Runtime Executor",
                );
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    #[cfg(feature = "telemetry")]
    fn telemetry_profile_forbidden(endpoint: &'static str, profile: TelemetryProfile) -> Self {
        Self::TelemetryProfileRestricted { endpoint, profile }
    }
}

impl Error {
    fn status_code_for_queue_error(err: &queue::Error) -> StatusCode {
        match err {
            queue::Error::Full | queue::Error::MaximumTransactionsPerUser => {
                StatusCode::TOO_MANY_REQUESTS
            }
            queue::Error::Expired => StatusCode::BAD_REQUEST,
            queue::Error::InBlockchain => StatusCode::CONFLICT,
            queue::Error::IsInQueue => StatusCode::CONFLICT,
            queue::Error::Governance(_) => StatusCode::INTERNAL_SERVER_ERROR,
            queue::Error::GovernanceNotPermitted { .. } => StatusCode::FORBIDDEN,
            queue::Error::LaneComplianceDenied { .. } => StatusCode::FORBIDDEN,
            queue::Error::LanePrivacyProofRejected { .. } => StatusCode::FORBIDDEN,
            queue::Error::UaidNotBound { .. } => StatusCode::FORBIDDEN,
        }
    }

    fn queue_error_summary(err: &queue::Error) -> (&'static str, &'static str) {
        match err {
            queue::Error::Full => ("queue_full", "transaction queue is at capacity"),
            queue::Error::MaximumTransactionsPerUser => (
                "per_user_queue_limit",
                "authority reached its per-user queue capacity",
            ),
            queue::Error::Expired => (
                "transaction_expired",
                "transaction expired before admission",
            ),
            queue::Error::InBlockchain => (
                "already_committed",
                "transaction already committed to the blockchain",
            ),
            queue::Error::IsInQueue => (
                "already_enqueued",
                "transaction already present in the queue",
            ),
            queue::Error::Governance(_) => (
                "queue_governance_invalid",
                "lane governance manifest is missing or invalid",
            ),
            queue::Error::GovernanceNotPermitted { .. } => (
                "queue_governance_rejected",
                "lane governance manifest rejected the transaction",
            ),
            queue::Error::LaneComplianceDenied { .. } => (
                "queue_lane_compliance_denied",
                "lane compliance policy rejected the transaction",
            ),
            queue::Error::LanePrivacyProofRejected { .. } => (
                "queue_lane_privacy_proof_rejected",
                "lane privacy proof rejected the transaction",
            ),
            queue::Error::UaidNotBound { .. } => (
                "queue_uaid_binding_missing",
                "authority UAID is not bound to the target dataspace",
            ),
        }
    }

    fn queue_error_envelope(
        err: &queue::Error,
        backpressure: queue::BackpressureState,
    ) -> QueueErrorEnvelope {
        let (code, message) = Self::queue_error_summary(err);
        let saturated = backpressure.is_saturated();
        let retry_after_seconds = match err {
            queue::Error::Full | queue::Error::MaximumTransactionsPerUser => Some(1),
            _ => None,
        };
        QueueErrorEnvelope {
            code: code.to_owned(),
            message: message.to_owned(),
            queue: QueueErrorSnapshot {
                state: if saturated {
                    "saturated".to_owned()
                } else {
                    "healthy".to_owned()
                },
                queued: backpressure.queued() as u64,
                capacity: backpressure.capacity().get() as u64,
                saturated,
            },
            retry_after_seconds,
        }
    }
}

fn accept_transaction_metadata(
    err: &iroha_core::tx::AcceptTransactionFail,
) -> (&'static str, String) {
    match err {
        iroha_core::tx::AcceptTransactionFail::SignatureVerification(fail) => {
            let detail = if fail.detail.is_empty() {
                fail.code().summary().to_owned()
            } else {
                fail.detail.clone()
            };
            (
                fail.code().as_str(),
                format!("failed to accept transaction: {detail}"),
            )
        }
        iroha_core::tx::AcceptTransactionFail::NetworkTimeUnhealthy { .. } => (
            "PRTRY:NTS_UNHEALTHY",
            format!("failed to accept transaction: {err}"),
        ),
        iroha_core::tx::AcceptTransactionFail::TransactionLimit(limit) => (
            "transaction_rejected",
            format!("failed to accept transaction: {}", limit.reason),
        ),
        _ => (
            "transaction_rejected",
            format!("failed to accept transaction: {err}"),
        ),
    }
}

fn queue_rejection_metadata(err: &queue::Error) -> (&'static str, String) {
    match err {
        queue::Error::Full => (
            "PRTRY:QUEUE_FULL",
            "transaction queue is at capacity".to_owned(),
        ),
        queue::Error::MaximumTransactionsPerUser => (
            "PRTRY:QUEUE_RATE",
            "authority reached per-user queue capacity".to_owned(),
        ),
        queue::Error::Expired => ("ED07", "transaction expired before admission".to_owned()),
        queue::Error::InBlockchain => (
            "PRTRY:ALREADY_COMMITTED",
            "transaction already committed to the blockchain".to_owned(),
        ),
        queue::Error::IsInQueue => (
            "PRTRY:ALREADY_ENQUEUED",
            "transaction already present in the queue".to_owned(),
        ),
        queue::Error::Governance(err) => (
            "PRTRY:QUEUE_GOVERNANCE_INVALID",
            format!("lane governance manifest invalid: {err}"),
        ),
        queue::Error::GovernanceNotPermitted { alias, reason } => (
            "PRTRY:QUEUE_GOVERNANCE_REJECTED",
            format!("lane governance rejected transaction for alias '{alias}': {reason}"),
        ),
        queue::Error::LaneComplianceDenied { alias, reason } => (
            "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED",
            format!("lane compliance policy rejected transaction for alias '{alias}': {reason}"),
        ),
        queue::Error::LanePrivacyProofRejected { alias, reason } => (
            "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED",
            format!("lane privacy proof rejected transaction for alias '{alias}': {reason}"),
        ),
        queue::Error::UaidNotBound { uaid, dataspace } => (
            "PRTRY:QUEUE_UAID_NOT_BOUND",
            format!(
                "UAID {uaid} is not bound to dataspace {}",
                dataspace.as_u64()
            ),
        ),
    }
}

/// Result type
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Provider of online peers
#[derive(Clone)]
pub struct OnlinePeersProvider {
    rx: watch::Receiver<HashSet<Peer>>,
}

impl OnlinePeersProvider {
    /// Constructor
    pub fn new(rx: watch::Receiver<HashSet<Peer>>) -> Self {
        Self { rx }
    }

    pub(crate) fn get(&self) -> HashSet<Peer> {
        self.rx.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    // for `collect`
    use std::{
        collections::HashSet,
        num::{NonZeroU64, NonZeroUsize},
        str::FromStr,
        sync::Arc,
        time::Duration,
    };

    use axum::{
        extract::State,
        http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
    };
    use futures::executor;
    use http_body_util::BodyExt as _;
    use iroha_config::parameters::actual;
    use iroha_crypto::{Hash, KeyPair, SignatureOf};
    use iroha_data_model::{
        ChainId, Registrable, ValidationFail,
        account::rekey::AccountLabel,
        account::{Account, AccountId, OpaqueAccountId},
        block::{BlockHeader, BlockSignature, SignedBlock},
        domain::{Domain, DomainId},
        name::Name,
        nexus::{
            AxtPolicySnapshot, AxtRejectContext, AxtRejectReason, DataSpaceId, LaneId,
            UniversalAccountId,
        },
        permission::Permission,
        proof::{ProofId, ProofRecord, ProofStatus, VerifyingKeyId, VerifyingKeyRecord},
        transaction::{
            IvmBytecode, IvmProved,
            signed::{TransactionBuilder, TransactionResultInner},
        },
    };
    use iroha_executor_data_model::permission::sorafs::CanOperateSorafsRepair;
    use nonzero_ext::nonzero;
    use sorafs_manifest::repair::{
        REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_WORKER_SIGNATURE_VERSION_V1,
        RepairCauseV1, RepairEvidenceV1, RepairReportV1, RepairTicketId, RepairWorkerActionV1,
        RepairWorkerSignaturePayloadV1,
    };

    use super::*;
    #[cfg(feature = "telemetry")]
    use crate::tests_runtime_handlers::mk_norito_rpc_test_harness;
    use crate::{
        limits,
        tests_runtime_handlers::{
            guard_account_resolvers, mk_app_state_for_tests,
            mk_app_state_for_tests_with_iso_bridge, mk_app_state_for_tests_with_options,
            mk_app_state_for_tests_with_world, negotiated,
        },
    };

    fn sample_iso_bridge_config(alias: &str, account_id: &AccountId) -> actual::IsoBridge {
        let signer_keypair = KeyPair::random();
        actual::IsoBridge {
            enabled: true,
            dedupe_ttl_secs: 30,
            signer: Some(actual::IsoBridgeSigner {
                account_id: account_id.to_string(),
                private_key: signer_keypair.private_key().clone(),
            }),
            account_aliases: vec![actual::IsoAccountAlias {
                iban: alias.to_string(),
                account_id: account_id.to_string(),
            }],
            currency_assets: Vec::new(),
            reference_data: actual::IsoReferenceData::default(),
        }
    }

    fn seed_proof_record_at_height(
        app: &SharedAppState,
        backend: &str,
        proof_hash: [u8; 32],
        verified_at_height: u64,
    ) -> String {
        let height = verified_at_height.max(1);
        // Ensure the core state height is aligned with the proof being seeded to avoid
        // commit-height mismatches when tests inject multiple proof blocks.
        set_latest_block_height(app, height.saturating_sub(1));
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        let id = ProofId {
            backend: backend.to_string(),
            proof_hash,
        };
        let rec = ProofRecord {
            id: id.clone(),
            vk_ref: None,
            vk_commitment: None,
            status: ProofStatus::Verified,
            verified_at_height: Some(verified_at_height),
            bridge: None,
        };
        stx.world.proofs_mut_for_testing().insert(id.clone(), rec);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("seed proof block commit should succeed");
        id.to_string()
    }

    fn seed_proof_record(app: &SharedAppState, backend: &str, proof_hash: [u8; 32]) -> String {
        seed_proof_record_at_height(app, backend, proof_hash, 1)
    }

    fn current_block_height(app: &SharedAppState) -> u64 {
        app.state
            .transactions_latest_height_for_testing()
            .try_into()
            .expect("height should fit into u64")
    }

    fn next_block_height(app: &SharedAppState) -> u64 {
        current_block_height(app).saturating_add(1).max(1)
    }

    #[cfg(feature = "zk-stark")]
    fn sample_stark_vk_box(
        backend: &str,
        circuit_id: &str,
        hash_fn: u8,
    ) -> iroha_data_model::proof::VerifyingKeyBox {
        let vk_payload = iroha_core::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.to_owned(),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn,
        };
        let bytes = norito::to_bytes(&vk_payload).expect("encode stark vk payload");
        iroha_data_model::proof::VerifyingKeyBox::new(backend.to_owned(), bytes)
    }

    fn sample_ivm_prove_authority() -> AccountId {
        AccountId::new(
            "wonderland".parse().expect("domain"),
            KeyPair::random().public_key().clone(),
        )
    }

    fn sample_ivm_prove_metadata() -> iroha_data_model::metadata::Metadata {
        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            Name::from_str("gas_limit").expect("static gas_limit key"),
            iroha_primitives::json::Json::new(50_000_000_u64),
        );
        metadata
    }

    fn make_ivm_prove_request(
        vk_ref: VerifyingKeyId,
        bytecode: IvmBytecode,
        proved: Option<IvmProved>,
    ) -> ZkIvmProveRequestDto {
        ZkIvmProveRequestDto {
            vk_ref,
            authority: sample_ivm_prove_authority(),
            metadata: sample_ivm_prove_metadata(),
            bytecode,
            proved,
        }
    }

    fn set_latest_block_height(app: &SharedAppState, height: u64) {
        let mut current_height = current_block_height(app);
        while current_height < height {
            let next_height = current_height.saturating_add(1);
            let header = BlockHeader::new(
                NonZeroU64::new(next_height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(next_height as usize).expect("block count should be non-zero"),
            );
            block
                .commit()
                .expect("set latest block height commit should succeed");
            current_height = next_height;
        }
    }

    #[cfg(feature = "app_api")]
    fn repair_report(
        ticket: &str,
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
        submitted_at_unix: u64,
    ) -> RepairReportV1 {
        RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId(ticket.to_string()),
            auditor_account: "auditor#sora".into(),
            submitted_at_unix,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual {
                    reason: "test".into(),
                },
                evidence_json: None,
                notes: None,
            },
            notes: None,
        }
    }

    #[cfg(feature = "app_api")]
    fn grant_repair_worker_permission(
        app: &SharedAppState,
        account_id: &AccountId,
        provider_id: [u8; 32],
    ) {
        let height = next_block_height(app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world_mut_for_testing().add_account_permission(
            account_id,
            Permission::from(CanOperateSorafsRepair {
                provider_id: ProviderId::new(provider_id),
            }),
        );
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block
            .commit()
            .expect("commit should persist repair worker permission");
    }

    #[tokio::test]
    async fn alias_resolve_returns_account_from_iso_bridge() {
        let account_id: AccountId = format!("{}@wonderland", KeyPair::random().public_key())
            .parse()
            .expect("valid account id");
        let alias = "GB82 WEST 1234 5698 7654 32";
        let iso_cfg = sample_iso_bridge_config(alias, &account_id);
        let app = mk_app_state_for_tests_with_iso_bridge(Some(iso_cfg));

        let response = handler_alias_resolve(
            State(app),
            NoritoJson(routing::AliasResolveRequestDto {
                alias: alias.to_string(),
            }),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: routing::AliasResolveResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(dto.alias, "GB82WEST12345698765432");
        assert_eq!(dto.account_id, account_id.to_string());
        assert_eq!(dto.source.as_deref(), Some("alias_service"));
        assert_eq!(dto.index, Some(0));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn sorafs_repair_worker_auth_accepts_signed_worker() {
        let app = mk_app_state_for_tests();
        let _guard = guard_account_resolvers(&app);
        let report = repair_report("REP-900", [0x11; 32], [0x22; 32], 1_701_000_000);
        app.sorafs_node
            .enqueue_repair_report(&report)
            .expect("enqueue report");

        let worker_key = KeyPair::random();
        let worker_id: AccountId = format!("{}@sora", worker_key.public_key())
            .parse()
            .expect("valid worker id");
        let worker_id_literal = format!("{worker_id}@{}", worker_id.domain());
        grant_repair_worker_permission(&app, &worker_id, report.evidence.provider_id);

        let claimed_at = report.submitted_at_unix + 10;
        let idempotency_key = "claim-900";
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            worker_id: worker_id_literal.clone(),
            idempotency_key: idempotency_key.to_string(),
            action: RepairWorkerActionV1::Claim {
                claimed_at_unix: claimed_at,
            },
        };
        let signature = SignatureOf::new(worker_key.private_key(), &payload);

        let auth = enforce_sorafs_repair_worker_auth(
            &app,
            &report.ticket_id,
            &hex::encode(report.evidence.manifest_digest),
            &worker_id_literal,
            idempotency_key,
            RepairWorkerActionV1::Claim {
                claimed_at_unix: claimed_at,
            },
            &signature,
        );
        assert!(auth.is_ok(), "signed worker should be accepted: {auth:?}");
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn sorafs_repair_worker_auth_rejects_missing_permission() {
        let app = mk_app_state_for_tests();
        let _guard = guard_account_resolvers(&app);
        let report = repair_report("REP-901", [0x33; 32], [0x44; 32], 1_701_000_100);
        app.sorafs_node
            .enqueue_repair_report(&report)
            .expect("enqueue report");

        let worker_key = KeyPair::random();
        let worker_id: AccountId = format!("{}@sora", worker_key.public_key())
            .parse()
            .expect("valid worker id");
        let worker_id_literal = format!("{worker_id}@{}", worker_id.domain());

        let failed_at = report.submitted_at_unix + 20;
        let idempotency_key = "fail-901";
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            worker_id: worker_id_literal.clone(),
            idempotency_key: idempotency_key.to_string(),
            action: RepairWorkerActionV1::Fail {
                failed_at_unix: failed_at,
                reason: "no-permission".into(),
            },
        };
        let signature = SignatureOf::new(worker_key.private_key(), &payload);

        let auth = enforce_sorafs_repair_worker_auth(
            &app,
            &report.ticket_id,
            &hex::encode(report.evidence.manifest_digest),
            &worker_id_literal,
            idempotency_key,
            RepairWorkerActionV1::Fail {
                failed_at_unix: failed_at,
                reason: "no-permission".into(),
            },
            &signature,
        );
        assert!(matches!(
            auth,
            Err(Error::Query(ValidationFail::NotPermitted(_)))
        ));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn sorafs_repair_worker_auth_rejects_manifest_digest_mismatch() {
        let app = mk_app_state_for_tests();
        let _guard = guard_account_resolvers(&app);
        let report = repair_report("REP-902", [0x55; 32], [0x66; 32], 1_701_000_200);
        app.sorafs_node
            .enqueue_repair_report(&report)
            .expect("enqueue report");

        let worker_key = KeyPair::random();
        let worker_id: AccountId = format!("{}@sora", worker_key.public_key())
            .parse()
            .expect("valid worker id");
        let worker_id_literal = format!("{worker_id}@{}", worker_id.domain());
        grant_repair_worker_permission(&app, &worker_id, report.evidence.provider_id);

        let claimed_at = report.submitted_at_unix + 10;
        let idempotency_key = "claim-902";
        let payload = RepairWorkerSignaturePayloadV1 {
            version: REPAIR_WORKER_SIGNATURE_VERSION_V1,
            ticket_id: report.ticket_id.clone(),
            manifest_digest: report.evidence.manifest_digest,
            provider_id: report.evidence.provider_id,
            worker_id: worker_id_literal.clone(),
            idempotency_key: idempotency_key.to_string(),
            action: RepairWorkerActionV1::Claim {
                claimed_at_unix: claimed_at,
            },
        };
        let signature = SignatureOf::new(worker_key.private_key(), &payload);

        let auth = enforce_sorafs_repair_worker_auth(
            &app,
            &report.ticket_id,
            &hex::encode([0x77u8; 32]),
            &worker_id_literal,
            idempotency_key,
            RepairWorkerActionV1::Claim {
                claimed_at_unix: claimed_at,
            },
            &signature,
        );
        assert!(matches!(
            auth,
            Err(Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
            )))
        ));
    }

    #[tokio::test]
    async fn alias_resolve_returns_not_found_for_unknown_alias() {
        let account_id: AccountId = format!("{}@wonderland", KeyPair::random().public_key())
            .parse()
            .expect("valid account id");
        let iso_cfg = sample_iso_bridge_config("GB82WEST12345698765432", &account_id);
        let app = mk_app_state_for_tests_with_iso_bridge(Some(iso_cfg));

        let response = handler_alias_resolve(
            State(app),
            NoritoJson(routing::AliasResolveRequestDto {
                alias: "FR7630006000011234567890189".to_string(),
            }),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn alias_resolve_returns_service_unavailable_without_runtime() {
        let app = mk_app_state_for_tests();
        let response = handler_alias_resolve(
            State(app),
            NoritoJson(routing::AliasResolveRequestDto {
                alias: "GB82WEST12345698765432".to_string(),
            }),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn accounts_resolve_accepts_alias_uaid_and_opaque() {
        let domain: DomainId = "wonderland".parse().expect("domain");
        let key_pair = KeyPair::random();
        let account_id = AccountId::new(domain.clone(), key_pair.public_key().clone());
        let label = AccountLabel::new(domain.clone(), Name::from_str("alice").expect("label"));
        let uaid = UniversalAccountId::from_hash(
            Hash::from_str("00112233445566778899aabbccddeeff00112233445566778899aabbccddeef1")
                .expect("uaid hash"),
        );
        let opaque = OpaqueAccountId::from_hash(
            Hash::from_str("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221101")
                .expect("opaque hash"),
        );

        let account = Account::new(account_id.clone())
            .with_label(Some(label))
            .with_uaid(Some(uaid))
            .with_opaque_ids(vec![opaque])
            .build(&account_id);
        let domain_entry = Domain::new(domain.clone()).build(&account_id);
        let world = World::with_assets([domain_entry], [account], [], [], []);
        let app = mk_app_state_for_tests_with_world(world);
        let _guard = guard_account_resolvers(&app);

        let raw_public_literal = format!("{}@{}", key_pair.public_key(), domain);
        let cases = [
            (format!("alice@{domain}"), "alias", None),
            (raw_public_literal, "public_key", None),
            (uaid.to_string(), "uaid", None),
            (opaque.to_string(), "opaque", None),
            (account_id.to_string(), "encoded", Some("ih58")),
        ];

        for (literal, source, format) in cases {
            let response = routing::handle_v1_accounts_resolve(
                NoritoJson(routing::AccountResolveRequestDto { literal }),
                app.telemetry.clone(),
            )
            .await
            .expect("handler should succeed")
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);
            let body = http_body_util::BodyExt::collect(response.into_body())
                .await
                .unwrap()
                .to_bytes();
            let dto: routing::AccountResolveResponseDto =
                norito::json::from_slice(&body).expect("json decode");
            assert_eq!(dto.account_id, account_id.to_string());
            assert_eq!(dto.domain, domain.to_string());
            assert_eq!(dto.source, source);
            assert_eq!(dto.format.as_deref(), format);
        }
    }

    #[tokio::test]
    async fn alias_voprf_returns_mock_digest() {
        let blinded_hex = "deadbeef";
        let response =
            handler_alias_voprf_evaluate(NoritoJson(routing::AliasVoprfEvaluateRequestDto {
                blinded_element_hex: blinded_hex.to_string(),
            }))
            .await
            .expect("handler should succeed")
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .expect("content-type header"),
            "application/json"
        );
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: routing::AliasVoprfEvaluateResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(dto.backend, routing::AliasVoprfBackendDto::Blake2b512Mock);
        let blinded = hex::decode(blinded_hex).expect("hex");
        let expected = iroha_core::alias::evaluate_alias_voprf(&blinded).expect("evaluates");
        let expected_hex = hex::encode(expected.evaluated_element);
        assert_eq!(dto.evaluated_element_hex, expected_hex);
    }

    #[tokio::test]
    async fn proof_record_get_advertises_cache_and_304() {
        let app = mk_app_state_for_tests();
        let id = seed_proof_record(&app, "debug-proof", [0xAB; 32]);

        let first = handler_proof_record_get(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::extract::Path(id.clone()),
        )
        .await
        .expect("proof record ok")
        .into_response();
        assert_eq!(first.status(), StatusCode::OK);
        let etag = first
            .headers()
            .get(axum::http::header::ETAG)
            .cloned()
            .expect("etag header");
        let cache_control = first
            .headers()
            .get(axum::http::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            cache_control.contains("max-age"),
            "cache header should be present"
        );
        let body = http_body_util::BodyExt::collect(first.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert!(!body.is_empty(), "first response includes body");

        let mut conditional_headers = HeaderMap::new();
        conditional_headers.insert(axum::http::header::IF_NONE_MATCH, etag);
        let not_modified = handler_proof_record_get(
            State(app.clone()),
            negotiated(&app),
            conditional_headers,
            axum::extract::Path(id),
        )
        .await
        .expect("conditional proof ok")
        .into_response();
        assert_eq!(not_modified.status(), StatusCode::NOT_MODIFIED);
        let empty = http_body_util::BodyExt::collect(not_modified.into_body())
            .await
            .unwrap()
            .to_bytes();
        assert!(empty.is_empty(), "304 responses have no body");
    }

    #[tokio::test]
    async fn proof_retention_status_reports_counts() {
        let mut app = mk_app_state_for_tests();
        let cap = iroha_config::parameters::defaults::zk::proof::RECORD_HISTORY_CAP;
        let grace = iroha_config::parameters::defaults::zk::proof::RETENTION_GRACE_BLOCKS;
        let prune_batch = iroha_config::parameters::defaults::zk::proof::PRUNE_BATCH_SIZE;
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique core state");
            state.zk.proof_history_cap = cap;
            state.zk.proof_retention_grace_blocks = grace;
            state.zk.proof_prune_batch = prune_batch;
        }
        // Seed one record outside the grace window, one on the boundary, and one fresh record.
        let current_height = grace + 5;
        let stale_height = current_height.saturating_sub(grace + 1);
        let boundary_height = current_height.saturating_sub(grace);
        let fresh_height = current_height;
        {
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            let mut insert_record = |proof_hash: [u8; 32], verified_at_height: u64| -> ProofId {
                let id = ProofId {
                    backend: "debug-proof".to_string(),
                    proof_hash,
                };
                let rec = ProofRecord {
                    id: id.clone(),
                    vk_ref: None,
                    vk_commitment: None,
                    status: ProofStatus::Verified,
                    verified_at_height: Some(verified_at_height),
                    bridge: None,
                };
                stx.world.proofs_mut_for_testing().insert(id.clone(), rec);
                id
            };

            let _ = insert_record([0xCC; 32], stale_height);
            let _ = insert_record([0xDD; 32], boundary_height);
            let _ = insert_record([0xEE; 32], fresh_height);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(1).expect("block count should be non-zero"),
            );
            block
                .commit()
                .expect("seed proof block commit should succeed");
        }
        // Advance the latest block past the grace window so the stale record becomes prunable.
        set_latest_block_height(&app, current_height);
        assert_eq!(
            app.state.view().world().proofs().len(),
            3,
            "expected three proof records in the fixture"
        );

        let response = handler_proof_retention_status(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            None,
        )
        .await
        .expect("retention status ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let status: iroha_torii_shared::ProofRetentionStatus =
            norito::json::from_slice(&body).expect("decode retention status");
        let backend = status
            .backends
            .iter()
            .find(|entry| entry.backend == "debug-proof")
            .expect("backend present");
        assert_eq!(status.cap_per_backend, cap);
        assert_eq!(status.grace_blocks, grace);
        assert_eq!(status.prune_batch, prune_batch);
        assert_eq!(status.total_records, 3);
        // With defaults, the grace window is large enough that none of the seeded proofs
        // are prunable yet; this ensures the endpoint mirrors policy rather than fixed numbers.
        assert_eq!(status.total_prunable, 0);
        assert_eq!(backend.records, 3);
        assert_eq!(backend.prunable, 0);
        assert_eq!(backend.oldest_height, Some(stale_height));
        assert_eq!(backend.newest_height, Some(fresh_height));
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn axt_proof_cache_debug_reports_snapshot() {
        let mut app = mk_app_state_for_tests();
        let dsid = DataSpaceId::new(9);
        let manifest_root = [0xAA; 32];
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app");
            let state = Arc::get_mut(&mut app_mut.state).expect("unique core state");
            state
                .telemetry
                .set_axt_proof_cache_state(dsid, "miss", manifest_root, 5, Some(10));
            state.telemetry.note_axt_policy_reject(
                LaneId::new(2),
                iroha_data_model::nexus::AxtRejectReason::Manifest,
                77,
            );
            state.telemetry.set_axt_reject_hint(
                dsid,
                LaneId::new(2),
                10,
                11,
                iroha_data_model::nexus::AxtRejectReason::HandleEra,
            );
            state
                .telemetry
                .set_axt_policy_snapshot_version(&AxtPolicySnapshot {
                    version: 77,
                    entries: Vec::new(),
                });
        }

        let response = handler_axt_proof_cache_status(
            State(app.clone()),
            HeaderMap::new(),
            axum::extract::ConnectInfo("127.0.0.1:8080".parse().unwrap()),
        )
        .await
        .expect("handler ok")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let snapshot: iroha_core::telemetry::AxtDebugStatus =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(snapshot.policy_snapshot_version, 77);
        assert_eq!(
            snapshot.last_reject.as_ref().map(|reject| reject.reason),
            Some(iroha_data_model::nexus::AxtRejectReason::Manifest)
        );
        assert_eq!(snapshot.hints.len(), 1);
        assert_eq!(
            snapshot.hints[0].reason,
            iroha_data_model::nexus::AxtRejectReason::HandleEra
        );
        assert_eq!(snapshot.cache.len(), 1);
        let entry = &snapshot.cache[0];
        assert_eq!(entry.dataspace, dsid);
        assert_eq!(entry.status, "miss");
        assert_eq!(entry.manifest_root, Some(manifest_root));
        assert_eq!(entry.verified_slot, 5);
        assert_eq!(entry.expiry_slot, Some(10));
    }

    #[test]
    fn axt_reject_query_response_carries_headers() {
        let ctx = AxtRejectContext {
            reason: AxtRejectReason::HandleEra,
            dataspace: Some(DataSpaceId::new(7)),
            lane: Some(LaneId::new(3)),
            snapshot_version: 77,
            detail: "handle era below policy minimum".to_owned(),
            next_min_handle_era: Some(5),
            next_min_sub_nonce: Some(2),
        };
        let response = Error::Query(ValidationFail::AxtReject(ctx)).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let headers = response.headers();
        assert_eq!(
            headers
                .get("x-iroha-axt-code")
                .and_then(|v| v.to_str().ok()),
            Some("AXT_HANDLE_ERA")
        );
        assert_eq!(
            headers
                .get("x-iroha-axt-reason")
                .and_then(|v| v.to_str().ok()),
            Some("era")
        );
        assert_eq!(
            headers
                .get("x-iroha-axt-snapshot-version")
                .and_then(|v| v.to_str().ok()),
            Some("77")
        );
        assert_eq!(
            headers
                .get("x-iroha-axt-dataspace")
                .and_then(|v| v.to_str().ok()),
            Some("7")
        );
        assert_eq!(
            headers
                .get("x-iroha-axt-lane")
                .and_then(|v| v.to_str().ok()),
            Some("3")
        );
        assert_eq!(
            headers
                .get("x-iroha-axt-next-handle-era")
                .and_then(|v| v.to_str().ok()),
            Some("5")
        );
        assert_eq!(
            headers
                .get("x-iroha-axt-next-sub-nonce")
                .and_then(|v| v.to_str().ok()),
            Some("2")
        );
    }

    #[tokio::test]
    async fn proof_get_egress_throttled_returns_retry_after() {
        let mut app = mk_app_state_for_tests();
        let _ = seed_proof_record(&app, "debug-proof", [0xBB; 32]);
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.proof_limits.retry_after = std::time::Duration::from_secs(2);
            state.proof_egress_limiter = limits::RateLimiter::new_u64(Some(1), Some(1));
        }

        let resp = handler_get_proof_by_backend_hash(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::extract::Path(("debug-proof".to_string(), hex::encode([0xBB; 32]))),
        )
        .await
        .unwrap_or_else(Error::into_response);

        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after = resp
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert_eq!(retry_after, "2");
    }

    #[tokio::test]
    async fn submit_proof_rejects_oversize_body() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.proof_limits.max_body_bytes = 4;
        }
        let err = match handler_zk_submit_proof(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from_static(&[0x11; 16]),
        )
        .await
        {
            Ok(_) => panic!("oversized proof should be rejected"),
            Err(err) => err,
        };

        match err {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(msg),
            )) => assert!(
                msg.contains("payload too large"),
                "error message should explain limit"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn zk_ivm_prove_job_completes_and_does_not_expose_gas_used() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_prover_keys_dir = temp.path().to_path_buf();
            let core = Arc::get_mut(&mut state.state).expect("unique core state");
            core.zk.halo2.enabled = true;
        }

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-fixture");
        let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        );
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture should include verifying key bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture should include verifying key commitment");
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = Some(vk_box);
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        let pk_bytes = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(
            vk_record.key.as_ref().expect("vk_box"),
        )
        .expect("derive proving key bytes");
        let pk_path = zk_pk_store_path(temp.path(), &vk_id);
        std::fs::write(&pk_path, &pk_bytes).expect("write proving key bytes");

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let body = norito::json::to_vec(&req).expect("json encode request");
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect("prove submit ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let created: ZkIvmProveJobCreatedDto =
            norito::json::from_slice(&body).expect("json decode created dto");

        let job_id = created.job_id;
        let mut final_dto: Option<ZkIvmProveJobDto> = None;
        for _ in 0..4000 {
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                negotiated(&app),
                HeaderMap::new(),
                axum::extract::Path(job_id.clone()),
            )
            .await
            .expect("prove get ok")
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);
            let body = http_body_util::BodyExt::collect(response.into_body())
                .await
                .unwrap()
                .to_bytes();
            let rendered = std::str::from_utf8(&body).expect("utf8 body");
            assert!(
                !rendered.contains("gas_used"),
                "prove job response must not expose gas_used"
            );
            let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
            match dto.status.as_str() {
                "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
                "done" => {
                    final_dto = Some(dto);
                    break;
                }
                "error" => panic!("prove job failed: {:?}", dto.error),
                other => panic!("unexpected prove job status: {other}"),
            }
        }
        let dto = final_dto.expect("prove job should complete");
        assert!(
            dto.attachment.is_some(),
            "expected proof attachment in done response"
        );

        let response = handler_zk_ivm_prove_delete(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::extract::Path(job_id.clone()),
        )
        .await
        .expect("prove delete ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cfg(feature = "zk-stark")]
    #[tokio::test]
    async fn zk_ivm_prove_job_completes_for_stark_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_prover_keys_dir = temp.path().to_path_buf();
            let core = Arc::get_mut(&mut state.state).expect("unique core state");
            core.zk.stark.enabled = true;
            core.zk.halo2.enabled = false;
            core.zk.verify_timeout = Duration::ZERO;
        }

        let backend = "stark/fri-v1/sha256-goldilocks-v1";
        let circuit_id = "stark/fri-v1/sha256-goldilocks-v1:ivm-execution-v1";
        let vk_id = VerifyingKeyId::new(backend, "ivm-exec-v1-stark");
        let vk_box = sample_stark_vk_box(
            backend,
            circuit_id,
            iroha_core::zk_stark::STARK_HASH_SHA256_V1,
        );
        let vk_commitment = iroha_core::zk::hash_vk(&vk_box);

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            circuit_id,
            iroha_data_model::zk::BackendTag::Stark,
            "goldilocks",
            iroha_core::zk::ivm_execution_public_inputs_schema_hash(),
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = Some(vk_box.clone());
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let body = norito::json::to_vec(&req).expect("json encode request");
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect("prove submit ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let created: ZkIvmProveJobCreatedDto =
            norito::json::from_slice(&body).expect("json decode created dto");

        let job_id = created.job_id;
        let mut final_dto: Option<ZkIvmProveJobDto> = None;
        for _ in 0..4000 {
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                negotiated(&app),
                HeaderMap::new(),
                axum::extract::Path(job_id.clone()),
            )
            .await
            .expect("prove get ok")
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);
            let body = http_body_util::BodyExt::collect(response.into_body())
                .await
                .unwrap()
                .to_bytes();
            let rendered = std::str::from_utf8(&body).expect("utf8 body");
            assert!(
                !rendered.contains("gas_used"),
                "prove job response must not expose gas_used"
            );
            let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
            match dto.status.as_str() {
                "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
                "done" => {
                    final_dto = Some(dto);
                    break;
                }
                "error" => panic!("prove job failed: {:?}", dto.error),
                other => panic!("unexpected prove job status: {other}"),
            }
        }
        let dto = final_dto.expect("prove job should complete");
        let attachment = dto
            .attachment
            .expect("expected proof attachment in done response");
        assert_eq!(attachment.backend.as_str(), backend);
        assert!(
            iroha_core::zk::verify_backend(backend, &attachment.proof, Some(&vk_box),),
            "generated STARK attachment should verify"
        );
    }

    #[tokio::test]
    async fn zk_ivm_prove_job_loads_vk_bytes_from_disk_when_inline_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_prover_keys_dir = temp.path().to_path_buf();
            let core = Arc::get_mut(&mut state.state).expect("unique core state");
            core.zk.halo2.enabled = true;
        }

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-disk-vk");
        let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        );
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture should include verifying key bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture should include verifying key commitment");

        let vk_path = zk_vk_store_path(temp.path(), &vk_id);
        std::fs::write(&vk_path, &vk_box.bytes).expect("write verifying key bytes");

        let pk_bytes = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(&vk_box)
            .expect("derive proving key bytes");
        let pk_path = zk_pk_store_path(temp.path(), &vk_id);
        std::fs::write(&pk_path, &pk_bytes).expect("write proving key bytes");

        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = None;
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let body = norito::json::to_vec(&req).expect("json encode request");
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect("prove submit ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let created: ZkIvmProveJobCreatedDto =
            norito::json::from_slice(&body).expect("json decode created dto");

        let job_id = created.job_id;
        let mut final_dto: Option<ZkIvmProveJobDto> = None;
        for _ in 0..4000 {
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                negotiated(&app),
                HeaderMap::new(),
                axum::extract::Path(job_id.clone()),
            )
            .await
            .expect("prove get ok")
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);
            let body = http_body_util::BodyExt::collect(response.into_body())
                .await
                .unwrap()
                .to_bytes();
            let rendered = std::str::from_utf8(&body).expect("utf8 body");
            assert!(
                !rendered.contains("gas_used"),
                "prove job response must not expose gas_used"
            );
            let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
            match dto.status.as_str() {
                "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
                "done" => {
                    final_dto = Some(dto);
                    break;
                }
                "error" => panic!("prove job failed: {:?}", dto.error),
                other => panic!("unexpected prove job status: {other}"),
            }
        }
        let dto = final_dto.expect("prove job should complete");
        assert!(
            dto.attachment.is_some(),
            "expected proof attachment in done response"
        );
    }

    #[tokio::test]
    async fn zk_ivm_prove_job_rejects_mismatched_client_proved_payload() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_prover_keys_dir = temp.path().to_path_buf();
            let core = Arc::get_mut(&mut state.state).expect("unique core state");
            core.zk.halo2.enabled = true;
        }

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-mismatched-proved");
        let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        );
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture should include verifying key bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture should include verifying key commitment");
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = Some(vk_box);
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        let pk_bytes = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(
            vk_record.key.as_ref().expect("vk_box"),
        )
        .expect("derive proving key bytes");
        let pk_path = zk_pk_store_path(temp.path(), &vk_id);
        std::fs::write(&pk_path, &pk_bytes).expect("write proving key bytes");

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let mismatched_proved = IvmProved {
            bytecode: bytecode.clone(),
            overlay: iroha_primitives::const_vec::ConstVec::new_empty(),
            events_commitment: Hash::new(b"wrong-events"),
            gas_policy_commitment: Hash::new(b"wrong-gas-policy"),
        };

        let req = make_ivm_prove_request(vk_id, bytecode, Some(mismatched_proved));
        let body = norito::json::to_vec(&req).expect("json encode request");
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect("prove submit ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let created: ZkIvmProveJobCreatedDto =
            norito::json::from_slice(&body).expect("json decode created dto");

        let job_id = created.job_id;
        let mut final_dto: Option<ZkIvmProveJobDto> = None;
        for _ in 0..4000 {
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                negotiated(&app),
                HeaderMap::new(),
                axum::extract::Path(job_id.clone()),
            )
            .await
            .expect("prove get ok")
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);
            let body = http_body_util::BodyExt::collect(response.into_body())
                .await
                .unwrap()
                .to_bytes();
            let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
            match dto.status.as_str() {
                "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
                "error" => {
                    final_dto = Some(dto);
                    break;
                }
                "done" => panic!("prove job should fail for mismatched proved payload"),
                other => panic!("unexpected prove job status: {other}"),
            }
        }

        let dto = final_dto.expect("prove job should fail");
        let error = dto.error.unwrap_or_default();
        assert!(
            error.contains(
                "provided `proved` payload does not match node-derived execution payload"
            ),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn zk_ivm_derive_returns_proved_payload_without_gas_used() {
        let kp = KeyPair::random();
        let authority = AccountId::new(
            "wonderland".parse().expect("domain"),
            kp.public_key().clone(),
        );
        let domain = Domain::new("wonderland".parse().expect("domain")).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let world = World::with_assets([domain], [account], [], [], []);
        let mut app = mk_app_state_for_tests_with_world(world);
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            let core = Arc::get_mut(&mut state.state).expect("unique core state");
            core.zk.halo2.enabled = true;
        }

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-derive");
        let schema_hash = iroha_core::zk::ivm_execution_public_inputs_schema_hash();
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            schema_hash,
            [0u8; 32],
        );
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
        vk_record.gas_schedule_id = Some("sched_0".to_owned());

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            max_cycles: 1,
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);

        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            Name::from_str("gas_limit").expect("static gas_limit key"),
            iroha_primitives::json::Json::new(50_000_000_u64),
        );

        let req = ZkIvmDeriveRequestDto {
            vk_ref: vk_id,
            authority: authority.clone(),
            metadata,
            bytecode: bytecode.clone(),
        };
        let body = norito::json::to_vec(&req).expect("json encode request");
        let response = handler_zk_ivm_derive(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect("derive ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let rendered = std::str::from_utf8(&body).expect("utf8 body");
        assert!(
            !rendered.contains("gas_used"),
            "derive response must not expose gas_used"
        );
        let dto: ZkIvmDeriveResponseDto = norito::json::from_slice(&body).expect("decode dto");
        assert_eq!(dto.proved.bytecode, bytecode);
    }

    #[test]
    fn zk_ivm_prove_gc_evicts_expired_jobs() {
        let jobs = DashMap::new();

        let vk_ref = VerifyingKeyId::new("halo2/ipa", "gc-fixture");
        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let proved = IvmProved {
            bytecode: IvmBytecode::from_compiled(program),
            overlay: iroha_primitives::const_vec::ConstVec::new_empty(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        };

        jobs.insert(
            "old".to_owned(),
            ZkIvmProveJobState {
                created_ms: 10,
                status: ZkIvmProveJobStatus::Done,
                proved: Some(proved.clone()),
                vk_ref: vk_ref.clone(),
                attachment: None,
                error: None,
                cancel: tokio::sync::watch::channel(false).0,
            },
        );
        let ttl_ms = 1_000;
        jobs.insert(
            "fresh".to_owned(),
            ZkIvmProveJobState {
                created_ms: ttl_ms + 10,
                status: ZkIvmProveJobStatus::Done,
                proved: Some(proved),
                vk_ref,
                attachment: None,
                error: None,
                cancel: tokio::sync::watch::channel(false).0,
            },
        );

        zk_ivm_prove_gc_jobs_at(&jobs, ttl_ms + 20, ttl_ms, 1_024);
        assert!(jobs.get("old").is_none(), "expired jobs should be removed");
        assert!(jobs.get("fresh").is_some(), "fresh jobs should be retained");
    }

    #[tokio::test]
    async fn zk_ivm_prove_rejects_vk_schema_hash_mismatch() {
        let app = mk_app_state_for_tests();

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-schema-mismatch");
        let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        );
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture should include verifying key bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture should include verifying key commitment");
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            [0xAA; 32],
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = Some(vk_box);
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let body = norito::json::to_vec(&req).expect("json encode request");
        let err = match handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        {
            Ok(_) => panic!("schema mismatch should be rejected"),
            Err(err) => err,
        };

        match err {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(msg),
            )) => assert!(
                msg.contains("schema hash"),
                "error should mention schema hash mismatch"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn zk_ivm_prove_rejects_when_queue_full() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_prover_keys_dir = temp.path().to_path_buf();
            state.zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(1));
            state.zk_ivm_prove_inflight = Arc::new(tokio::sync::Semaphore::new(1));
            state.zk_ivm_prove_slots_total = 1;
            state.zk_ivm_prove_inflight_total = 1;
        }

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-queue-full");
        let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        );
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture should include verifying key bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture should include verifying key commitment");
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = Some(vk_box);
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let _saturated = app
            .zk_ivm_prove_slots
            .clone()
            .try_acquire_owned()
            .expect("acquire slot");

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let body = norito::json::to_vec(&req).expect("json encode request");
        let err = match handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        {
            Ok(_) => panic!("queue full should be rejected"),
            Err(err) => err,
        };

        match err {
            Error::ProofRateLimited { endpoint, .. } => assert_eq!(endpoint, "v1/zk/ivm/prove"),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(
            app.zk_ivm_prove_jobs.is_empty(),
            "rejected request must not create a job entry"
        );
    }

    #[tokio::test]
    async fn zk_ivm_prove_delete_cancels_and_frees_capacity_slot() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(1));
            // Force the job to remain queued so cancellation is deterministic.
            state.zk_ivm_prove_inflight = Arc::new(tokio::sync::Semaphore::new(0));
            state.zk_ivm_prove_slots_total = 1;
            state.zk_ivm_prove_inflight_total = 0;
        }

        let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-cancel");
        let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        );
        let vk_box = fixture
            .vk_box("halo2/ipa")
            .expect("fixture should include verifying key bytes");
        let vk_commitment = fixture
            .vk_hash("halo2/ipa")
            .expect("fixture should include verifying key commitment");
        let mut vk_record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            fixture.schema_hash,
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.max_proof_bytes = 8 * 1024 * 1024;
        vk_record.key = Some(vk_box);
        vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;

        {
            let height = next_block_height(&app);
            let header = BlockHeader::new(
                NonZeroU64::new(height).expect("height>0"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut block = app.state.block(header);
            let mut stx = block.transaction();
            stx.world
                .verifying_keys_mut_for_testing()
                .insert(vk_id.clone(), vk_record);
            stx.apply();
            block.transactions.insert_block(
                HashSet::new(),
                NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
            );
            block.commit().expect("commit should persist vk record");
        }

        let meta = ivm::ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            ..Default::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
        let bytecode = IvmBytecode::from_compiled(program);
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let req_body = norito::json::to_vec(&req).expect("json encode request");

        let response = handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(req_body.clone()),
        )
        .await
        .expect("first submission ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let resp_body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let created: ZkIvmProveJobCreatedDto =
            norito::json::from_slice(&resp_body).expect("json decode created dto");
        let job_id = created.job_id;

        let err = match handler_zk_ivm_prove(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::body::Bytes::from(req_body.clone()),
        )
        .await
        {
            Ok(_) => panic!("second submission should be rate limited due to capacity slot"),
            Err(err) => err,
        };
        assert!(
            matches!(err, Error::ProofRateLimited { endpoint, .. } if endpoint == "v1/zk/ivm/prove")
        );

        let response = handler_zk_ivm_prove_delete(
            State(app.clone()),
            negotiated(&app),
            HeaderMap::new(),
            axum::extract::Path(job_id),
        )
        .await
        .expect("prove delete ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        // Allow the cancellation signal to reach the queued task so it can drop the slot permit.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            app.zk_ivm_prove_slots.available_permits(),
            1,
            "capacity slot should be released after delete cancels a queued job"
        );
    }

    #[tokio::test]
    async fn alias_voprf_invalid_hex_returns_error() {
        let err =
            match handler_alias_voprf_evaluate(NoritoJson(routing::AliasVoprfEvaluateRequestDto {
                blinded_element_hex: "zz".to_string(),
            }))
            .await
            {
                Ok(_) => panic!("invalid hex should error"),
                Err(err) => err,
            };

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        use iroha_data_model::{ValidationFail, query::error::QueryExecutionFail};

        let validation: ValidationFail =
            norito::decode_from_bytes(&body).expect("validation fail payload");
        match validation {
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(message)) => {
                assert!(
                    message.to_lowercase().contains("invalid"),
                    "unexpected conversion message: {message}"
                );
            }
            other => panic!("unexpected validation error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn alias_voprf_oversized_payload_rejected() {
        let oversized = "aa".repeat(5000);
        let err =
            match handler_alias_voprf_evaluate(NoritoJson(routing::AliasVoprfEvaluateRequestDto {
                blinded_element_hex: oversized,
            }))
            .await
            {
                Ok(_) => panic!("oversized payload should error"),
                Err(err) => err,
            };

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        use iroha_data_model::{ValidationFail, query::error::QueryExecutionFail};

        let validation: ValidationFail =
            norito::decode_from_bytes(&body).expect("validation fail payload");
        assert!(matches!(
            validation,
            ValidationFail::QueryFailed(QueryExecutionFail::Conversion(_))
        ));
    }

    #[tokio::test]
    async fn alias_resolve_index_requires_runtime() {
        let app = mk_app_state_for_tests();
        let response = handler_alias_resolve_index(
            State(app),
            NoritoJson(routing::AliasResolveIndexRequestDto { index: 0 }),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_alias_record() {
        let account_id: AccountId = format!("{}@wonderland", KeyPair::random().public_key())
            .parse()
            .expect("valid account id");
        let alias = "GB82 WEST 1234 5698 7654 32";
        let iso_cfg = sample_iso_bridge_config(alias, &account_id);
        let app = mk_app_state_for_tests_with_iso_bridge(Some(iso_cfg));

        let response = handler_alias_resolve_index(
            State(app),
            NoritoJson(routing::AliasResolveIndexRequestDto { index: 0 }),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: routing::AliasResolveIndexResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(dto.index, 0);
        assert_eq!(dto.alias, "GB82WEST12345698765432");
        assert_eq!(dto.account_id, account_id.to_string());
        assert_eq!(dto.source.as_deref(), Some("alias_service"));
    }

    #[tokio::test]
    async fn alias_resolve_non_account_target_returns_not_implemented() {
        let mut app = mk_app_state_for_tests();
        let owner: AccountId = format!("{}@wonderland", KeyPair::random().public_key())
            .parse()
            .expect("valid account id");
        let alias_name = Name::from_str("CUSTOM").expect("valid alias name");

        let service = {
            let service = AliasService::new();
            let record = AliasRecord::new(
                alias_name.clone(),
                owner,
                AliasTarget::Custom(vec![0xDE, 0xAD]),
                AliasIndex(0),
            );
            service
                .storage()
                .put(record)
                .expect("alias insertion should succeed");
            service
        };

        Arc::get_mut(&mut app)
            .expect("unique app state")
            .alias_service = Some(Arc::new(service));

        let response = handler_alias_resolve(
            State(app.clone()),
            NoritoJson(routing::AliasResolveRequestDto {
                alias: "custom".to_string(),
            }),
        )
        .await
        .expect("handler should succeed")
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let err: routing::AliasErrorResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert!(
            err.error
                .contains("alias targets other than accounts are not supported"),
            "unexpected error message: {}",
            err.error
        );

        let response_index = handler_alias_resolve_index(
            State(app),
            NoritoJson(routing::AliasResolveIndexRequestDto { index: 0 }),
        )
        .await
        .expect("handler should succeed")
        .into_response();
        assert_eq!(response_index.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn validate_api_token_rejects_missing_or_unconfigured() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::new());

        let headers = HeaderMap::new();
        assert!(validate_api_token(state, &headers).is_err());

        let mut configured_headers = HeaderMap::new();
        configured_headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        let mut tokens = HashSet::new();
        tokens.insert("secret".to_string());
        state.api_tokens_set = Arc::new(tokens);
        assert!(validate_api_token(state, &configured_headers).is_ok());
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn norito_rpc_gate_records_metrics() {
        let cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: false,
            stage: actual::NoritoRpcStage::Canary,
            allowed_clients: vec!["ok".into()],
        };
        let (app, metrics) = mk_norito_rpc_test_harness(cfg.clone()).await;

        let mut headers = HeaderMap::new();
        headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("ok"));
        app.check_norito_rpc_allowed(&headers)
            .expect("canary token should be allowed");
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[cfg.stage.label(), "allowed"])
                .get(),
            1
        );

        let missing_token_headers = HeaderMap::new();
        assert!(
            app.check_norito_rpc_allowed(&missing_token_headers)
                .is_err()
        );
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[cfg.stage.label(), "canary_missing_token"])
                .get(),
            1
        );

        let mut wrong_token_headers = HeaderMap::new();
        wrong_token_headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("wrong"));
        assert!(app.check_norito_rpc_allowed(&wrong_token_headers).is_err());
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[cfg.stage.label(), "canary_denied"])
                .get(),
            1
        );

        let mtls_cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: true,
            stage: actual::NoritoRpcStage::Ga,
            allowed_clients: Vec::new(),
        };
        let (mtls_app, mtls_metrics) = mk_norito_rpc_test_harness(mtls_cfg.clone()).await;
        assert!(
            mtls_app
                .check_norito_rpc_allowed(&HeaderMap::new())
                .is_err()
        );
        assert_eq!(
            mtls_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[mtls_cfg.stage.label(), "mtls_required"])
                .get(),
            1
        );
        let mut mtls_headers = HeaderMap::new();
        mtls_headers.insert(HEADER_MTLS_FORWARD, HeaderValue::from_static("present"));
        mtls_app
            .check_norito_rpc_allowed(&mtls_headers)
            .expect("mtls header should allow RPC");
        assert_eq!(
            mtls_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[mtls_cfg.stage.label(), "allowed"])
                .get(),
            1
        );

        let disabled_cfg = actual::NoritoRpcTransport::default();
        let (disabled_app, disabled_metrics) =
            mk_norito_rpc_test_harness(disabled_cfg.clone()).await;
        assert!(
            disabled_app
                .check_norito_rpc_allowed(&HeaderMap::new())
                .is_err()
        );
        assert_eq!(
            disabled_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[actual::NoritoRpcStage::Disabled.label(), "disabled"])
                .get(),
            1
        );
    }

    #[tokio::test]
    async fn rpc_capabilities_reflect_transport_config() {
        let cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: true,
            stage: actual::NoritoRpcStage::Canary,
            allowed_clients: vec!["alpha".into(), "beta".into()],
        };
        let app = mk_app_state_for_tests_with_options(None, None, Some(cfg.clone()), None);

        let response = app.rpc_capabilities();
        assert!(response.norito_rpc.enabled);
        assert!(response.norito_rpc.require_mtls);
        assert_eq!(
            response.norito_rpc.stage,
            cfg.stage.label(),
            "stage label should match config"
        );
        assert_eq!(
            response.norito_rpc.canary_allowlist_size,
            cfg.allowed_clients.len()
        );
    }

    #[tokio::test]
    async fn rpc_ping_reuses_capabilities_advert() {
        let cfg = actual::NoritoRpcTransport {
            enabled: false,
            require_mtls: false,
            stage: actual::NoritoRpcStage::Ga,
            allowed_clients: Vec::new(),
        };
        let app = mk_app_state_for_tests_with_options(None, None, Some(cfg.clone()), None);

        let response = app.rpc_ping();
        assert!(response.ok);
        assert!(response.unix_time_ms > 0);
        assert_eq!(
            response.norito_rpc.stage,
            cfg.stage.label(),
            "ping should advertise the same stage"
        );
        assert_eq!(response.norito_rpc.canary_allowlist_size, 0);
    }

    #[tokio::test]
    async fn error_response_contains_details() {
        let err = Error::AcceptTransaction(iroha_core::tx::AcceptTransactionFail::ChainIdMismatch(
            iroha_data_model::isi::error::Mismatch {
                expected: "123".into(),
                actual: "321".into(),
            },
        ));
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(|value| value.to_str().unwrap())
            .expect("content-type header present");
        assert_eq!(content_type, super::utils::NORITO_MIME_TYPE);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload = norito::decode_from_bytes::<super::ErrorEnvelope>(&body).unwrap();
        assert_eq!(payload.code(), "transaction_rejected");
        assert_eq!(
            payload.message(),
            "failed to accept transaction: Chain id doesn't correspond to the id of current blockchain: Expected ChainId(\"123\"), actual ChainId(\"321\")"
        );
    }

    #[test]
    fn start_server_maps_to_internal_server_error() {
        assert_eq!(
            Error::StartServer.status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn failed_exit_maps_to_internal_server_error() {
        assert_eq!(
            Error::FailedExit.status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
#[test]
fn conn_scheme_detects_norito_rpc() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .header(
            axum::http::header::CONTENT_TYPE,
            crate::utils::NORITO_MIME_TYPE,
        )
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::NoritoRpc
    ));
}

#[test]
fn conn_scheme_marks_transaction_path_as_norito_rpc() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri(iroha_torii_shared::uri::TRANSACTION)
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::NoritoRpc
    ));
}

#[test]
fn conn_scheme_labels_use_norito_rpc_name() {
    assert_eq!(ConnScheme::NoritoRpc.label(), "norito_rpc");
}

#[test]
fn conn_scheme_defaults_to_http_for_json() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(())
        .unwrap();
    assert!(matches!(
        ConnScheme::from_request(&request),
        ConnScheme::Http
    ));
}

#[test]
fn conn_scheme_flags_websocket_upgrade() {
    let request = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .header(axum::http::header::UPGRADE, "websocket")
        .body(())
        .unwrap();
    assert!(matches!(ConnScheme::from_request(&request), ConnScheme::Ws));
}

#[cfg(all(test, feature = "app_api", feature = "telemetry"))]
mod peer_telemetry_tests {
    use std::{collections::HashSet, str::FromStr};

    use iroha_crypto::KeyPair;
    use iroha_data_model::peer::Peer;
    use tokio::sync::watch;

    use super::{OnlinePeersProvider, collect_peer_urls};
    use crate::telemetry::peers::ToriiUrl;

    #[test]
    fn collect_peer_urls_requires_configured_urls() {
        let (tx, rx) = watch::channel(HashSet::new());
        let provider = OnlinePeersProvider::new(rx);

        let peer_a = Peer::new(
            "127.0.0.1:1337".parse().expect("valid socket address"),
            KeyPair::random().public_key().clone(),
        );
        let peer_b = Peer::new(
            "10.0.0.5:8080".parse().expect("valid socket address"),
            KeyPair::random().public_key().clone(),
        );

        tx.send(HashSet::from([peer_a.clone(), peer_b.clone()]))
            .expect("watch update should succeed");

        let urls = collect_peer_urls(&provider, &[]);

        assert!(
            urls.is_empty(),
            "no configured URLs means no peer telemetry"
        );
    }

    #[test]
    fn collect_peer_urls_prefers_configured_urls() {
        let (tx, rx) = watch::channel(HashSet::new());
        let provider = OnlinePeersProvider::new(rx);

        let peer = Peer::new(
            "127.0.0.1:1337".parse().expect("valid socket address"),
            KeyPair::random().public_key().clone(),
        );
        tx.send(HashSet::from([peer]))
            .expect("watch update should succeed");

        let configured = vec![
            ToriiUrl::from_str("http://127.0.0.1:8080").expect("valid torii url"),
            ToriiUrl::from_str("http://127.0.0.1:8080").expect("duplicate torii url"),
            ToriiUrl::from_str("http://127.0.0.1:8081").expect("valid torii url"),
        ];
        let mut expected = configured.clone();
        expected.sort();
        expected.dedup();

        let urls = collect_peer_urls(&provider, &configured);

        assert_eq!(
            urls, expected,
            "configured URLs should override peer-derived telemetry targets"
        );
    }
}

#[cfg(feature = "app_api")]
fn gateway_denylist_policy_from_config(
    config: &iroha_config::parameters::actual::SorafsGatewayDenylist,
) -> sorafs::gateway::DenylistPolicy {
    sorafs::gateway::DenylistPolicy::new(
        config.standard_ttl,
        config.emergency_ttl,
        config.emergency_review_window,
        config.require_governance_reference,
    )
}

#[cfg(feature = "app_api")]
fn finalize_gateway_denylist_entry(
    builder: sorafs::gateway::DenylistEntryBuilder,
    meta: GatewayDenylistPolicyMeta,
    policy: &sorafs::gateway::DenylistPolicy,
) -> Result<sorafs::gateway::DenylistEntry, String> {
    builder
        .policy_tier(meta.tier)
        .canon(meta.canon)
        .governance_reference(meta.governance_reference)
        .build_with_policy(policy)
}

#[cfg(feature = "app_api")]
fn parse_policy_tier(value: Option<&str>) -> Result<sorafs::gateway::DenylistPolicyTier, String> {
    let Some(label) = value.map(|raw| raw.trim().to_ascii_lowercase()) else {
        return Ok(sorafs::gateway::DenylistPolicyTier::Standard);
    };
    match label.as_str() {
        "standard" | "default" => Ok(sorafs::gateway::DenylistPolicyTier::Standard),
        "emergency" => Ok(sorafs::gateway::DenylistPolicyTier::Emergency),
        "permanent" => Ok(sorafs::gateway::DenylistPolicyTier::Permanent),
        other => Err(format!(
            "unsupported denylist policy tier `{other}`; expected standard|emergency|permanent"
        )),
    }
}
