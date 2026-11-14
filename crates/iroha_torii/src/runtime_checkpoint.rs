//! Operator-authenticated bounded delivery of one pre-exported immutable checkpoint package.
//!
//! Neither route parameters nor delivered bytes grant State authority. The receiver still needs
//! independently pinned genesis, complete-State authentication and actual native successor replay.
use super::*;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use iroha_core::{
    execution_attempt::ExecutionAttemptError,
    snapshot::transfer::transport::{
        self, CheckpointTransport, ExportedCheckpoint, MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES,
    },
};

pub(crate) type SharedPackage = Option<Arc<parking_lot::Mutex<ExportedCheckpoint>>>;
const METADATA_BYTES: usize = 16 * 1024;
pub(crate) const REQUEST_BYTES: usize = 1024;

/// Select one artifact only within the independently pinned immutable inventory.
#[derive(
    Debug,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_checkpoint::ArtifactRequest")]
#[norito(deny_unknown_fields)]
pub(crate) struct ArtifactRequest {
    /// Exact digest returned by the package descriptor.
    pub inventory_sha256: [u8; 32],
    /// Position in the closed artifact inventory, never a filesystem path.
    pub index: u64,
}
/// Select one exact bounded range of an original artifact.
#[derive(
    Debug,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_checkpoint::RangeRequest")]
#[norito(deny_unknown_fields)]
pub(crate) struct RangeRequest {
    /// Exact digest returned by the package descriptor.
    pub inventory_sha256: [u8; 32],
    /// Position in the closed artifact inventory.
    pub index: u64,
    /// Byte offset in the original file.
    pub offset: u64,
    /// Exact nonzero range length, bounded by both the source and local response policy.
    pub length: u32,
}
/// Finish delivery by rechecking all original source snapshots.
#[derive(
    Debug,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_torii::runtime_checkpoint::FinishRequest")]
#[norito(deny_unknown_fields)]
pub(crate) struct FinishRequest {
    /// Exact digest returned by the package descriptor.
    pub inventory_sha256: [u8; 32],
}

enum Work {
    Descriptor,
    Artifact(ArtifactRequest),
    Range(RangeRequest),
    Finish(FinishRequest),
}
impl Work {
    fn inventory(&self) -> Option<&[u8; 32]> {
        match self {
            Self::Descriptor => None,
            Self::Artifact(request) => Some(&request.inventory_sha256),
            Self::Range(request) => Some(&request.inventory_sha256),
            Self::Finish(request) => Some(&request.inventory_sha256),
        }
    }
    fn response_bytes(&self, local_limit: usize) -> Result<usize, WorkError> {
        let bytes = match self {
            Self::Descriptor | Self::Artifact(_) => METADATA_BYTES,
            Self::Finish(_) => 1,
            Self::Range(request) => {
                let length = request.length as usize;
                if length == 0 || length > MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES {
                    return Err(WorkError::Range);
                }
                length
            }
        };
        if bytes > local_limit {
            return Err(WorkError::Range);
        }
        Ok(bytes)
    }
}

#[derive(Debug, thiserror::Error)]
enum WorkError {
    #[error("original checkpoint source refused delivery: {0}")]
    Source(#[from] ExecutionAttemptError<transport::Error>),
    #[error("original response allocation refused: {0}")]
    Buffer(#[from] ChargedBufferError),
    #[error("original metadata encoding refused: {0}")]
    Encoding(#[from] native_projection_response::NativeProjectionError),
    #[error("checkpoint source is busy")]
    Busy,
    #[error("checkpoint package differs from the node's independent chain or network")]
    SourcePin,
    #[error("checkpoint package is below the node's current authenticated startup floor")]
    HistoricalFloor,
    #[error("checkpoint inventory differs from the request")]
    Inventory,
    #[error("checkpoint range exceeds the unchanged finite response policy")]
    Range,
}
fn unavailable(code: &'static str) -> Error {
    Error::AppServiceUnavailable {
        code,
        message: "The configured immutable checkpoint package is unavailable.".into(),
    }
}
impl From<WorkError> for Error {
    fn from(error: WorkError) -> Self {
        match error {
            WorkError::Source(ExecutionAttemptError::Deferred(_)) | WorkError::Buffer(_) => {
                native_projection_response::capacity()
            }
            WorkError::Encoding(original) => original.into(),
            WorkError::Busy => unavailable("runtime_checkpoint_source_busy"),
            WorkError::Source(ExecutionAttemptError::Rejected(transport::Error::Io(_))) => {
                unavailable("runtime_checkpoint_source_unavailable")
            }
            other => conversion_error(other.to_string()),
        }
    }
}
struct RangeBody {
    bytes: ChargedBuffer<u8>,
    // Physical response bytes retire before their original query-pool credit.
    _memory: QueryFanoutMemoryReservation,
}
impl AsRef<[u8]> for RangeBody {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}

fn validate_source(
    app: &AppState,
    descriptor: &transport::PackageDescriptorV1,
) -> Result<(), WorkError> {
    if &descriptor.chain_id != app.state.chain_id_ref()
        || &descriptor.network_id != app.state.network_id_ref()
    {
        return Err(WorkError::SourcePin);
    }
    if app
        .kura
        .history_boundary()
        .is_some_and(|floor| descriptor.height < floor.height)
    {
        return Err(WorkError::HistoricalFloor);
    }
    Ok(())
}

fn execute(
    source: &mut ExportedCheckpoint,
    work: Work,
    format: ResponseFormat,
    bytes: usize,
    budget: &AllocationBudget,
    memory: QueryFanoutMemoryReservation,
    app: &AppState,
) -> Result<(Response, u64), WorkError> {
    let descriptor = source.descriptor()?;
    validate_source(app, &descriptor)?;
    if work
        .inventory()
        .is_some_and(|expected| expected != &descriptor.inventory_sha256)
    {
        return Err(WorkError::Inventory);
    }
    let mut response = match work {
        Work::Descriptor => {
            let mut body = native_projection_response::encode(&descriptor, format, bytes, budget)?;
            body.memory = Some(memory);
            Response::new(Body::from(Bytes::from_owner(body)))
        }
        Work::Artifact(request) => {
            let artifact = source.artifact(request.index)?;
            let mut body = native_projection_response::encode(&artifact, format, bytes, budget)?;
            body.memory = Some(memory);
            Response::new(Body::from(Bytes::from_owner(body)))
        }
        Work::Range(request) => {
            if bytes != request.length as usize || request.length > descriptor.max_chunk_bytes {
                return Err(WorkError::Range);
            }
            let mut output = ChargedBuffer::new(bytes, budget)?;
            for _ in 0..bytes {
                output.push_reserved(0);
            }
            source.read_range(request.index, request.offset, output.as_mut_slice())?;
            let mut response = Response::new(Body::from(Bytes::from_owner(RangeBody {
                bytes: output,
                _memory: memory,
            })));
            response.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            );
            response.headers_mut().insert(
                axum::http::header::CACHE_CONTROL,
                HeaderValue::from_static("private, no-store"),
            );
            validate_source(app, &descriptor)?;
            return Ok((response, descriptor.height));
        }
        Work::Finish(_) => {
            source.finish(&descriptor)?;
            StatusCode::NO_CONTENT.into_response()
        }
    };
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static(match format {
            ResponseFormat::Norito => "application/x-norito",
            ResponseFormat::Json => "application/json",
        }),
    );
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    validate_source(app, &descriptor)?;
    Ok((response, descriptor.height))
}

async fn handle(
    app: SharedAppState,
    headers: HeaderMap,
    remote: std::net::SocketAddr,
    route: &'static str,
    work: Work,
) -> Result<Response, Error> {
    check_access_enforced(&app, &headers, Some(remote.ip()), route, true).await?;
    let format = if matches!(work, Work::Range(_) | Work::Finish(_)) {
        ResponseFormat::Json
    } else {
        match negotiate_heavy_query_response_format(&headers) {
            Ok(format) => format,
            Err(response) => return Ok(response),
        }
    };
    let source = app
        .runtime_checkpoint_package
        .clone()
        .ok_or_else(|| unavailable("runtime_checkpoint_not_configured"))?;
    let bytes = work.response_bytes(app.torii_proxy_max_response_bytes)?;
    let admission = acquire_query_admission(&app, true).await?;
    let memory = acquire_query_pool_credit(&app, bytes)
        .await
        .ok_or_else(native_projection_response::capacity)?;
    let memory = QueryFanoutMemoryReservation::new(memory);
    // This sub-budget cannot exceed the original query pool credit retained through HTTP drain.
    let budget = AllocationBudget::new(bytes);
    let worker_app = app.clone();
    let result = panic_recovery::spawn_blocking_recoverable(move || {
        let _admission = admission;
        let mut source = source.try_lock().ok_or(WorkError::Busy)?;
        execute(
            &mut source,
            work,
            format,
            bytes,
            &budget,
            memory,
            &worker_app,
        )
    })
    .await;
    let (response, package_height) = match result {
        Ok(Ok(result)) => result.map_err(Error::from)?,
        _ => return Err(unavailable("runtime_checkpoint_worker_unavailable")),
    };
    let response =
        proof_response_with_exact_egress(&app, &headers, Some(remote.ip()), route, response, true)
            .await?;
    // Egress admission may yield; honor a floor learned while this response was waiting too.
    if app
        .kura
        .history_boundary()
        .is_some_and(|floor| package_height < floor.height)
    {
        return Err(WorkError::HistoricalFloor.into());
    }
    Ok(response)
}

pub(crate) async fn descriptor(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, Error> {
    handle(
        app,
        headers,
        remote,
        "/v1/runtime/checkpoint/package",
        Work::Descriptor,
    )
    .await
}
pub(crate) async fn artifact(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<ArtifactRequest>,
) -> Result<Response, Error> {
    handle(
        app,
        headers,
        remote,
        "/v1/runtime/checkpoint/artifact",
        Work::Artifact(request),
    )
    .await
}
pub(crate) async fn range(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<RangeRequest>,
) -> Result<Response, Error> {
    handle(
        app,
        headers,
        remote,
        "/v1/runtime/checkpoint/range",
        Work::Range(request),
    )
    .await
}
pub(crate) async fn finish(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    NoritoJson(request): NoritoJson<FinishRequest>,
) -> Result<Response, Error> {
    handle(
        app,
        headers,
        remote,
        "/v1/runtime/checkpoint/finish",
        Work::Finish(request),
    )
    .await
}

#[cfg(test)]
mod tests;
