//! Criterion benchmarks for Torii hot paths.
#![cfg(feature = "app_api")]

use std::{
    borrow::Cow,
    net::SocketAddr,
    num::NonZeroUsize,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::Path,
    http::{Method, Request, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use criterion::{BatchSize, BenchmarkId, Criterion};
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::{
    block::{BlockBuilder, ValidBlock},
    kura::Kura,
    query::store::{LiveQueryStore, LiveQueryStoreHandle},
    queue::Queue,
    state::{State, StateReadOnly, World},
    telemetry::{StateTelemetry, Telemetry},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    SignedQuery,
    account::rekey::{AccountAlias, AccountAliasDomain},
    name::Name,
    prelude::*,
    query::{
        ErasedIterQuery, QueryBox, QueryOutputBatchBox, QueryResponse, QueryWithParams,
        dsl::{CompoundPredicate, SelectorTuple},
        parameters::{FetchSize, ForwardCursor, QueryParams},
    },
    query::{QueryRequest, SingularQueryBox, executor::prelude::FindParameters},
};
use iroha_logger::Level;
use iroha_primitives::{const_vec::ConstVec, numeric::Numeric};
use iroha_telemetry::metrics::Metrics;
use iroha_torii::{
    BenchRateLimiter, ContractActivityGetParamsForBench, MaybeTelemetry, NoritoJson, NoritoQuery,
    QueryOptions, ResponseFormat, accept_transaction_for_ingress_for_bench,
    filter::{
        AggregateFn, AggregateMetric, AggregateSpec, FieldPath, FilterExpr, Order, Pagination,
        QueryEnvelope, Selector, SortKey,
    },
    handle_queries_with_opts, handle_transaction_with_metrics_for_bench,
    handle_v1_account_assets_query_for_bench, handle_v1_accounts_query_for_bench,
    handle_v1_asset_holders_query_for_bench, handle_v1_contracts_activity_get_for_bench,
    profile_stats::print_profile,
    query_load_profiles::{QueryLoadProfile, QueryLoadWorkload, standard_query_load_profiles},
    verify_signed_query_request_for_bench,
};
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};
use tower::ServiceExt as _;

fn direct_metrics_telemetry() -> MaybeTelemetry {
    let metrics = Arc::new(Metrics::default());
    let telemetry = Telemetry::from(StateTelemetry::new(metrics, true));
    MaybeTelemetry::from_profile(Some(telemetry), TelemetryProfile::Operator)
}

fn signed_find_parameters(key_pair: &KeyPair) -> SignedQuery {
    let authority = AccountId::new(key_pair.public_key().clone());
    QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters))
        .with_authority(authority)
        .sign(key_pair)
}

fn deterministic_key_pair(label: &str) -> KeyPair {
    let seed: Vec<u8> = label.as_bytes().iter().copied().cycle().take(32).collect();
    KeyPair::try_from_seed(seed, Algorithm::Ed25519).expect("derive Torii benchmark key")
}

fn query_load_domain_id() -> DomainId {
    DomainId::try_new("querybench", "universal").expect("query bench domain")
}

fn query_load_asset_definition_id(index: usize) -> AssetDefinitionId {
    AssetDefinitionId::new(
        query_load_domain_id(),
        format!("coin{index}")
            .parse()
            .expect("query bench asset name"),
    )
}

fn query_load_account_alias(index: usize) -> AccountAlias {
    AccountAlias::new(
        Name::from_str(&format!("user{index}")).expect("alias label"),
        Some(AccountAliasDomain::new(
            Name::from_str(&format!("bank{}", index % 8)).expect("alias domain"),
        )),
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    )
}

fn query_load_account_id(index: usize) -> AccountId {
    let key_pair = deterministic_key_pair(&format!("query-load-account-{index}"));
    AccountId::new(key_pair.public_key().clone())
}

fn query_load_contract_authority_key_pair(index: usize) -> KeyPair {
    deterministic_key_pair(&format!("query-load-contract-authority-{index}"))
}

fn query_load_contract_authority_id(index: usize) -> AccountId {
    let key_pair = query_load_contract_authority_key_pair(index);
    AccountId::new(key_pair.public_key().clone())
}

const CONTRACT_ACTIVITY_CHAIN_ID: &str = "query-load-contract-activity";
const CONTRACT_ACTIVITY_BASE_TIMESTAMP_MS: u64 = 1_710_000_000_000;
const CONTRACT_ACTIVITY_MATCH_ALIAS: &str = "dlmm_router";
const CONTRACT_ACTIVITY_MATCH_ADDRESS: &str = "tairac1queryloadcontractdlmmrouter";
const CONTRACT_ACTIVITY_MATCH_ENTRYPOINT: &str = "route_swap";

struct QueryLoadFixture {
    state: Arc<State>,
    query_store: LiveQueryStoreHandle,
    authority: AccountId,
    key_pair: Arc<KeyPair>,
    account_asset_account_id: AccountId,
    asset_definition_id: AssetDefinitionId,
    contract_activity_authority: AccountId,
}

fn contract_activity_metadata(index: usize) -> Metadata {
    let mut metadata = Metadata::default();
    let matching = index.is_multiple_of(4);
    metadata.insert(
        "contract_address".parse().expect("metadata key"),
        Json::new(if matching {
            CONTRACT_ACTIVITY_MATCH_ADDRESS.to_owned()
        } else {
            format!("tairac1queryloadcontractnoise{}", index % 16)
        }),
    );
    metadata.insert(
        "contract_alias".parse().expect("metadata key"),
        Json::new(if matching {
            CONTRACT_ACTIVITY_MATCH_ALIAS.to_owned()
        } else {
            format!("noise_router_{}", index % 16)
        }),
    );
    metadata.insert(
        "contract_entrypoint".parse().expect("metadata key"),
        Json::new(if matching {
            CONTRACT_ACTIVITY_MATCH_ENTRYPOINT.to_owned()
        } else {
            format!("noise_entrypoint_{}", index % 8)
        }),
    );
    let amount_in = (index as u64).saturating_add(1);
    let min_out = index as u64;
    let input_is_base = index.is_multiple_of(2);
    metadata.insert(
        "contract_payload".parse().expect("metadata key"),
        Json::new(norito::json!({
            "amount_in": amount_in,
            "min_out": min_out,
            "input_is_base": input_is_base
        })),
    );
    metadata.insert(
        "gas_asset_id".parse().expect("metadata key"),
        Json::new("xor#universal"),
    );
    metadata.insert(
        "gas_limit".parse().expect("metadata key"),
        Json::new(100_000_u64),
    );
    metadata
}

fn contract_activity_accepted_transaction(
    chain_id: &ChainId,
    index: usize,
) -> AcceptedTransaction<'static> {
    let authority_index = if index.is_multiple_of(4) {
        0
    } else {
        (index % 7) + 1
    };
    let key_pair = query_load_contract_authority_key_pair(authority_index);
    let authority = AccountId::new(key_pair.public_key().clone());
    let mut metadata = contract_activity_metadata(index);
    metadata.insert(
        "fee_sponsor".parse().expect("metadata key"),
        Json::new(authority.to_string()),
    );
    let mut builder = TransactionBuilder::new(chain_id.clone(), authority);
    builder.set_creation_time(Duration::from_millis(
        CONTRACT_ACTIVITY_BASE_TIMESTAMP_MS + index as u64,
    ));
    let signed = builder
        .with_metadata(metadata)
        .with_executable(Executable::Instructions(ConstVec::from(Vec::<
            InstructionBox,
        >::new())))
        .sign(key_pair.private_key());
    AcceptedTransaction::new_unchecked(Cow::Owned(signed))
}

fn commit_contract_activity_transactions(state: &Arc<State>, profile: QueryLoadProfile) {
    if profile.committed_transactions == 0 {
        return;
    }
    let chain_id: ChainId = CONTRACT_ACTIVITY_CHAIN_ID
        .parse()
        .expect("contract activity chain id");
    let transactions = (0..profile.committed_transactions)
        .map(|index| contract_activity_accepted_transaction(&chain_id, index))
        .collect();
    let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let unverified = BlockBuilder::new(transactions)
        .chain(0, state.view().latest_block().as_deref())
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut state_block = state.block(unverified.header());
    let valid: ValidBlock = unverified
        .validate_and_record_transactions(&mut state_block)
        .unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    iroha_torii::test_utils::finalize_committed_block(state, state_block, committed);
}

fn build_query_load_fixture(profile: QueryLoadProfile) -> QueryLoadFixture {
    profile.validate().expect("valid query load profile");
    let query_store = LiveQueryStore::start_test();
    let domain_id = query_load_domain_id();
    let key_pair = Arc::new(deterministic_key_pair("query-load-authority"));
    let authority = AccountId::new(key_pair.public_key().clone());
    let authority_alias = AccountAlias::new(
        Name::from_str("authority").expect("authority alias name"),
        Some(AccountAliasDomain::new(
            Name::from_str("bench").expect("authority alias domain"),
        )),
        iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
    );

    let mut domains = Vec::with_capacity(profile.dataset_accounts + 1);
    domains.push(Domain::new(domain_id.clone()).build(&authority));
    for index in 0..profile.dataset_accounts {
        let domain_id =
            DomainId::try_new(format!("querybench{index}"), "universal").expect("bench domain id");
        domains.push(Domain::new(domain_id).build(&authority));
    }
    let mut accounts = Vec::with_capacity(profile.dataset_accounts + 1);
    accounts.push(
        Account::new(authority.clone())
            .with_label(Some(authority_alias))
            .build(&authority),
    );
    for index in 0..profile.dataset_accounts {
        let account_id = query_load_account_id(index);
        accounts.push(
            Account::new(account_id.clone())
                .with_label(Some(query_load_account_alias(index)))
                .build(&authority),
        );
    }

    let mut definition_ids = Vec::with_capacity(profile.assets_per_account);
    let mut definitions = Vec::with_capacity(profile.assets_per_account);
    for index in 0..profile.assets_per_account {
        let definition_id = query_load_asset_definition_id(index);
        definitions.push(
            AssetDefinition::numeric(definition_id.clone())
                .with_name(format!("Coin {index}"))
                .build(&authority),
        );
        definition_ids.push(definition_id);
    }

    let mut assets = Vec::with_capacity(profile.dataset_accounts * profile.assets_per_account);
    for account_index in 0..profile.dataset_accounts {
        let account_id = query_load_account_id(account_index);
        for (definition_index, definition_id) in definition_ids.iter().enumerate() {
            let asset_id = AssetId::new(definition_id.clone(), account_id.clone());
            let quantity = u32::try_from((account_index % 97) + definition_index + 1)
                .expect("quantity fits u32");
            assets.push(Asset::new(asset_id, Numeric::from(quantity)));
        }
    }

    let world = World::with_assets(domains, accounts, definitions, assets, []);
    let state = Arc::new(State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        query_store.clone(),
    ));
    commit_contract_activity_transactions(&state, profile);

    QueryLoadFixture {
        state,
        query_store,
        authority,
        key_pair,
        account_asset_account_id: query_load_account_id(0),
        asset_definition_id: definition_ids
            .into_iter()
            .next()
            .expect("at least one asset definition"),
        contract_activity_authority: query_load_contract_authority_id(0),
    }
}

fn signed_find_domains_query(
    authority: &AccountId,
    key_pair: &KeyPair,
    fetch_size: usize,
) -> SignedQuery {
    let fetch_size = u64::try_from(fetch_size)
        .ok()
        .and_then(std::num::NonZeroU64::new)
        .expect("non-zero fetch size");
    let payload =
        norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
    let qbox: QueryBox<QueryOutputBatchBox> = Box::new(ErasedIterQuery::<Domain>::new(
        CompoundPredicate::<Domain>::PASS,
        SelectorTuple::<Domain>::default(),
        payload,
    ));
    let iter = QueryWithParams::new(
        &qbox,
        QueryParams {
            fetch_size: FetchSize::new(Some(fetch_size)),
            ..QueryParams::default()
        },
    );
    QueryRequest::Start(iter)
        .with_authority(authority.clone())
        .sign(key_pair)
}

#[derive(Clone)]
struct HttpRequestTemplate {
    method: Method,
    uri: String,
    content_type: Option<&'static str>,
    body: Arc<Vec<u8>>,
}

impl HttpRequestTemplate {
    fn get(uri: impl Into<String>) -> Self {
        Self {
            method: Method::GET,
            uri: uri.into(),
            content_type: None,
            body: Arc::new(Vec::new()),
        }
    }

    fn json(uri: impl Into<String>, envelope: &QueryEnvelope) -> Self {
        Self {
            method: Method::POST,
            uri: uri.into(),
            content_type: Some("application/json"),
            body: Arc::new(norito::json::to_vec(envelope).expect("encode query envelope")),
        }
    }

    fn norito(uri: impl Into<String>, body: Vec<u8>) -> Self {
        Self {
            method: Method::POST,
            uri: uri.into(),
            content_type: Some("application/octet-stream"),
            body: Arc::new(body),
        }
    }

    fn request(&self) -> Request<Body> {
        let mut builder = Request::builder()
            .method(self.method.clone())
            .uri(self.uri.as_str());
        if let Some(content_type) = self.content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        builder
            .body(Body::from(self.body.as_ref().clone()))
            .expect("build benchmark request")
    }

    fn absolute_url(&self, base_url: &str) -> String {
        if self.uri.starts_with("http://") || self.uri.starts_with("https://") {
            return self.uri.clone();
        }
        format!("{base_url}{}", self.uri)
    }
}

struct SocketBenchServer {
    base_url: String,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
}

impl SocketBenchServer {
    async fn spawn(router: Router) -> Self {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind socket benchmark server");
        let addr = listener.local_addr().expect("socket benchmark address");
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("socket benchmark server");
        });
        Self {
            base_url: format!("http://{addr}"),
            shutdown: Some(shutdown_tx),
            handle,
        }
    }

    async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.handle.await.expect("socket benchmark server joined");
    }
}

fn response_body_cursor(body: &[u8]) -> Option<ForwardCursor> {
    let response: QueryResponse = norito::decode_from_bytes(body).expect("decode query response");
    let QueryResponse::Iterable(iterable) = response else {
        panic!("expected iterable query response");
    };
    let (_batch, _remaining, cursor) = iterable.into_parts();
    cursor
}

async fn send_http_request(router: Router, template: &HttpRequestTemplate) -> Vec<u8> {
    let response = router
        .oneshot(template.request())
        .await
        .expect("benchmark http request");
    let status = response.status();
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("collect benchmark response")
        .to_bytes()
        .to_vec();
    assert_eq!(
        status,
        StatusCode::OK,
        "benchmark request failed: {}",
        String::from_utf8_lossy(&body)
    );
    std::hint::black_box(body.clone())
}

async fn send_socket_request(
    client: &reqwest::Client,
    base_url: &str,
    template: &HttpRequestTemplate,
) -> Vec<u8> {
    let method =
        reqwest::Method::from_bytes(template.method.as_str().as_bytes()).expect("http method");
    let mut request = client.request(method, template.absolute_url(base_url));
    if let Some(content_type) = template.content_type {
        request = request.header(reqwest::header::CONTENT_TYPE, content_type);
    }
    if !template.body.is_empty() {
        request = request.body(template.body.as_ref().clone());
    }
    let response = request.send().await.expect("socket benchmark http request");
    let status = response.status();
    let body = response
        .bytes()
        .await
        .expect("collect socket benchmark response")
        .to_vec();
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "socket benchmark request failed: {}",
        String::from_utf8_lossy(&body)
    );
    std::hint::black_box(body.clone())
}

fn signed_query_router(fixture: &QueryLoadFixture) -> Router {
    let query_store = fixture.query_store.clone();
    let state = Arc::clone(&fixture.state);
    let telemetry = direct_metrics_telemetry();
    Router::new().route(
        "/query",
        post(move |body: Bytes| {
            let query_store = query_store.clone();
            let state = Arc::clone(&state);
            let telemetry = telemetry.clone();
            async move {
                let Ok(query) = SignedQuery::decode_all_versioned(body.as_ref()) else {
                    return (
                        StatusCode::BAD_REQUEST,
                        "invalid versioned signed query payload",
                    )
                        .into_response();
                };
                match handle_queries_with_opts(
                    query_store,
                    state,
                    query,
                    telemetry,
                    NoritoQuery(QueryOptions {
                        cursor_mode: Some("stored".to_owned()),
                        count_mode: Some("bounded".to_owned()),
                        gas_units: None,
                    }),
                    ResponseFormat::Norito,
                )
                .await
                {
                    Ok(response) => response.into_response(),
                    Err(error) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                    }
                }
            }
        }),
    )
}

fn accounts_query_router(fixture: &QueryLoadFixture) -> Router {
    let state = Arc::clone(&fixture.state);
    let telemetry = direct_metrics_telemetry();
    Router::new().route(
        "/v1/accounts/query",
        post(move |NoritoJson(envelope): NoritoJson<QueryEnvelope>| {
            let state = Arc::clone(&state);
            let telemetry = telemetry.clone();
            async move {
                match handle_v1_accounts_query_for_bench(state, NoritoJson(envelope), telemetry)
                    .await
                {
                    Ok(response) => response.into_response(),
                    Err(error) => {
                        (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                    }
                }
            }
        }),
    )
}

fn account_assets_query_router(fixture: &QueryLoadFixture) -> Router {
    let state = Arc::clone(&fixture.state);
    let telemetry = direct_metrics_telemetry();
    Router::new().route(
        "/v1/accounts/{account_id}/assets/query",
        post(
            move |Path(account_id): Path<String>,
                  NoritoJson(envelope): NoritoJson<QueryEnvelope>| {
                let state = Arc::clone(&state);
                let telemetry = telemetry.clone();
                async move {
                    match handle_v1_account_assets_query_for_bench(
                        state,
                        Path(account_id),
                        NoritoJson(envelope),
                        telemetry,
                    )
                    .await
                    {
                        Ok(response) => response.into_response(),
                        Err(error) => {
                            (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                        }
                    }
                }
            },
        ),
    )
}

fn asset_holders_query_router(fixture: &QueryLoadFixture) -> Router {
    let state = Arc::clone(&fixture.state);
    let telemetry = direct_metrics_telemetry();
    Router::new().route(
        "/v1/assets/{definition_id}/holders/query",
        post(
            move |Path(definition_id): Path<String>,
                  NoritoJson(envelope): NoritoJson<QueryEnvelope>| {
                let state = Arc::clone(&state);
                let telemetry = telemetry.clone();
                async move {
                    match handle_v1_asset_holders_query_for_bench(
                        state,
                        Path(definition_id),
                        NoritoJson(envelope),
                        telemetry,
                    )
                    .await
                    {
                        Ok(response) => response.into_response(),
                        Err(error) => {
                            (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                        }
                    }
                }
            },
        ),
    )
}

fn contracts_activity_router(fixture: &QueryLoadFixture) -> Router {
    let state = Arc::clone(&fixture.state);
    let telemetry = direct_metrics_telemetry();
    Router::new().route(
        "/v1/contracts/activity",
        get(
            move |NoritoQuery(params): NoritoQuery<ContractActivityGetParamsForBench>| {
                let state = Arc::clone(&state);
                let telemetry = telemetry.clone();
                async move {
                    match handle_v1_contracts_activity_get_for_bench(
                        state,
                        NoritoQuery(params),
                        telemetry,
                    )
                    .await
                    {
                        Ok(response) => response.into_response(),
                        Err(error) => {
                            (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
                        }
                    }
                }
            },
        ),
    )
}

fn account_alias_projection_envelope(profile: QueryLoadProfile) -> QueryEnvelope {
    QueryEnvelope {
        query: Some("accounts_alias_projection".to_owned()),
        filter: Some(FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(true),
        )),
        select: Some(Selector(vec![
            FieldPath("id".to_owned()),
            FieldPath("primary_alias".to_owned()),
            FieldPath("primary_alias_domain".to_owned()),
            FieldPath("has_primary_alias".to_owned()),
        ])),
        aggregate: None,
        sort: vec![
            SortKey {
                key: FieldPath("primary_alias_domain".to_owned()),
                order: Order::Asc,
            },
            SortKey {
                key: FieldPath("id".to_owned()),
                order: Order::Asc,
            },
        ],
        pagination: Pagination {
            limit: Some(profile.page_limit as u64),
            offset: 0,
        },
        fetch_size: Some(profile.fetch_size as u64),
        count_mode: Some("bounded".to_owned()),
    }
}

fn asset_holders_envelope(profile: QueryLoadProfile) -> QueryEnvelope {
    QueryEnvelope {
        query: Some("asset_holders".to_owned()),
        filter: Some(FilterExpr::Gt(
            FieldPath("quantity".to_owned()),
            norito::json::Value::from(0_u64),
        )),
        select: None,
        aggregate: None,
        sort: vec![SortKey {
            key: FieldPath("quantity".to_owned()),
            order: Order::Desc,
        }],
        pagination: Pagination {
            limit: Some(profile.page_limit as u64),
            offset: 0,
        },
        fetch_size: Some(profile.fetch_size as u64),
        count_mode: Some("bounded".to_owned()),
    }
}

fn account_assets_predicate_envelope(profile: QueryLoadProfile) -> QueryEnvelope {
    QueryEnvelope {
        query: Some("account_assets_predicate".to_owned()),
        filter: Some(FilterExpr::And(vec![
            FilterExpr::Gt(
                FieldPath("quantity".to_owned()),
                norito::json::Value::from(0_u64),
            ),
            FilterExpr::Eq(
                FieldPath("scope".to_owned()),
                norito::json::Value::from("global"),
            ),
            FilterExpr::Eq(
                FieldPath("has_primary_alias".to_owned()),
                norito::json::Value::from(true),
            ),
        ])),
        select: None,
        aggregate: None,
        sort: vec![
            SortKey {
                key: FieldPath("quantity".to_owned()),
                order: Order::Desc,
            },
            SortKey {
                key: FieldPath("asset".to_owned()),
                order: Order::Asc,
            },
        ],
        pagination: Pagination {
            limit: Some(profile.page_limit as u64),
            offset: 0,
        },
        fetch_size: Some(profile.fetch_size as u64),
        count_mode: Some("bounded".to_owned()),
    }
}

fn contracts_activity_uri(fixture: &QueryLoadFixture, profile: QueryLoadProfile) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("limit", &profile.page_limit.to_string());
    serializer.append_pair("count_mode", "bounded");
    serializer.append_pair(
        "authority",
        &fixture.contract_activity_authority.to_string(),
    );
    serializer.append_pair("contract_alias", CONTRACT_ACTIVITY_MATCH_ALIAS);
    serializer.append_pair("contract_entrypoint", CONTRACT_ACTIVITY_MATCH_ENTRYPOINT);
    serializer.append_pair("result_ok", "true");
    serializer.append_pair(
        "since_timestamp_ms",
        &(CONTRACT_ACTIVITY_BASE_TIMESTAMP_MS + (profile.committed_transactions as u64 / 2))
            .to_string(),
    );
    format!("/v1/contracts/activity?{}", serializer.finish())
}

fn generic_aggregate_envelope(profile: QueryLoadProfile) -> QueryEnvelope {
    QueryEnvelope {
        query: Some("accounts_generic_aggregate".to_owned()),
        filter: Some(FilterExpr::Eq(
            FieldPath("has_primary_alias".to_owned()),
            norito::json::Value::from(true),
        )),
        select: None,
        aggregate: Some(AggregateSpec {
            group_by: vec![FieldPath("primary_alias_domain".to_owned())],
            metrics: vec![
                AggregateMetric {
                    alias: "accounts".to_owned(),
                    r#fn: AggregateFn::Count,
                    field: None,
                },
                AggregateMetric {
                    alias: "distinct_aliases".to_owned(),
                    r#fn: AggregateFn::DistinctCount,
                    field: Some(FieldPath("primary_alias".to_owned())),
                },
            ],
            having: None,
        }),
        sort: vec![SortKey {
            key: FieldPath("primary_alias_domain".to_owned()),
            order: Order::Asc,
        }],
        pagination: Pagination {
            limit: Some(profile.page_limit as u64),
            offset: 0,
        },
        fetch_size: Some(profile.fetch_size as u64),
        count_mode: Some("bounded".to_owned()),
    }
}

async fn run_app_http_profile(
    profile: QueryLoadProfile,
    router: Router,
    template: HttpRequestTemplate,
) -> Duration {
    for _ in 0..profile.warmup_ops {
        let body = send_http_request(router.clone(), &template).await;
        std::hint::black_box(body.len());
    }

    let next = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let mut workers = Vec::with_capacity(profile.concurrency);
    for _ in 0..profile.concurrency {
        let next = Arc::clone(&next);
        let router = router.clone();
        let template = template.clone();
        workers.push(tokio::spawn(async move {
            let mut completed = 0usize;
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= profile.measured_ops {
                    break;
                }
                let body = send_http_request(router.clone(), &template).await;
                std::hint::black_box(body.len());
                completed += 1;
            }
            completed
        }));
    }
    let mut completed = 0usize;
    for worker in workers {
        completed += worker.await.expect("query worker joined");
    }
    assert_eq!(completed, profile.measured_ops);
    started.elapsed()
}

async fn run_app_socket_profile(
    profile: QueryLoadProfile,
    client: reqwest::Client,
    base_url: String,
    template: HttpRequestTemplate,
) -> Duration {
    for _ in 0..profile.warmup_ops {
        let body = send_socket_request(&client, &base_url, &template).await;
        std::hint::black_box(body.len());
    }

    let next = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let mut workers = Vec::with_capacity(profile.concurrency);
    for _ in 0..profile.concurrency {
        let next = Arc::clone(&next);
        let client = client.clone();
        let base_url = base_url.clone();
        let template = template.clone();
        workers.push(tokio::spawn(async move {
            let mut completed = 0usize;
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= profile.measured_ops {
                    break;
                }
                let body = send_socket_request(&client, &base_url, &template).await;
                std::hint::black_box(body.len());
                completed += 1;
            }
            completed
        }));
    }
    let mut completed = 0usize;
    for worker in workers {
        completed += worker.await.expect("socket query worker joined");
    }
    assert_eq!(completed, profile.measured_ops);
    started.elapsed()
}

async fn run_signed_http_operation(
    router: Router,
    query_store: LiveQueryStoreHandle,
    authority: AccountId,
    key_pair: Arc<KeyPair>,
    start_template: HttpRequestTemplate,
    continuation_depth: usize,
) {
    let body = send_http_request(router.clone(), &start_template).await;
    let mut cursor = response_body_cursor(&body).expect("first batch exposes stored cursor");
    for _ in 0..continuation_depth {
        let signed = QueryRequest::Continue(cursor)
            .with_authority(authority.clone())
            .sign(key_pair.as_ref());
        let template = HttpRequestTemplate::norito("/query", signed.encode_versioned());
        let body = send_http_request(router.clone(), &template).await;
        let next_cursor = response_body_cursor(&body);
        if let Some(next_cursor) = next_cursor {
            cursor = next_cursor;
        } else {
            return;
        }
    }
    query_store.drop_query(&cursor.query);
}

async fn run_signed_http_profile(
    profile: QueryLoadProfile,
    fixture: &QueryLoadFixture,
    router: Router,
) -> Duration {
    let start_query = signed_find_domains_query(
        &fixture.authority,
        fixture.key_pair.as_ref(),
        profile.fetch_size,
    );
    let start_template = HttpRequestTemplate::norito("/query", start_query.encode_versioned());

    for _ in 0..profile.warmup_ops {
        run_signed_http_operation(
            router.clone(),
            fixture.query_store.clone(),
            fixture.authority.clone(),
            Arc::clone(&fixture.key_pair),
            start_template.clone(),
            profile.continuation_depth,
        )
        .await;
    }

    let next = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let mut workers = Vec::with_capacity(profile.concurrency);
    for _ in 0..profile.concurrency {
        let next = Arc::clone(&next);
        let router = router.clone();
        let query_store = fixture.query_store.clone();
        let authority = fixture.authority.clone();
        let key_pair = Arc::clone(&fixture.key_pair);
        let start_template = start_template.clone();
        workers.push(tokio::spawn(async move {
            let mut completed = 0usize;
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= profile.measured_ops {
                    break;
                }
                run_signed_http_operation(
                    router.clone(),
                    query_store.clone(),
                    authority.clone(),
                    Arc::clone(&key_pair),
                    start_template.clone(),
                    profile.continuation_depth,
                )
                .await;
                completed += 1;
            }
            completed
        }));
    }
    let mut completed = 0usize;
    for worker in workers {
        completed += worker.await.expect("signed query worker joined");
    }
    assert_eq!(completed, profile.measured_ops);
    started.elapsed()
}

async fn run_signed_socket_operation(
    client: reqwest::Client,
    query_store: LiveQueryStoreHandle,
    base_url: String,
    authority: AccountId,
    key_pair: Arc<KeyPair>,
    start_template: HttpRequestTemplate,
    continuation_depth: usize,
) {
    let body = send_socket_request(&client, &base_url, &start_template).await;
    let mut cursor = response_body_cursor(&body).expect("first batch exposes stored cursor");
    for _ in 0..continuation_depth {
        let signed = QueryRequest::Continue(cursor)
            .with_authority(authority.clone())
            .sign(key_pair.as_ref());
        let template = HttpRequestTemplate::norito("/query", signed.encode_versioned());
        let body = send_socket_request(&client, &base_url, &template).await;
        let next_cursor = response_body_cursor(&body);
        if let Some(next_cursor) = next_cursor {
            cursor = next_cursor;
        } else {
            return;
        }
    }
    query_store.drop_query(&cursor.query);
}

async fn run_signed_socket_profile(
    profile: QueryLoadProfile,
    fixture: &QueryLoadFixture,
    client: reqwest::Client,
    base_url: String,
) -> Duration {
    let start_query = signed_find_domains_query(
        &fixture.authority,
        fixture.key_pair.as_ref(),
        profile.fetch_size,
    );
    let start_template = HttpRequestTemplate::norito("/query", start_query.encode_versioned());

    for _ in 0..profile.warmup_ops {
        run_signed_socket_operation(
            client.clone(),
            fixture.query_store.clone(),
            base_url.clone(),
            fixture.authority.clone(),
            Arc::clone(&fixture.key_pair),
            start_template.clone(),
            profile.continuation_depth,
        )
        .await;
    }

    let next = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let mut workers = Vec::with_capacity(profile.concurrency);
    for _ in 0..profile.concurrency {
        let next = Arc::clone(&next);
        let client = client.clone();
        let query_store = fixture.query_store.clone();
        let base_url = base_url.clone();
        let authority = fixture.authority.clone();
        let key_pair = Arc::clone(&fixture.key_pair);
        let start_template = start_template.clone();
        workers.push(tokio::spawn(async move {
            let mut completed = 0usize;
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= profile.measured_ops {
                    break;
                }
                run_signed_socket_operation(
                    client.clone(),
                    query_store.clone(),
                    base_url.clone(),
                    authority.clone(),
                    Arc::clone(&key_pair),
                    start_template.clone(),
                    profile.continuation_depth,
                )
                .await;
                completed += 1;
            }
            completed
        }));
    }
    let mut completed = 0usize;
    for worker in workers {
        completed += worker.await.expect("socket signed query worker joined");
    }
    assert_eq!(completed, profile.measured_ops);
    started.elapsed()
}

fn bench_signed_query_verify(c: &mut Criterion) {
    let key_pair = KeyPair::random();
    c.bench_function("torii_signed_query_verify_find_parameters", |b| {
        b.iter_batched(
            || signed_find_parameters(&key_pair),
            |signed_query| {
                let verified =
                    verify_signed_query_request_for_bench(signed_query).expect("query verifies");
                std::hint::black_box(verified);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_query_find_parameters(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let key_pair = KeyPair::random();
    let query_store = LiveQueryStore::start_test();
    let query_state = Arc::new(State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        query_store.clone(),
    ));
    let telemetry = direct_metrics_telemetry();

    c.bench_function("torii_query_find_parameters_norito", |b| {
        b.iter_batched(
            || signed_find_parameters(&key_pair),
            |signed_query| {
                let response = runtime
                    .block_on(handle_queries_with_opts(
                        query_store.clone(),
                        Arc::clone(&query_state),
                        signed_query,
                        telemetry.clone(),
                        NoritoQuery(QueryOptions::default()),
                        ResponseFormat::Norito,
                    ))
                    .expect("query completes");
                std::hint::black_box(response);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_transaction_admission(c: &mut Criterion) {
    let chain_id: Arc<ChainId> = Arc::new(
        "torii_hot_path_bench_chain"
            .parse()
            .expect("valid chain id"),
    );
    let tx_state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let tx_key_pair = KeyPair::random();
    let tx_authority = AccountId::new(tx_key_pair.public_key().clone());
    let telemetry = direct_metrics_telemetry();
    let counter = AtomicUsize::new(0);

    c.bench_function("torii_transaction_admission_direct_metrics", |b| {
        b.iter_batched(
            || {
                let index = counter.fetch_add(1, Ordering::Relaxed);
                let instruction = Log::new(Level::INFO, format!("torii-hot-path-bench-{index}"));
                TransactionBuilder::new(chain_id.as_ref().clone(), tx_authority.clone())
                    .with_instructions([InstructionBox::from(instruction)])
                    .sign(tx_key_pair.private_key())
            },
            |tx| {
                let accepted = accept_transaction_for_ingress_for_bench(
                    Arc::clone(&chain_id),
                    Arc::clone(&tx_state),
                    tx,
                    &telemetry,
                )
                .expect("transaction admission succeeds");
                std::hint::black_box(accepted.hash());
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_transaction_handle_enqueue(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let chain_id: Arc<ChainId> = Arc::new(
        "torii_hot_path_enqueue_bench_chain"
            .parse()
            .expect("valid chain id"),
    );
    let tx_state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let tx_key_pair = KeyPair::random();
    let tx_authority = AccountId::new(tx_key_pair.public_key().clone());
    let telemetry = direct_metrics_telemetry();
    let counter = AtomicUsize::new(0);
    let queue_capacity = NonZeroUsize::new(1_000_000).expect("non-zero queue capacity");
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: queue_capacity,
        capacity_per_user: queue_capacity,
        transaction_time_to_live: Duration::from_secs(60),
        ..Default::default()
    };

    c.bench_function("torii_transaction_handle_enqueue_direct_metrics", |b| {
        b.iter_batched(
            || {
                let index = counter.fetch_add(1, Ordering::Relaxed);
                let instruction =
                    Log::new(Level::INFO, format!("torii-hot-path-enqueue-bench-{index}"));
                let tx = TransactionBuilder::new(chain_id.as_ref().clone(), tx_authority.clone())
                    .with_instructions([InstructionBox::from(instruction)])
                    .sign(tx_key_pair.private_key());
                let (events, _) = tokio::sync::broadcast::channel(queue_capacity.get());
                let queue = Arc::new(Queue::from_config(queue_cfg, events));
                (queue, tx)
            },
            |(queue, tx)| {
                let decision = runtime
                    .block_on(handle_transaction_with_metrics_for_bench(
                        Arc::clone(&chain_id),
                        queue,
                        Arc::clone(&tx_state),
                        tx,
                        telemetry.clone(),
                        iroha_torii_shared::uri::TRANSACTION,
                    ))
                    .expect("transaction handle succeeds");
                std::hint::black_box(decision);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_transaction_enqueue_sustained_pressure(c: &mut Criterion) {
    let chain_id: Arc<ChainId> = Arc::new(
        "torii_hot_path_sustained_enqueue_bench_chain"
            .parse()
            .expect("valid chain id"),
    );
    let tx_state = Arc::new(State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let tx_key_pair = KeyPair::random();
    let tx_authority = AccountId::new(tx_key_pair.public_key().clone());
    let telemetry = direct_metrics_telemetry();
    let counter = AtomicUsize::new(0);
    let queue_capacity = NonZeroUsize::new(1_000_000).expect("non-zero queue capacity");
    let queue_cfg = iroha_config::parameters::actual::Queue {
        capacity: queue_capacity,
        capacity_per_user: queue_capacity,
        transaction_time_to_live: Duration::from_secs(600),
        ..Default::default()
    };

    let make_tx = |index: usize| {
        let instruction = Log::new(
            Level::INFO,
            format!("torii-hot-path-sustained-enqueue-bench-{index}"),
        );
        TransactionBuilder::new(chain_id.as_ref().clone(), tx_authority.clone())
            .with_instructions([InstructionBox::from(instruction)])
            .sign(tx_key_pair.private_key())
    };

    let mut group = c.benchmark_group("torii_transaction_enqueue_sustained_pressure");
    for backlog in [1_000usize, 10_000, 20_000] {
        let (events, _) = tokio::sync::broadcast::channel(queue_capacity.get());
        let queue = Arc::new(Queue::from_config(queue_cfg, events));

        for _ in 0..backlog {
            let index = counter.fetch_add(1, Ordering::Relaxed);
            let tx = make_tx(index);
            let accepted = accept_transaction_for_ingress_for_bench(
                Arc::clone(&chain_id),
                Arc::clone(&tx_state),
                tx,
                &telemetry,
            )
            .expect("prefill transaction admission succeeds");
            queue
                .push(accepted, tx_state.view())
                .expect("prefill enqueue succeeds");
        }

        group.bench_with_input(
            BenchmarkId::from_parameter(backlog),
            &backlog,
            |b, _backlog| {
                b.iter_batched(
                    || {
                        let index = counter.fetch_add(1, Ordering::Relaxed);
                        make_tx(index)
                    },
                    |tx| {
                        let accepted = accept_transaction_for_ingress_for_bench(
                            Arc::clone(&chain_id),
                            Arc::clone(&tx_state),
                            tx,
                            &telemetry,
                        )
                        .expect("transaction admission succeeds");
                        queue
                            .push(accepted, tx_state.view())
                            .expect("sustained enqueue succeeds");

                        let mut guards = Vec::new();
                        queue.get_transactions_for_block(
                            &tx_state.view(),
                            NonZeroUsize::new(1).expect("non-zero block limit"),
                            &mut guards,
                        );

                        std::hint::black_box(());
                        std::hint::black_box(queue.pressure_snapshot());
                        drop(guards);
                    },
                    BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

#[allow(clippy::too_many_lines)]
fn bench_query_http_sustained(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .expect("tokio runtime");
    let mut group = c.benchmark_group("torii_query_http_sustained");

    for profile in standard_query_load_profiles() {
        profile.validate().expect("valid built-in query profile");
        let fixture = build_query_load_fixture(profile);
        match profile.workload {
            QueryLoadWorkload::SignedIterableStoredContinuation => {
                let router = signed_query_router(&fixture);
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_signed_http_profile(
                                    *profile,
                                    &fixture,
                                    router.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
            }
            QueryLoadWorkload::AccountAliasProjection => {
                let router = accounts_query_router(&fixture);
                let template = HttpRequestTemplate::json(
                    "/v1/accounts/query",
                    &account_alias_projection_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_http_profile(
                                    *profile,
                                    router.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
            }
            QueryLoadWorkload::AccountAssetsPredicate => {
                let router = account_assets_query_router(&fixture);
                let template = HttpRequestTemplate::json(
                    format!(
                        "/v1/accounts/{}/assets/query",
                        fixture.account_asset_account_id
                    ),
                    &account_assets_predicate_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_http_profile(
                                    *profile,
                                    router.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
            }
            QueryLoadWorkload::AssetHolders => {
                let router = asset_holders_query_router(&fixture);
                let template = HttpRequestTemplate::json(
                    format!("/v1/assets/{}/holders/query", fixture.asset_definition_id),
                    &asset_holders_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_http_profile(
                                    *profile,
                                    router.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
            }
            QueryLoadWorkload::ContractActivityPredicate => {
                let router = contracts_activity_router(&fixture);
                let template = HttpRequestTemplate::get(contracts_activity_uri(&fixture, profile));
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_http_profile(
                                    *profile,
                                    router.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
            }
            QueryLoadWorkload::GenericAggregate => {
                let router = accounts_query_router(&fixture);
                let template = HttpRequestTemplate::json(
                    "/v1/accounts/query",
                    &generic_aggregate_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_http_profile(
                                    *profile,
                                    router.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
            }
        }
    }

    group.finish();
}

fn socket_profile_client(profile: QueryLoadProfile) -> reqwest::Client {
    reqwest::Client::builder()
        .pool_max_idle_per_host(profile.concurrency)
        .tcp_nodelay(true)
        .build()
        .expect("socket benchmark client")
}

#[allow(clippy::too_many_lines)]
fn bench_query_http_socket_sustained(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .expect("tokio runtime");
    let mut group = c.benchmark_group("torii_query_http_socket_sustained");

    for profile in standard_query_load_profiles() {
        profile.validate().expect("valid built-in query profile");
        let fixture = build_query_load_fixture(profile);
        let client = socket_profile_client(profile);
        match profile.workload {
            QueryLoadWorkload::SignedIterableStoredContinuation => {
                let server =
                    runtime.block_on(SocketBenchServer::spawn(signed_query_router(&fixture)));
                let base_url = server.base_url.clone();
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_signed_socket_profile(
                                    *profile,
                                    &fixture,
                                    client.clone(),
                                    base_url.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
                runtime.block_on(server.shutdown());
            }
            QueryLoadWorkload::AccountAliasProjection => {
                let server =
                    runtime.block_on(SocketBenchServer::spawn(accounts_query_router(&fixture)));
                let base_url = server.base_url.clone();
                let template = HttpRequestTemplate::json(
                    "/v1/accounts/query",
                    &account_alias_projection_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_socket_profile(
                                    *profile,
                                    client.clone(),
                                    base_url.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
                runtime.block_on(server.shutdown());
            }
            QueryLoadWorkload::AccountAssetsPredicate => {
                let server = runtime.block_on(SocketBenchServer::spawn(
                    account_assets_query_router(&fixture),
                ));
                let base_url = server.base_url.clone();
                let template = HttpRequestTemplate::json(
                    format!(
                        "/v1/accounts/{}/assets/query",
                        fixture.account_asset_account_id
                    ),
                    &account_assets_predicate_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_socket_profile(
                                    *profile,
                                    client.clone(),
                                    base_url.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
                runtime.block_on(server.shutdown());
            }
            QueryLoadWorkload::AssetHolders => {
                let server = runtime.block_on(SocketBenchServer::spawn(
                    asset_holders_query_router(&fixture),
                ));
                let base_url = server.base_url.clone();
                let template = HttpRequestTemplate::json(
                    format!("/v1/assets/{}/holders/query", fixture.asset_definition_id),
                    &asset_holders_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_socket_profile(
                                    *profile,
                                    client.clone(),
                                    base_url.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
                runtime.block_on(server.shutdown());
            }
            QueryLoadWorkload::ContractActivityPredicate => {
                let server = runtime.block_on(SocketBenchServer::spawn(contracts_activity_router(
                    &fixture,
                )));
                let base_url = server.base_url.clone();
                let template = HttpRequestTemplate::get(contracts_activity_uri(&fixture, profile));
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_socket_profile(
                                    *profile,
                                    client.clone(),
                                    base_url.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
                runtime.block_on(server.shutdown());
            }
            QueryLoadWorkload::GenericAggregate => {
                let server =
                    runtime.block_on(SocketBenchServer::spawn(accounts_query_router(&fixture)));
                let base_url = server.base_url.clone();
                let template = HttpRequestTemplate::json(
                    "/v1/accounts/query",
                    &generic_aggregate_envelope(profile),
                );
                group.bench_with_input(
                    BenchmarkId::new(profile.workload.label(), profile.name),
                    &profile,
                    |b, profile| {
                        b.iter_custom(|iters| {
                            let mut total = Duration::ZERO;
                            for _ in 0..iters {
                                total += runtime.block_on(run_app_socket_profile(
                                    *profile,
                                    client.clone(),
                                    base_url.clone(),
                                    template.clone(),
                                ));
                            }
                            total
                        });
                    },
                );
                runtime.block_on(server.shutdown());
            }
        }
    }

    group.finish();
}

fn bench_rate_limiter(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    let distinct_limiter = BenchRateLimiter::new(Some(1_000_000), Some(1_000_000));
    let distinct_counter = AtomicUsize::new(0);
    c.bench_function("torii_rate_limiter_distinct_key", |b| {
        b.iter_batched(
            || {
                let index = distinct_counter.fetch_add(1, Ordering::Relaxed);
                format!("torii-hot-path-bench-{index}")
            },
            |key| {
                let allowed = runtime.block_on(distinct_limiter.allow(&key));
                std::hint::black_box(allowed);
            },
            BatchSize::SmallInput,
        );
    });

    let same_key_limiter = BenchRateLimiter::new(Some(1_000_000), Some(1_000_000));
    c.bench_function("torii_rate_limiter_same_key", |b| {
        b.iter(|| {
            let allowed = runtime.block_on(same_key_limiter.allow("torii-hot-path-bench-shared"));
            std::hint::black_box(allowed);
        });
    });

    let same_key_batch_limiter = BenchRateLimiter::new(Some(1_000_000), Some(1_000_000));
    c.bench_function("torii_rate_limiter_same_key_batch_32", |b| {
        b.iter(|| {
            let allowed = runtime
                .block_on(same_key_batch_limiter.allow_repeated("torii-hot-path-bench-batch", 32));
            std::hint::black_box(allowed);
        });
    });
}

fn smoke_profile_output() {
    let mut samples = Vec::with_capacity(8);
    for micros in 1..=8 {
        samples.push(Duration::from_micros(micros));
    }
    print_profile(
        "criterion_smoke",
        "profile_stats_output_shape",
        samples,
        0,
        NonZeroUsize::new(1).expect("non-zero").get(),
        Duration::from_millis(1),
    );
}

/// Entry point for the benchmark binary.
fn main() {
    smoke_profile_output();
    let mut c = Criterion::default().configure_from_args();
    bench_signed_query_verify(&mut c);
    bench_query_find_parameters(&mut c);
    bench_transaction_admission(&mut c);
    bench_transaction_handle_enqueue(&mut c);
    bench_transaction_enqueue_sustained_pressure(&mut c);
    bench_query_http_sustained(&mut c);
    bench_query_http_socket_sustained(&mut c);
    bench_rate_limiter(&mut c);
    c.final_summary();
}
