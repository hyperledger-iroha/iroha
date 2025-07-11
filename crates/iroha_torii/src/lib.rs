//! The web server of Iroha. `Torii` translates to gateway.
//!
//! Crate provides the following features that are not enabled by default:
//!
//! - `telemetry`: enables Status, Metrics, and API Version endpoints
//! - `schema`: enables Data Model Schema endpoint

use std::{collections::HashSet, fmt::Debug, sync::Arc, time::Duration};

use axum::{
    extract::{DefaultBodyLimit, WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use error_stack::ResultExt;
use iroha_config::{
    base::{util::Bytes, WithOrigin},
    parameters::actual::Torii as Config,
};
#[cfg(feature = "telemetry")]
use iroha_core::telemetry::Telemetry;
use iroha_core::{
    kiso::{Error as KisoError, KisoHandle},
    kura::Kura,
    prelude::*,
    query::store::LiveQueryStoreHandle,
    queue::{self, Queue},
    state::State,
    EventsSender,
};
use iroha_data_model::{peer::Peer, ChainId};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_primitives::addr::SocketAddr;
use iroha_torii_shared::uri;
use tokio::{net::TcpListener, sync::watch};
use tower_http::{
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};
use utils::{extractors::ScaleVersioned, Scale};

#[macro_use]
pub(crate) mod utils;
mod block;
mod event;
mod routing;
mod stream;

const SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(60);

/// Main network handler and the only entrypoint of the Iroha.
pub struct Torii {
    chain_id: Arc<ChainId>,
    kiso: KisoHandle,
    queue: Arc<Queue>,
    events: EventsSender,
    query_service: LiveQueryStoreHandle,
    kura: Arc<Kura>,
    transaction_max_content_len: Bytes<u64>,
    address: WithOrigin<SocketAddr>,
    state: Arc<State>,
    #[cfg(feature = "telemetry")]
    telemetry: Telemetry,
    online_peers: OnlinePeersProvider,
}

impl Torii {
    /// Construct `Torii`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: ChainId,
        kiso: KisoHandle,
        config: Config,
        queue: Arc<Queue>,
        events: EventsSender,
        query_service: LiveQueryStoreHandle,
        kura: Arc<Kura>,
        state: Arc<State>,
        online_peers: OnlinePeersProvider,
        #[cfg(feature = "telemetry")] telemetry: Telemetry,
    ) -> Self {
        Self {
            chain_id: Arc::new(chain_id),
            kiso,
            queue,
            events,
            query_service,
            kura,
            state,
            online_peers,
            #[cfg(feature = "telemetry")]
            telemetry,
            address: config.address,
            transaction_max_content_len: config.max_content_len,
        }
    }

    /// Helper function to create router. This router can be tested without starting up an HTTP server
    #[allow(clippy::too_many_lines)]
    fn create_api_router(&self) -> axum::Router {
        let router = Router::new()
            .route(uri::HEALTH, get(routing::handle_health))
            .route(
                uri::CONFIGURATION,
                get({
                    let kiso = self.kiso.clone();
                    move || routing::handle_get_configuration(kiso)
                }),
            )
            .route(
                uri::API_VERSION,
                get({
                    let state = self.state.clone();
                    move || routing::handle_version(state)
                }),
            )
            .route(
                uri::PEERS,
                get({
                    let peers = self.online_peers.clone();
                    move || async move { routing::handle_peers(&peers) }
                }),
            );

        #[cfg(feature = "telemetry")]
        let router = router
            .route(
                &format!("{}/*tail", uri::STATUS),
                get({
                    let tel = self.telemetry.clone();
                    move |accept: Option<utils::extractors::ExtractAccept>, axum::extract::Path(tail): axum::extract::Path<String>| async move {
                        routing::handle_status(
                            &tel,
                            accept.map(|extract| extract.0),
                            Some(&tail),
                        ).await
                    }
                }),
            )
            .route(
                uri::STATUS,
                get({
                    let tel = self.telemetry.clone();
                    move |accept: Option<utils::extractors::ExtractAccept>| async move {
                        routing::handle_status(&tel, accept.map(|extract| extract.0), None).await
                    }
                }),
            )
            .route(
                uri::METRICS,
                get({
                    let tel = self.telemetry.clone();
                    move || async move { routing::handle_metrics(&tel).await }
                }),
            );
        #[cfg(not(feature = "telemetry"))]
        let router = router
            .route(uri::STATUS, get(routing::telemetry_not_implemented))
            .route(
                &format!("{}/*rest", uri::STATUS),
                get(routing::telemetry_not_implemented),
            )
            .route(uri::METRICS, get(routing::telemetry_not_implemented));

        #[cfg(feature = "schema")]
        let router = router.route(uri::SCHEMA, get(routing::handle_schema));
        #[cfg(not(feature = "schema"))]
        let router = router.route(uri::SCHEMA, get(routing::schema_not_implemented));

        #[cfg(feature = "profiling")]
        let router = router.route(
            uri::PROFILE,
            get({
                let profiling_lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
                move |axum::extract::Query(params): axum::extract::Query<_>| {
                    let profiling_lock = Arc::clone(&profiling_lock);
                    routing::profiling::handle_profile(params, profiling_lock)
                }
            }),
        );
        #[cfg(not(feature = "profiling"))]
        let router = router.route(uri::PROFILE, get(routing::profiling_not_implemented));

        let router = router
            .route(
                uri::TRANSACTION,
                post({
                    let chain_id = self.chain_id.clone();
                    let queue = self.queue.clone();
                    let state = self.state.clone();
                    move |ScaleVersioned(transaction): ScaleVersioned<_>| {
                        routing::handle_transaction(chain_id, queue, state, transaction)
                    }
                })
                .layer(DefaultBodyLimit::max(
                    self.transaction_max_content_len
                        .get()
                        .try_into()
                        .expect("should't exceed usize"),
                )),
            )
            .route(
                uri::QUERY,
                post({
                    let query_service = self.query_service.clone();
                    let state = self.state.clone();
                    move |ScaleVersioned(query_request): ScaleVersioned<_>| {
                        routing::handle_queries(query_service, state, query_request)
                    }
                }),
            )
            .route(
                uri::CONFIGURATION,
                post({
                    let kiso = self.kiso.clone();
                    move |Json(config): Json<_>| routing::handle_post_configuration(kiso, config)
                }),
            );

        let router = router
            .route(
                uri::SUBSCRIPTION,
                get({
                    let events = self.events.clone();
                    move |ws: WebSocketUpgrade| {
                        core::future::ready(ws.on_upgrade(|ws| async move {
                            if let Err(error) =
                                routing::event::handle_events_stream(events, ws).await
                            {
                                iroha_logger::error!(%error, "Failure during event streaming");
                            }
                        }))
                    }
                }),
            )
            .route(
                uri::BLOCKS_STREAM,
                get({
                    let kura = self.kura.clone();
                    move |ws: WebSocketUpgrade| {
                        core::future::ready(ws.on_upgrade(|ws| async move {
                            if let Err(error) = routing::block::handle_blocks_stream(kura, ws).await
                            {
                                iroha_logger::error!(%error, "Failure during block streaming");
                            }
                        }))
                    }
                }),
            );

        let router = router.route(
            uri::SERVER_VERSION,
            get(move || async move { routing::handle_server_version() }),
        );

        router.layer((
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
            // Graceful shutdown will wait for outstanding requests to complete.
            // Add a timeout so requests don't hang forever.
            TimeoutLayer::new(SERVER_SHUTDOWN_TIMEOUT),
        ))
    }

    /// To handle incoming requests `Torii` should be started first.
    ///
    /// # Errors
    /// Can fail due to listening to network or if http server fails
    // #[iroha_futures::telemetry_future]
    pub async fn start(self, shutdown_signal: ShutdownSignal) -> error_stack::Result<(), Error> {
        let torii_address = self.address.value().clone();

        let listener = match torii_address {
            SocketAddr::Ipv4(v) => TcpListener::bind(std::net::SocketAddr::V4(v.into())).await,
            SocketAddr::Ipv6(v) => TcpListener::bind(std::net::SocketAddr::V6(v.into())).await,
            SocketAddr::Host(v) => TcpListener::bind((v.host.as_ref(), v.port)).await,
        }
        .change_context(Error::StartServer)
        .attach_printable("failed to bind to the specified address")
        .attach_printable_lazy(|| self.address.clone().into_attachment())?;
        let api_router = self.create_api_router();

        axum::serve(listener, api_router)
            .with_graceful_shutdown(async move { shutdown_signal.receive().await })
            .await
            .change_context(Error::FailedExit)
    }
}

/// Torii errors.
#[derive(thiserror::Error, displaydoc::Display, pretty_error_debug::Debug)]
pub enum Error {
    /// Failed to process query
    Query(#[from] iroha_data_model::ValidationFail),
    /// Failed to accept transaction
    AcceptTransaction(#[from] iroha_core::tx::AcceptTransactionFail),
    /// Failed to get or set configuration
    Config(#[source] eyre::Report),
    /// Failed to push into queue
    PushIntoQueue(#[from] Box<queue::Error>),
    #[cfg(feature = "telemetry")]
    /// Failed to get Prometheus metrics
    Prometheus(#[source] eyre::Report),
    #[cfg(feature = "profiling")]
    /// Failed to get pprof profile
    Pprof(#[source] eyre::Report),
    #[cfg(feature = "telemetry")]
    /// Failed to get status
    StatusFailure(#[source] eyre::Report),
    /// Failure caused by configuration subsystem
    ConfigurationFailure(#[from] KisoError),
    /// Failed to find status segment by provided path
    StatusSegmentNotFound(#[source] eyre::Report),
    /// Failed to start Torii
    StartServer,
    /// Torii server terminated with an error
    FailedExit,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Self::Query(err) => (Self::query_status_code(&err), utils::Scale(err)).into_response(),
            _ => (self.status_code(), format!("{self:?}")).into_response(),
        }
    }
}

impl Error {
    fn status_code(&self) -> StatusCode {
        use Error::*;

        match self {
            Query(e) => Self::query_status_code(e),
            AcceptTransaction(_) => StatusCode::BAD_REQUEST,
            Config(_) | StatusSegmentNotFound(_) => StatusCode::NOT_FOUND,
            PushIntoQueue(err) => match **err {
                queue::Error::Full => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::BAD_REQUEST,
            },
            #[cfg(feature = "telemetry")]
            Prometheus(_) | StatusFailure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "profiling")]
            Pprof(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ConfigurationFailure(_) => StatusCode::INTERNAL_SERVER_ERROR,
            StartServer | FailedExit => unreachable!("these never occur during request handling"),
        }
    }

    fn query_status_code(validation_error: &iroha_data_model::ValidationFail) -> StatusCode {
        use iroha_data_model::{
            isi::error::InstructionExecutionError, query::error::QueryExecutionFail::*,
            ValidationFail::*,
        };

        match validation_error {
            NotPermitted(_) => StatusCode::FORBIDDEN,
            QueryFailed(query_error)
            | InstructionFailed(InstructionExecutionError::Query(query_error)) => match query_error
            {
                Conversion(_)
                | CursorMismatch
                | CursorDone
                | NotFound
                | FetchSizeTooBig
                | InvalidSingularParameters => StatusCode::BAD_REQUEST,
                Find(_) => StatusCode::NOT_FOUND,
                CapacityLimit => StatusCode::TOO_MANY_REQUESTS,
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
    use http_body_util::BodyExt as _;

    use super::*;

    #[tokio::test]
    async fn error_response_contains_details() {
        let err = Error::AcceptTransaction(iroha_core::tx::AcceptTransactionFail::ChainIdMismatch(
            iroha_data_model::isi::error::Mismatch {
                expected: "123".into(),
                actual: "321".into(),
            },
        ));
        let response = err.into_response();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let text =
            String::from_utf8(body.iter().copied().collect()).expect("to be a valid UTF8 string");
        assert_eq!(text, "Failed to accept transaction\n\nCaused by:\n    Chain id doesn't correspond to the id of current blockchain: Expected ChainId(\"123\"), actual ChainId(\"321\")");
    }
}
