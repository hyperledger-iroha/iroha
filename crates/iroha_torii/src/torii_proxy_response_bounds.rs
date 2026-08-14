#[cfg(any(feature = "p2p_ws", feature = "connect"))]
const TORII_PROXY_MAX_HEADERS_V1: usize = 128;
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
const TORII_PROXY_MAX_HEADER_BYTES_V1: usize = 64 * 1024;
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
const TORII_PROXY_MAX_HEADER_NAME_BYTES_V1: usize = 256;
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
const TORII_PROXY_MAX_HEADER_VALUE_BYTES_V1: usize = 16 * 1024;
/// Maximum retryable diagnostic body retained while the next authority runs.
const TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1: usize = 64 * 1024;
/// Drop an oversized retryable body before another full response is admitted.
///
/// The original snapshot has already passed the route body limit, but keeping
/// that entire allocation beside the next transport chunk and accumulated
/// response would multiply the one-slot proxy envelope. Ordinary diagnostics
/// are preserved byte-for-byte; an oversized retryable diagnostic is replaced
/// by fixed local text while retaining its status code.
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn bound_retained_retryable_torii_proxy_snapshot(
    snapshot: ToriiProxyHttpResponseV1,
) -> ToriiProxyHttpResponseV1 {
    if snapshot.body.len() <= TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1 {
        return snapshot;
    }
    const MESSAGE: &[u8] =
        b"proxied retryable response body exceeded the retained diagnostic limit";
    let mut body = Vec::new();
    if body.try_reserve_exact(MESSAGE.len()).is_ok() {
        body.extend_from_slice(MESSAGE);
    }
    ToriiProxyHttpResponseV1 {
        status_code: snapshot.status_code,
        headers: Vec::new(),
        body,
    }
}
#[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
#[derive(Debug)]
enum BoundedReqwestBodyError {
    Limit(String),
    Transport(String),
    Allocation(String),
}
#[cfg(feature = "app_api")]
impl BoundedReqwestBodyError {
    const fn is_limit(&self) -> bool {
        matches!(self, Self::Limit(_))
    }
}
#[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
impl core::fmt::Display for BoundedReqwestBodyError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Limit(message) | Self::Transport(message) | Self::Allocation(message) => {
                formatter.write_str(message)
            }
        }
    }
}
#[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
fn validate_reqwest_response_content_length(
    declared: Option<u64>,
    max_body_bytes: usize,
    label: &str,
) -> Result<(), BoundedReqwestBodyError> {
    let max_body_bytes_u64 = u64::try_from(max_body_bytes).unwrap_or(u64::MAX);
    if declared.is_some_and(|declared| declared > max_body_bytes_u64) {
        return Err(BoundedReqwestBodyError::Limit(format!(
            "{label} response Content-Length exceeds the {max_body_bytes}-byte limit"
        )));
    }
    Ok(())
}
#[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
fn extend_bounded_reqwest_body(
    body: &mut Vec<u8>,
    chunk: &[u8],
    max_body_bytes: usize,
    label: &str,
) -> Result<(), BoundedReqwestBodyError> {
    let next_len = body.len().checked_add(chunk.len()).ok_or_else(|| {
        BoundedReqwestBodyError::Limit(format!("{label} response body length overflowed"))
    })?;
    if next_len > max_body_bytes {
        return Err(BoundedReqwestBodyError::Limit(format!(
            "{label} response body exceeds the {max_body_bytes}-byte limit"
        )));
    }
    body.try_reserve_exact(chunk.len()).map_err(|error| {
        BoundedReqwestBodyError::Allocation(format!(
            "failed to reserve bounded {label} response body: {error}"
        ))
    })?;
    body.extend_from_slice(chunk);
    Ok(())
}
/// Read one decoded HTTP response body without trusting `Content-Length`.
///
/// The declared length is only a fail-fast check; the streaming counter is the
/// authority because transfer/content decoding may expand the body.
#[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
async fn read_reqwest_response_body_bounded(
    response: &mut reqwest::Response,
    max_body_bytes: usize,
    label: &str,
) -> Result<Vec<u8>, BoundedReqwestBodyError> {
    validate_reqwest_response_content_length(response.content_length(), max_body_bytes, label)?;
    let initial_capacity = response
        .content_length()
        .and_then(|declared| usize::try_from(declared).ok())
        .unwrap_or(0)
        .min(max_body_bytes)
        .min(16 * 1024);
    let mut body = Vec::new();
    body.try_reserve_exact(initial_capacity).map_err(|error| {
        BoundedReqwestBodyError::Allocation(format!(
            "failed to reserve bounded {label} response body: {error}"
        ))
    })?;
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        BoundedReqwestBodyError::Transport(format!("failed to read {label} response body: {error}"))
    })? {
        extend_bounded_reqwest_body(&mut body, &chunk, max_body_bytes, label)?;
    }
    Ok(body)
}
#[cfg(feature = "app_api")]
fn public_dataspace_upstream_response_body_limit(
    endpoint: ToriiReadEndpointV1,
    configured_max_bytes: usize,
) -> usize {
    if matches!(
        endpoint,
        ToriiReadEndpointV1::ContractViewPost | ToriiReadEndpointV1::ContractViewBatchPost
    ) {
        configured_max_bytes.min(routing::CONTRACT_CALL_SIMULATION_JSON_MAX_BYTES)
    } else {
        configured_max_bytes
    }
    .max(1)
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn bounded_torii_proxy_headers(
    headers: &HeaderMap,
) -> Result<Vec<iroha_core::torii_proxy::ToriiProxyHeaderV1>, String> {
    if headers.len() > TORII_PROXY_MAX_HEADERS_V1 {
        return Err(format!(
            "proxied response contains more than {TORII_PROXY_MAX_HEADERS_V1} headers"
        ));
    }
    let mut bounded = Vec::new();
    bounded
        .try_reserve_exact(headers.len())
        .map_err(|_| "failed to reserve bounded proxied response headers".to_owned())?;
    let mut total_bytes = 0_usize;
    for (name, value) in headers {
        let name_len = name.as_str().len();
        let value_len = value.as_bytes().len();
        if name_len > TORII_PROXY_MAX_HEADER_NAME_BYTES_V1
            || value_len > TORII_PROXY_MAX_HEADER_VALUE_BYTES_V1
        {
            return Err("proxied response header exceeds its field limit".to_owned());
        }
        total_bytes = total_bytes
            .checked_add(name_len)
            .and_then(|bytes| bytes.checked_add(value_len))
            .filter(|bytes| *bytes <= TORII_PROXY_MAX_HEADER_BYTES_V1)
            .ok_or_else(|| "proxied response headers exceed their aggregate limit".to_owned())?;
        bounded.push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: name.as_str().to_owned(),
            value: value.as_bytes().to_vec(),
        });
    }
    Ok(bounded)
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn validate_torii_proxy_snapshot_bounds(
    snapshot: &ToriiProxyHttpResponseV1,
    max_body_bytes: usize,
) -> Result<(), String> {
    if snapshot.body.len() > max_body_bytes {
        return Err(format!(
            "proxied response body exceeds configured limit of {max_body_bytes} bytes"
        ));
    }
    if snapshot.headers.len() > TORII_PROXY_MAX_HEADERS_V1 {
        return Err(format!(
            "proxied response contains more than {TORII_PROXY_MAX_HEADERS_V1} headers"
        ));
    }
    let mut total_bytes = 0_usize;
    for header in &snapshot.headers {
        if header.name.len() > TORII_PROXY_MAX_HEADER_NAME_BYTES_V1
            || header.value.len() > TORII_PROXY_MAX_HEADER_VALUE_BYTES_V1
        {
            return Err("proxied response header exceeds its field limit".to_owned());
        }
        total_bytes = total_bytes
            .checked_add(header.name.len())
            .and_then(|bytes| bytes.checked_add(header.value.len()))
            .filter(|bytes| *bytes <= TORII_PROXY_MAX_HEADER_BYTES_V1)
            .ok_or_else(|| "proxied response headers exceed their aggregate limit".to_owned())?;
    }
    Ok(())
}
#[cfg(any(feature = "p2p_ws", feature = "connect"))]
fn torii_proxy_response_body_limit(app: &AppState, request: &ToriiProxyRequestKindV4) -> usize {
    match request {
        ToriiProxyRequestKindV4::HostedHttp(_) => app
            .soracloud_public_max_response_bytes
            .min(app.torii_proxy_max_response_bytes)
            .max(1),
        ToriiProxyRequestKindV4::SubmitTransaction {
            admission: ToriiProxyTransactionAdmissionV2::QueuePlanSynced,
            ..
        } => QUEUE_PLAN_SYNCED_CERTIFICATE_MAX_BODY_BYTES_V2.max(1),
        ToriiProxyRequestKindV4::SignedQuery { .. }
        | ToriiProxyRequestKindV4::SignedQueryRouteScan { .. } => {
            QueryFanoutMemoryEnvelope::for_body_admission(app.query_fanout_working_set_bytes)
                .map_or(1, |envelope| envelope.route_body_bytes.max(1))
        }
        ToriiProxyRequestKindV4::SignedQueryFanout { .. } => {
            QueryFanoutMemoryEnvelope::for_body_admission(app.query_fanout_working_set_bytes)
                .map_or(1, |envelope| envelope.final_body_bytes.max(1))
        }
        _ => app.torii_proxy_max_response_bytes.max(1),
    }
}
#[cfg(all(test, any(feature = "p2p_ws", feature = "connect")))]
mod retained_retryable_snapshot_tests {
    use super::*;
    #[test]
    fn exact_diagnostic_is_preserved_and_overflow_is_replaced() {
        let exact = ToriiProxyHttpResponseV1 {
            status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            headers: Vec::new(),
            body: vec![0x5a; TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1],
        };
        assert_eq!(
            bound_retained_retryable_torii_proxy_snapshot(exact.clone()),
            exact
        );
        let overflow = ToriiProxyHttpResponseV1 {
            status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            headers: vec![iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                name: "content-length".to_owned(),
                value: b"65537".to_vec(),
            }],
            body: vec![0x5a; TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1 + 1],
        };
        let bounded = bound_retained_retryable_torii_proxy_snapshot(overflow);
        assert_eq!(
            bounded.status_code,
            StatusCode::SERVICE_UNAVAILABLE.as_u16()
        );
        assert!(bounded.headers.is_empty());
        assert!(bounded.body.len() < TORII_PROXY_RETRYABLE_RETAINED_BODY_BYTES_V1);
    }
}
#[cfg(all(test, feature = "app_api"))]
mod bounded_reqwest_body_tests {
    use super::*;
    #[test]
    fn chunk_admission_accepts_exact_limit_and_rejects_max_plus_one() {
        let mut body = Vec::new();
        extend_bounded_reqwest_body(&mut body, b"abcd", 8, "test").expect("first bounded chunk");
        extend_bounded_reqwest_body(&mut body, b"efgh", 8, "test").expect("exact response limit");
        assert_eq!(body, b"abcdefgh");
        let error = extend_bounded_reqwest_body(&mut body, b"i", 8, "test")
            .expect_err("response limit plus one must fail");
        assert!(error.is_limit());
        assert!(error.to_string().contains("8-byte limit"));
        assert_eq!(body, b"abcdefgh", "overflow must not mutate the body");
        validate_reqwest_response_content_length(Some(8), 8, "test")
            .expect("exact declared response limit");
        let declared_error = validate_reqwest_response_content_length(Some(9), 8, "test")
            .expect_err("declared response limit plus one must fail");
        assert!(declared_error.is_limit());
    }
    #[test]
    fn public_dataspace_contract_views_use_the_stricter_response_limit() {
        let configured = routing::CONTRACT_CALL_SIMULATION_JSON_MAX_BYTES * 2;
        assert_eq!(
            public_dataspace_upstream_response_body_limit(
                ToriiReadEndpointV1::ContractViewPost,
                configured,
            ),
            routing::CONTRACT_CALL_SIMULATION_JSON_MAX_BYTES
        );
        assert_eq!(
            public_dataspace_upstream_response_body_limit(
                ToriiReadEndpointV1::DomainsList,
                configured,
            ),
            configured
        );
        assert_eq!(
            public_dataspace_upstream_response_body_limit(ToriiReadEndpointV1::DomainsList, 0),
            1
        );
    }
}
