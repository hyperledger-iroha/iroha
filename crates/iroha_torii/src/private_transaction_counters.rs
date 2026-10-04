//! Bounded original-frame transport to the native private-counter computation owner.

use super::*;
use iroha_data_model::private_transaction_counters::{
    MAX_PRIVATE_COUNTER_FRAME_BYTES_V1, PrivateCountersErrorV1,
};

fn transport_headers(headers: &axum::http::HeaderMap) -> bool {
    let exact = |name: axum::http::header::HeaderName, expected: &str| {
        let mut values = headers.get_all(name).iter();
        matches!(values.next(), Some(value) if value.as_bytes() == expected.as_bytes())
            && values.next().is_none()
    };
    exact(axum::http::header::CONTENT_TYPE, "application/x-norito")
        && exact(axum::http::header::ACCEPT, "application/x-norito")
        && (!headers.contains_key(axum::http::header::CONTENT_ENCODING)
            || exact(axum::http::header::CONTENT_ENCODING, "identity"))
}

fn declared_length_admitted(headers: &axum::http::HeaderMap) -> bool {
    let mut values = headers.get_all(axum::http::header::CONTENT_LENGTH).iter();
    let Some(value) = values.next() else {
        return true;
    };
    if values.next().is_some() {
        return false;
    }
    let Ok(text) = value.to_str() else {
        return false;
    };
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    text.parse::<usize>().is_ok_and(|length| {
        length > 0 && length <= MAX_PRIVATE_COUNTER_FRAME_BYTES_V1 && length.to_string() == text
    })
}

fn refusal(status: StatusCode) -> AxResponse {
    // No private request, parser, account, policy or history body is reflected.
    (status, "private counters request refused").into_response()
}

fn native_refusal(error: PrivateCountersErrorV1) -> AxResponse {
    let status = match error {
        PrivateCountersErrorV1::Unauthorized | PrivateCountersErrorV1::Signature => {
            StatusCode::FORBIDDEN
        }
        PrivateCountersErrorV1::Bounds => StatusCode::PAYLOAD_TOO_LARGE,
        PrivateCountersErrorV1::Codec | PrivateCountersErrorV1::UnsupportedMultisig => {
            StatusCode::BAD_REQUEST
        }
        PrivateCountersErrorV1::Context
        | PrivateCountersErrorV1::Freshness
        | PrivateCountersErrorV1::Replay
        | PrivateCountersErrorV1::Quorum => StatusCode::CONFLICT,
        PrivateCountersErrorV1::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    refusal(status)
}

fn response_from_owned_original(original: Bytes) -> AxResponse {
    let mut response = AxResponse::new(axum::body::Body::from(original));
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/x-norito"),
    );
    response
}

/// Forward one bounded original request after private-root, token and physical admission.
pub(crate) async fn compute(
    State(app): State<SharedAppState>,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    request: axum::extract::Request,
) -> Result<AxResponse, Error> {
    const ROUTE: &str = "/v1/private/transaction-counters";
    let (parts, body) = request.into_parts();
    // This handler takes Request rather than Bytes so a body is never polled before
    // the immutable private-root check and mandatory owner-token admission.
    if !app.is_private_root() {
        return Ok(refusal(StatusCode::FORBIDDEN));
    }
    let principal = validate_api_token(app.as_ref(), &parts.headers)?
        .authenticated_principal()
        .ok_or_else(|| {
            Error::Query(iroha_data_model::ValidationFail::NotPermitted(
                "private counters require an authenticated API token".into(),
            ))
        })?;
    if parts.uri.query().is_some() || !transport_headers(&parts.headers) {
        return Ok(refusal(StatusCode::UNSUPPORTED_MEDIA_TYPE));
    }
    if !declared_length_admitted(&parts.headers) {
        return Ok(refusal(StatusCode::PAYLOAD_TOO_LARGE));
    }
    let key = rate_limit_key(&parts.headers, Some(remote.ip()), ROUTE, Some(principal));
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let admission = acquire_query_admission(app.as_ref(), true).await?;
    let Some(service) = app.sumeragi.clone() else {
        return Ok(refusal(StatusCode::SERVICE_UNAVAILABLE));
    };
    let deadline = Instant::now() + DEFAULT_ROUTE_TIMEOUT;
    let readiness = service.clone();
    if !wait_for_consensus_readiness(deadline, move |until| readiness.ready_until(until)).await {
        return Ok(refusal(StatusCode::SERVICE_UNAVAILABLE));
    }
    let original = match tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        axum::body::to_bytes(body, MAX_PRIVATE_COUNTER_FRAME_BYTES_V1),
    )
    .await
    {
        Ok(Ok(bytes)) if !bytes.is_empty() => bytes,
        Ok(_) => return Ok(refusal(StatusCode::PAYLOAD_TOO_LARGE)),
        Err(_) => return Ok(refusal(StatusCode::REQUEST_TIMEOUT)),
    };
    // The complete original is the service input. HTTP never authenticates a reader
    // boolean, projects history, computes a count, chooses a cut or signs a claim.
    let native_result = routing::run_admitted_blocking(
        admission,
        "private counters worker unavailable",
        move || Ok(service.private_transaction_counters_v1(&original)),
    )
    .await?;
    let response = match native_result {
        Ok(original_response)
            if !original_response.is_empty()
                && original_response.len() <= MAX_PRIVATE_COUNTER_FRAME_BYTES_V1 =>
        {
            // This one immutable original has a known exact length. Keep its State-funded
            // owner in Bytes all the way through Hyper; no collect/copy detaches custody.
            let original = Bytes::from_owner(original_response);
            let maximum = app.torii_proxy_max_response_bytes.max(1);
            if original.len() > maximum {
                return Err(Error::Query(
                    iroha_data_model::ValidationFail::InternalError(
                        "private counters response exceeds configured content limit".into(),
                    ),
                ));
            }
            enforce_proof_egress(
                app.as_ref(),
                &parts.headers,
                Some(remote.ip()),
                ROUTE,
                u64::try_from(original.len()).unwrap_or(u64::MAX),
                true,
            )
            .await?;
            return Ok(response_from_owned_original(original));
        }
        Ok(_) => refusal(StatusCode::INTERNAL_SERVER_ERROR),
        Err(error) => native_refusal(error),
    };
    proof_response_with_exact_egress(
        app.as_ref(),
        &parts.headers,
        Some(remote.ip()),
        ROUTE,
        response,
        true,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn original_http_frame_and_last_slice_keep_native_backing_funded() {
        use http_body_util::BodyExt as _;
        use iroha_allocation::AllocationBudget;

        // Exercise the actual charged native encoder and the same single-original body path.
        // This is a transport lifetime control, not a constructed counters attestation.
        let length = norito::canonical_frame_len(&42_u64).unwrap();
        let budget = AllocationBudget::new(length);
        let encoded = crate::native_projection_response::encode(
            &42_u64,
            ResponseFormat::Norito,
            length,
            &budget,
            crate::native_projection_response::capacity,
        )
        .unwrap();
        let original = Bytes::from_owner(encoded);
        let pointer = original.as_ptr();
        let mut body = response_from_owned_original(original).into_body();
        assert_eq!(budget.reserved_bytes(), length);
        let frame = body.frame().await.unwrap().unwrap();
        let original = frame.into_data().unwrap();
        assert_eq!(original.as_ptr(), pointer);
        assert_eq!(
            original.as_ref(),
            norito::encode_canonical(&42_u64).unwrap()
        );
        drop(body);
        let last_byte = original.slice(length - 1..);
        drop(original);
        assert_eq!(budget.reserved_bytes(), length);
        drop(last_byte);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn transport_has_one_exact_identity_norito_representation() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            "application/x-norito".parse().unwrap(),
        );
        headers.insert(
            axum::http::header::ACCEPT,
            "application/x-norito".parse().unwrap(),
        );
        assert!(transport_headers(&headers));
        headers.insert(
            axum::http::header::CONTENT_ENCODING,
            "gzip".parse().unwrap(),
        );
        assert!(!transport_headers(&headers));
        headers.remove(axum::http::header::CONTENT_ENCODING);
        headers.append(
            axum::http::header::ACCEPT,
            "application/json".parse().unwrap(),
        );
        assert!(!transport_headers(&headers));
    }

    #[test]
    fn oversized_or_ambiguous_declared_length_refuses_before_body_admission() {
        let mut headers = axum::http::HeaderMap::new();
        assert!(declared_length_admitted(&headers));
        for text in ["0", "065536", "65537", "-1", "1,1", "99999999999999999999"] {
            headers.insert(axum::http::header::CONTENT_LENGTH, text.parse().unwrap());
            assert!(!declared_length_admitted(&headers));
        }
        headers.insert(axum::http::header::CONTENT_LENGTH, "65536".parse().unwrap());
        assert!(declared_length_admitted(&headers));
        headers.append(axum::http::header::CONTENT_LENGTH, "65536".parse().unwrap());
        assert!(!declared_length_admitted(&headers));
    }

    #[tokio::test]
    async fn global_root_refusal_does_not_poll_the_body() {
        let polled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = polled.clone();
        let body = axum::body::Body::from_stream(futures::stream::once(async move {
            observed.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"private original"))
        }));
        let request = axum::http::Request::builder()
            .uri("/v1/private/transaction-counters")
            .body(body)
            .unwrap();
        let response = compute(
            State(mk_app_state_for_tests()),
            axum::extract::ConnectInfo("127.0.0.1:1".parse().unwrap()),
            request,
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!polled.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn private_root_token_refusals_do_not_poll_the_body() {
        for supplied in [None, Some("wrong token"), Some("duplicate token")] {
            let mut app = crate::tests_runtime_handlers::app_with_root_scope_for_handler_test(
                iroha_core::state::World::new(),
                true,
            );
            let mutable = Arc::get_mut(&mut app).unwrap();
            mutable.require_api_token = false;
            mutable.api_token_digests =
                Arc::new(limits::ApiTokenDigestSet::from_tokens(["duplicate token"]));
            let polled = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let observed = polled.clone();
            let body = axum::body::Body::from_stream(futures::stream::once(async move {
                observed.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok::<_, std::io::Error>(axum::body::Bytes::from_static(b"private original"))
            }));
            let mut request = axum::http::Request::builder()
                .uri("/v1/private/transaction-counters")
                .body(body)
                .unwrap();
            if let Some(token) = supplied {
                request
                    .headers_mut()
                    .append(HEADER_API_TOKEN, token.parse().unwrap());
                if token == "duplicate token" {
                    request
                        .headers_mut()
                        .append(HEADER_API_TOKEN, token.parse().unwrap());
                }
            }
            assert!(
                compute(
                    State(app),
                    axum::extract::ConnectInfo("127.0.0.1:1".parse().unwrap()),
                    request
                )
                .await
                .is_err()
            );
            assert!(!polled.load(std::sync::atomic::Ordering::SeqCst));
        }
    }

    #[test]
    fn native_refusals_are_fixed_statuses_without_private_error_details() {
        assert_eq!(
            native_refusal(PrivateCountersErrorV1::Signature).status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            native_refusal(PrivateCountersErrorV1::Freshness).status(),
            StatusCode::CONFLICT
        );
    }
}
