//! Utilities for Norito encoding and Axum integration.

use std::any::TypeId;
use std::future::Future;

use axum::{
    http::{HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use iroha_data_model::{
    query::{SignedQuery, json_wrappers::SignedQueryJson},
    transaction::{SignedTransaction, TransactionEntrypoint},
};
use iroha_version::Version;
use norito::{
    json::{self, JsonDeserializeOwned, JsonSerialize, Value},
    prelude::*,
};

/// MIME used in Torii for Norito encoding
// note: no elegant way to associate it with generic `NoritoBody<T>`
pub const NORITO_MIME_TYPE: &'_ str = "application/x-norito";
const JSON_MIME_TYPE: &str = "application/json";

fn base_media_type(raw: &str) -> &str {
    raw.split(';').next().map(str::trim).unwrap_or_default()
}

fn is_norito_media_type(raw: &str) -> bool {
    let base = base_media_type(raw);
    !base.is_empty() && base.eq_ignore_ascii_case(NORITO_MIME_TYPE)
}

fn is_json_media_type(raw: &str) -> bool {
    let base = base_media_type(raw);
    if base.is_empty() {
        return false;
    }
    if base.eq_ignore_ascii_case(JSON_MIME_TYPE) || base.eq_ignore_ascii_case("text/json") {
        return true;
    }
    base.to_ascii_lowercase().ends_with("+json")
}

/// Preferred response encoding negotiated from the `Accept` header.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResponseFormat {
    /// Use Norito binary encoding.
    Norito,
    /// Use JSON encoding backed by the Norito JSON codec.
    Json,
}

tokio::task_local! {
    static CURRENT_RESPONSE_FORMAT: ResponseFormat;
}

/// Run a future with the response format negotiated for the current request.
pub async fn with_current_response_format<F>(format: ResponseFormat, future: F) -> F::Output
where
    F: Future,
{
    CURRENT_RESPONSE_FORMAT.scope(format, future).await
}

/// Return the response format associated with the current request.
///
/// Code paths outside an HTTP request use Norito, matching Torii's default wire preference.
pub fn current_response_format() -> ResponseFormat {
    CURRENT_RESPONSE_FORMAT
        .try_with(|format| *format)
        .unwrap_or(ResponseFormat::Norito)
}

impl ResponseFormat {
    fn prefer_for_default(a: Self, b: Self, default_format: Self) -> bool {
        match (a == default_format, b == default_format) {
            (true, false) => true,
            (false, true) => false,
            _ => a == Self::Norito && b == Self::Json,
        }
    }
}

/// Negotiate the response format from an optional `Accept` header value.
///
/// Returns an HTTP response carrying status `406 Not Acceptable` when the header
/// explicitly forbids both JSON and Norito or contains an invalid q-value.
#[allow(clippy::result_large_err)] // callers expect to bubble the full HTTP response on negotiation failure
pub fn negotiate_response_format(accept: Option<&HeaderValue>) -> Result<ResponseFormat, Response> {
    negotiate_response_format_with_default(accept, ResponseFormat::Norito)
}

/// Negotiate a response format for REST-style routes that are JSON by default.
///
/// Explicit `application/x-norito` requests still receive Norito, but omitted
/// or wildcard `Accept` headers receive JSON to match ordinary HTTP clients.
#[allow(clippy::result_large_err)] // callers expect to bubble the full HTTP response on negotiation failure
pub fn negotiate_json_preferred_response_format(
    accept: Option<&HeaderValue>,
) -> Result<ResponseFormat, Response> {
    negotiate_response_format_with_default(accept, ResponseFormat::Json)
}

#[allow(clippy::result_large_err)] // callers expect to bubble the full HTTP response on negotiation failure
fn negotiate_response_format_with_default(
    accept: Option<&HeaderValue>,
    default_format: ResponseFormat,
) -> Result<ResponseFormat, Response> {
    let Some(header) = accept else {
        return Ok(default_format);
    };

    let raw = match header.to_str() {
        Ok(h) => h,
        Err(_) => {
            return Err((
                StatusCode::NOT_ACCEPTABLE,
                "invalid Accept header encoding; supported: application/json, application/x-norito",
            )
                .into_response());
        }
    };

    #[derive(Copy, Clone)]
    struct Candidate {
        format: ResponseFormat,
        quality: f32,
        index: usize,
    }

    const EPS: f32 = 1e-6;
    let mut best: Option<Candidate> = None;

    for (idx, entry) in raw.split(',').enumerate() {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split(';');
        let media_type = parts.next().unwrap().trim();
        let mut quality = 1.0_f32;
        for param in parts {
            let p = param.trim();
            let p_lower = p.to_ascii_lowercase();
            if let Some(rest) = p_lower.strip_prefix("q=") {
                let parsed = rest.parse::<f32>().map_err(|_| {
                    (
                        StatusCode::NOT_ACCEPTABLE,
                        "invalid q-value in Accept header",
                    )
                        .into_response()
                })?;
                if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
                    return Err((
                        StatusCode::NOT_ACCEPTABLE,
                        "invalid q-value in Accept header",
                    )
                        .into_response());
                }
                quality = parsed;
            }
        }

        if quality <= 0.0 {
            continue;
        }

        let format = if is_norito_media_type(media_type) {
            Some(ResponseFormat::Norito)
        } else if media_type.eq_ignore_ascii_case("application/*")
            || media_type.eq_ignore_ascii_case("*/*")
        {
            Some(default_format)
        } else if is_json_media_type(media_type) {
            Some(ResponseFormat::Json)
        } else {
            None
        };

        let Some(format) = format else {
            continue;
        };

        let candidate = Candidate {
            format,
            quality,
            index: idx,
        };

        match best {
            None => best = Some(candidate),
            Some(current) => {
                if candidate.quality > current.quality + EPS
                    || ((candidate.quality - current.quality).abs() <= EPS
                        && (ResponseFormat::prefer_for_default(
                            candidate.format,
                            current.format,
                            default_format,
                        ) || (candidate.format == current.format
                            && candidate.index < current.index)))
                {
                    best = Some(candidate);
                }
            }
        }
    }

    best.map(|c| c.format).ok_or_else(|| {
        (
            StatusCode::NOT_ACCEPTABLE,
            "unsupported Accept header; use application/json or application/x-norito",
        )
            .into_response()
    })
}

/// Validate `Accept` for JSON-only dynamic endpoints.
///
/// Dynamic `norito::json::Value` payloads do not have a stable Norito schema,
/// so JSON-only routes accept omitted, wildcard, and JSON `Accept` values, but
/// reject clients that explicitly ask for Norito without accepting JSON.
#[allow(clippy::result_large_err)] // callers bubble the full HTTP response on negotiation failure
pub fn negotiate_json_only_response(accept: Option<&HeaderValue>) -> Result<(), Response> {
    let Some(header) = accept else {
        return Ok(());
    };

    let raw = match header.to_str() {
        Ok(h) => h,
        Err(_) => {
            return Err((
                StatusCode::NOT_ACCEPTABLE,
                "invalid Accept header encoding; supported: application/json",
            )
                .into_response());
        }
    };

    for entry in raw.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split(';');
        let media_type = parts.next().unwrap().trim();
        let mut quality = 1.0_f32;
        for param in parts {
            let p = param.trim();
            let p_lower = p.to_ascii_lowercase();
            if let Some(rest) = p_lower.strip_prefix("q=") {
                let parsed = rest.parse::<f32>().map_err(|_| {
                    (
                        StatusCode::NOT_ACCEPTABLE,
                        "invalid q-value in Accept header",
                    )
                        .into_response()
                })?;
                if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
                    return Err((
                        StatusCode::NOT_ACCEPTABLE,
                        "invalid q-value in Accept header",
                    )
                        .into_response());
                }
                quality = parsed;
            }
        }

        if quality <= 0.0 {
            continue;
        }

        if is_json_media_type(media_type)
            || media_type.eq_ignore_ascii_case("application/*")
            || media_type.eq_ignore_ascii_case("*/*")
        {
            return Ok(());
        }
    }

    Err((
        StatusCode::NOT_ACCEPTABLE,
        "requested content type is not acceptable for this endpoint; supported: application/json",
    )
        .into_response())
}

/// Encode a response payload using the negotiated format.
pub fn respond_with_format<T>(value: T, format: ResponseFormat) -> Response
where
    T: JsonSerialize + norito::core::NoritoSerialize,
{
    respond_with_status_and_format(StatusCode::OK, value, format)
}

/// Encode a response payload using the given HTTP status and negotiated format.
pub fn respond_with_status_and_format<T>(
    status: StatusCode,
    value: T,
    format: ResponseFormat,
) -> Response
where
    T: JsonSerialize + norito::core::NoritoSerialize,
{
    match format {
        ResponseFormat::Norito => {
            let mut bytes = Vec::new();
            match norito::core::to_bytes_in(&value, &mut bytes) {
                Ok(()) => Response::builder()
                    .status(status)
                    .header(CONTENT_TYPE, HeaderValue::from_static(NORITO_MIME_TYPE))
                    .body(axum::body::Body::from(bytes))
                    .expect("build Norito response"),
                Err(err) => {
                    iroha_logger::error!(?err, "failed to serialise response payload");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to serialise response",
                    )
                        .into_response()
                }
            }
        }
        ResponseFormat::Json => match norito::json::to_vec(&value) {
            Ok(bytes) => Response::builder()
                .status(status)
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .body(axum::body::Body::from(bytes))
                .expect("build JSON response"),
            Err(err) => {
                iroha_logger::error!(?err, "failed to serialise response payload");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to serialise response",
                )
                    .into_response()
            }
        },
    }
}

/// Encode a dynamically constructed Norito JSON value as JSON.
pub fn respond_json_value(value: Value) -> Response {
    respond_json_value_with_status(StatusCode::OK, value)
}

/// Encode a dynamically constructed Norito JSON value as JSON with an HTTP status.
pub fn respond_json_value_with_status(status: StatusCode, value: Value) -> Response {
    match norito::json::to_vec(&value) {
        Ok(bytes) => Response::builder()
            .status(status)
            .header(CONTENT_TYPE, HeaderValue::from_static(JSON_MIME_TYPE))
            .body(axum::body::Body::from(bytes))
            .expect("build JSON response"),
        Err(err) => {
            iroha_logger::error!(?err, "failed to serialise response payload");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to serialise response",
            )
                .into_response()
        }
    }
}

/// Encode a dynamically constructed Norito JSON value.
///
/// Dynamic values do not carry a stable Norito schema, so they remain JSON-only.
/// Use [`respond_with_format`] for typed DTOs that support both JSON and Norito.
pub fn respond_value_with_format(value: Value, _format: ResponseFormat) -> Response {
    respond_json_value(value)
}

#[cfg(test)]
mod response_format_tests {
    use http_body_util::BodyExt as _;

    use super::*;

    #[derive(
        Clone,
        Debug,
        PartialEq,
        norito::derive::NoritoSerialize,
        norito::derive::NoritoDeserialize,
        crate::json_macros::JsonSerialize,
        crate::json_macros::JsonDeserialize,
    )]
    struct DummyPayload {
        value: u32,
    }

    #[tokio::test]
    async fn respond_with_format_produces_norito_bytes() {
        let payload = DummyPayload { value: 42 };
        let (parts, body) =
            respond_with_format(payload.clone(), ResponseFormat::Norito).into_parts();
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(NORITO_MIME_TYPE))
        );
        let bytes = body
            .collect()
            .await
            .expect("collect Norito body")
            .to_bytes();
        let decoded: DummyPayload = norito::decode_from_bytes(&bytes).expect("decode Norito body");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn respond_with_format_produces_json() {
        let payload = DummyPayload { value: 7 };
        let (parts, body) = respond_with_format(payload.clone(), ResponseFormat::Json).into_parts();
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static("application/json"))
        );
        let bytes = body.collect().await.expect("collect JSON body").to_bytes();
        let decoded: DummyPayload = norito::json::from_slice(&bytes).expect("decode JSON body");
        assert_eq!(decoded, payload);
    }

    #[tokio::test]
    async fn respond_value_with_format_keeps_dynamic_payloads_json_only() {
        let value = json::Value::from(7_u64);
        let (parts, body) = respond_value_with_format(value, ResponseFormat::Norito).into_parts();
        assert_eq!(
            parts.headers.get(CONTENT_TYPE),
            Some(&HeaderValue::from_static(JSON_MIME_TYPE))
        );
        let bytes = body.collect().await.expect("collect JSON body").to_bytes();
        let parsed: json::Value = json::from_slice(&bytes).expect("decode JSON payload");
        assert_eq!(parsed, json::Value::from(7_u64));
    }
}

/// Structure to reply using Norito encoding
#[derive(Debug)]
pub struct NoritoBody<T>(pub T);

impl<T: NoritoSerialize + Send> IntoResponse for NoritoBody<T> {
    fn into_response(self) -> Response {
        // Encode with Norito header + checksum so clients can reliably decode.
        let mut buf = Vec::new();
        norito::core::to_bytes_in(&self.0, &mut buf).expect("norito serialization failed");
        let mut res = Response::new(buf.into());
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static(NORITO_MIME_TYPE));
        res
    }
}

/// Structure to reply with a dynamically constructed JSON value.
///
/// Prefer typed DTOs plus [`NoritoBody`] or [`JsonBody`] for dual-format routes.
#[derive(Debug)]
pub struct JsonValueBody(pub Value);

impl IntoResponse for JsonValueBody {
    fn into_response(self) -> Response {
        respond_json_value(self.0)
    }
}

/// Structure to reply using Norito-backed JSON encoding.
#[derive(Debug)]
pub struct JsonBody<T>(pub T);

impl<T: JsonSerialize + Send> IntoResponse for JsonBody<T> {
    fn into_response(self) -> Response {
        // Serialize using Norito's JSON codec and attach the appropriate MIME type header.
        let buf = norito::json::to_vec(&self.0).expect("json serialization failed");
        let mut res = Response::new(buf.into());
        res.headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        res
    }
}

pub mod extractors {
    use axum::{
        body::Bytes,
        extract::{FromRequest, FromRequestParts, OptionalFromRequestParts, Request},
        http::StatusCode,
    };
    use norito::{
        core::NoritoDeserialize,
        json::{self, JsonDeserializeOwned, Number, Value},
    };
    use urlencoding::decode;

    use super::*;

    /// Extractor of Norito-encoded, versioned data from the request body.
    ///
    /// Missing or unsupported `Content-Type` yields `415 Unsupported Media Type`;
    /// decode failures surface as `400 Bad Request` to distinguish payload issues from negotiation.
    #[derive(Clone, Copy, Debug)]
    pub struct Norito<T>(pub T);

    impl<S, T> FromRequest<S> for Norito<T>
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
        T: SupportsNoritoDecode + Send + 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            if !super::is_norito_media_type(raw) {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}` (got `{raw}`)",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            }

            let body = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;

            decode_as_norito::<T>(&body).map(Norito)
        }
    }

    /// Extractor of Norito-encoded, versioned data from the request body.
    ///
    /// Missing or unsupported `Content-Type` yields `415 Unsupported Media Type`;
    /// decode failures surface as `400 Bad Request` to distinguish payload issues from negotiation.
    #[derive(Clone, Copy, Debug)]
    pub struct NoritoVersioned<T>(pub T);

    /// Extractor of raw Norito-versioned bytes from the request body.
    ///
    /// Missing or unsupported `Content-Type` yields `415 Unsupported Media Type`.
    /// Callers that need exact payload bytes can decode the returned buffer with
    /// the appropriate versioned decoder.
    #[derive(Clone, Debug)]
    pub struct NoritoVersionedBytes(pub Bytes);

    impl<S> FromRequest<S> for NoritoVersionedBytes
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            if !super::is_norito_media_type(raw) {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}` (got `{raw}`)",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            }

            Bytes::from_request(req, state)
                .await
                .map(NoritoVersionedBytes)
                .map_err(IntoResponse::into_response)
        }
    }

    impl<S, T> FromRequest<S> for NoritoVersioned<T>
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
        // Accept payloads encoded with Norito + iroha_version leading byte
        T: iroha_version::codec::DecodeVersioned,
        T: 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            if !super::is_norito_media_type(raw) {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "Norito requests must set `Content-Type: {}` (got `{raw}`)",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            }

            let body = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;

            match <T as iroha_version::codec::DecodeVersioned>::decode_all_versioned(&body) {
                Ok(val) => Ok(NoritoVersioned(val)),
                Err(versioned_err) => Err((
                    axum::http::StatusCode::BAD_REQUEST,
                    format!("Could not decode versioned request: {versioned_err}"),
                )
                    .into_response()),
            }
        }
    }

    fn version_from_json_value(value: &Value) -> Option<u8> {
        match value {
            Value::Number(number) => u8::try_from(number.as_u64()?).ok(),
            Value::String(raw) => raw.parse::<u8>().ok(),
            _ => None,
        }
    }

    #[allow(clippy::result_large_err)]
    fn decode_versioned_json<T>(body: &Bytes, expected: &'static str) -> Result<T, Response>
    where
        T: JsonDeserializeOwned + Version,
    {
        let value = json::from_slice::<Value>(body.as_ref()).map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("invalid JSON body: {e}")).into_response()
        })?;
        let object = value.as_object().ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid JSON body: expected versioned {expected} object"),
            )
                .into_response()
        })?;
        let version = object
            .get("version")
            .and_then(version_from_json_value)
            .ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    "invalid JSON body: missing numeric `version` field",
                )
                    .into_response()
            })?;
        if !T::supported_versions().contains(&version) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unsupported JSON payload version `{version}` for {expected}"),
            )
                .into_response());
        }
        let content = object.get("content").ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                "invalid JSON body: missing `content` field",
            )
                .into_response()
        })?;
        json::from_value::<T>(content.clone()).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("invalid JSON {expected} content: {e}"),
            )
                .into_response()
        })
    }

    /// Helper trait exposing versioned JSON decoding for selected Torii ingress payloads.
    pub trait SupportsVersionedJsonDecode: Sized {
        /// Decode a JSON request body carrying a versioned payload.
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response>;
    }

    impl SupportsVersionedJsonDecode for SignedTransaction {
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response> {
            decode_versioned_json::<Self>(body, "SignedTransaction")
        }
    }

    impl SupportsVersionedJsonDecode for TransactionEntrypoint {
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response> {
            decode_versioned_json::<Self>(body, "TransactionEntrypoint")
        }
    }

    impl SupportsVersionedJsonDecode for SignedQuery {
        fn decode_versioned_json_body(body: &Bytes) -> Result<Self, Response> {
            let json = json::from_slice::<SignedQueryJson>(body.as_ref()).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("invalid JSON SignedQuery body: {e}"),
                )
                    .into_response()
            })?;
            json.try_into().map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("invalid JSON SignedQuery content: {e}"),
                )
                    .into_response()
            })
        }
    }

    /// Extractor for versioned request bodies supporting both Norito and JSON payloads.
    #[derive(Clone, Copy, Debug)]
    pub struct JsonOrNoritoVersioned<T>(pub T);

    impl<S, T> FromRequest<S> for JsonOrNoritoVersioned<T>
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
        T: iroha_version::codec::DecodeVersioned + SupportsVersionedJsonDecode + 'static,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let declared = req
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty())
                .map(str::to_owned);

            let Some(raw) = declared.as_deref() else {
                return Err((
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                    format!(
                        "versioned requests must set `Content-Type: {}` or `Content-Type: application/json`",
                        super::NORITO_MIME_TYPE
                    ),
                )
                    .into_response());
            };

            let body = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;

            if super::is_norito_media_type(raw) {
                return <T as iroha_version::codec::DecodeVersioned>::decode_all_versioned(&body)
                    .map(JsonOrNoritoVersioned)
                    .map_err(|versioned_err| {
                        (
                            StatusCode::BAD_REQUEST,
                            format!("Could not decode versioned request: {versioned_err}"),
                        )
                            .into_response()
                    });
            }

            if super::is_json_media_type(raw) {
                return T::decode_versioned_json_body(&body).map(JsonOrNoritoVersioned);
            }

            Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "unsupported Content-Type `{raw}`; use application/json or {}",
                    super::NORITO_MIME_TYPE
                ),
            )
                .into_response())
        }
    }

    /// Extractor of Accept header
    #[cfg_attr(not(feature = "telemetry"), allow(unused))]
    pub struct ExtractAccept(pub HeaderValue);

    impl<S> FromRequestParts<S> for ExtractAccept
    where
        S: Send + Sync,
    {
        type Rejection = (StatusCode, &'static str);

        fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> impl core::future::Future<Output = Result<Self, Self::Rejection>> + Send {
            let res = parts
                .headers
                .get(axum::http::header::ACCEPT)
                .cloned()
                .map(ExtractAccept)
                .ok_or((StatusCode::BAD_REQUEST, "`Accept` header is missing"));
            core::future::ready(res)
        }
    }

    impl<S> OptionalFromRequestParts<S> for ExtractAccept
    where
        S: Send + Sync,
    {
        type Rejection = (StatusCode, &'static str);

        fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> impl core::future::Future<Output = Result<Option<Self>, Self::Rejection>> + Send
        {
            let value = parts
                .headers
                .get(axum::http::header::ACCEPT)
                .cloned()
                .map(ExtractAccept);
            core::future::ready(Ok(value))
        }
    }

    /// Helper trait exposing Norito decoding for extractor-bound types.
    pub trait SupportsNoritoDecode: Sized {
        fn decode_norito(bytes: &[u8]) -> Result<Self, norito::Error>;
    }

    impl<T> SupportsNoritoDecode for T
    where
        for<'a> T: NoritoDeserialize<'a>,
    {
        fn decode_norito(bytes: &[u8]) -> Result<Self, norito::Error> {
            norito::decode_from_bytes::<T>(bytes)
        }
    }

    #[allow(clippy::result_large_err)] // extraction needs to return a fully-formed HTTP rejection response
    fn decode_as_json<T: JsonDeserializeOwned>(body: &Bytes) -> Result<T, Response> {
        norito::json::from_slice::<T>(body.as_ref()).map_err(|e| {
            (StatusCode::BAD_REQUEST, format!("invalid JSON body: {e}")).into_response()
        })
    }

    fn payload_kind_label<T: 'static>() -> &'static str {
        if TypeId::of::<T>() == TypeId::of::<SignedTransaction>() {
            "signed_transaction"
        } else if TypeId::of::<T>() == TypeId::of::<SignedQuery>() {
            "signed_query"
        } else {
            "other"
        }
    }

    fn record_payload_decode_failure<T: 'static>(error: &norito::Error) {
        record_norito_decode_failure(payload_kind_label::<T>(), error);
    }

    #[allow(clippy::result_large_err)] // extraction needs to return a fully-formed HTTP rejection response
    fn decode_as_norito<T: SupportsNoritoDecode + 'static>(body: &Bytes) -> Result<T, Response> {
        T::decode_norito(body.as_ref()).map_err(|e| {
            record_payload_decode_failure::<T>(&e);
            (StatusCode::BAD_REQUEST, format!("invalid Norito body: {e}")).into_response()
        })
    }

    #[cfg(feature = "telemetry")]
    fn record_norito_decode_failure(payload_kind: &'static str, error: &norito::Error) {
        iroha_telemetry::metrics::global_or_default()
            .inc_torii_norito_decode_failure(payload_kind, classify_norito_error(error));
    }

    #[cfg(feature = "telemetry")]
    fn classify_norito_error(error: &norito::Error) -> &'static str {
        use norito::Error;

        match error {
            Error::InvalidMagic => "invalid_magic",
            Error::UnsupportedVersion { .. } => "unsupported_version",
            Error::UnsupportedMinorVersion { .. } => "unsupported_minor_version",
            Error::UnsupportedCompression { .. } => "unsupported_compression",
            Error::LengthMismatch => "length_mismatch",
            Error::ArchiveLengthExceeded { .. } => "archive_length_exceeded",
            Error::ChecksumMismatch => "checksum_mismatch",
            Error::SchemaMismatch => "schema_mismatch",
            Error::UnsupportedFeature(flag) => match *flag {
                "layout flag" => "unsupported_feature_layout_flag",
                _ => "unsupported_feature",
            },
            Error::MissingPayloadContext => "missing_payload_context",
            Error::MissingLayoutFlags => "missing_layout_flags",
            Error::InvalidUtf8 => "invalid_utf8",
            Error::InvalidTag { .. } => "invalid_tag",
            Error::InvalidNonZero => "invalid_non_zero",
            Error::DecodePanic { .. } => "decode_panic",
            Error::Misaligned { .. } => "misaligned",
            Error::Io(_) => "io_error",
            Error::Message(_) => "message",
            #[cfg(feature = "json")]
            Error::Json(_) => "json_error",
            _ => "other",
        }
    }

    #[cfg(not(feature = "telemetry"))]
    fn record_norito_decode_failure(_: &'static str, _: &norito::Error) {}

    #[allow(clippy::result_large_err)]
    fn decode_body_as_norito_or_json<T: JsonDeserializeOwned + SupportsNoritoDecode + 'static>(
        body: &Bytes,
        declared: Option<&str>,
    ) -> Result<T, Response> {
        let Some(ct) = declared else {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                format!(
                    "missing Content-Type; use application/json or {}",
                    super::NORITO_MIME_TYPE
                ),
            )
                .into_response());
        };
        if super::is_norito_media_type(ct) {
            return decode_as_norito::<T>(body);
        }
        if super::is_json_media_type(ct) {
            return decode_as_json::<T>(body);
        }
        Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!(
                "unsupported Content-Type `{ct}`; use application/json or {}",
                super::NORITO_MIME_TYPE
            ),
        )
            .into_response())
    }

    /// Extractor for request bodies supporting both Norito and JSON payloads.
    #[derive(Clone, Copy, Debug)]
    pub struct NoritoJson<T>(pub T);

    impl<S, T> FromRequest<S> for NoritoJson<T>
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send + 'static,
        T: SupportsNoritoDecode,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let content_type = req.headers().get(CONTENT_TYPE).cloned();
            let body = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
            let declared = content_type
                .as_ref()
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty());

            decode_body_as_norito_or_json::<T>(&body, declared).map(NoritoJson)
        }
    }

    /// Extractor that returns both the decoded payload and the raw request body.
    #[derive(Clone, Debug)]
    pub struct NoritoJsonWithBytes<T> {
        /// Parsed payload decoded from Norito or JSON.
        pub value: T,
        /// Raw request body bytes as sent over the wire.
        pub raw: Bytes,
    }

    impl<S, T> FromRequest<S> for NoritoJsonWithBytes<T>
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send + 'static,
        T: SupportsNoritoDecode,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let content_type = req.headers().get(CONTENT_TYPE).cloned();
            let body = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
            let declared = content_type
                .as_ref()
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty());

            decode_body_as_norito_or_json::<T>(&body, declared)
                .map(|value| NoritoJsonWithBytes { value, raw: body })
        }
    }

    /// Extractor enforcing JSON payloads decoded with the Norito JSON codec.
    #[derive(Clone, Debug)]
    pub struct JsonOnly<T>(pub T);

    impl<S, T> FromRequest<S> for JsonOnly<T>
    where
        Bytes: FromRequest<S>,
        S: Send + Sync,
        T: JsonDeserializeOwned + Send,
    {
        type Rejection = Response;

        async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
            let content_type = req.headers().get(CONTENT_TYPE).cloned();
            let body = Bytes::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
            let declared = content_type
                .as_ref()
                .and_then(|hv| hv.to_str().ok())
                .map(str::trim)
                .filter(|ct| !ct.is_empty());

            if let Some(ct) = declared {
                if !super::is_json_media_type(ct) {
                    return Err((
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        format!(
                            "unsupported Content-Type `{ct}`; expected application/json or text/json"
                        ),
                    )
                        .into_response());
                }
            }

            decode_as_json::<T>(&body).map(JsonOnly)
        }
    }

    /// Extractor for URL query strings decoded into `JsonDeserialize` types.
    #[derive(Clone, Debug)]
    pub struct NoritoQuery<T>(pub T);

    impl<S, T> FromRequestParts<S> for NoritoQuery<T>
    where
        S: Send + Sync,
        T: JsonDeserializeOwned + Send,
    {
        type Rejection = Response;

        async fn from_request_parts(
            parts: &mut axum::http::request::Parts,
            _state: &S,
        ) -> Result<Self, Self::Rejection> {
            let query = parts.uri.query().unwrap_or("");
            match decode_query::<T>(query) {
                Ok(value) => Ok(NoritoQuery(value)),
                Err(e) => Err((
                    StatusCode::BAD_REQUEST,
                    format!("invalid query params: {e}"),
                )
                    .into_response()),
            }
        }
    }

    fn decode_query<T: JsonDeserializeOwned>(query: &str) -> Result<T, json::Error> {
        let mut object = json::Map::new();
        for (key, value) in query_pairs(query) {
            object.insert(key, scalar_to_value(&value));
        }
        json::from_value(Value::Object(object))
    }

    fn query_pairs(query: &str) -> Vec<(String, String)> {
        query
            .split('&')
            .filter(|segment| !segment.is_empty())
            .map(|segment| {
                let mut parts = segment.splitn(2, '=');
                let raw_key = parts.next().unwrap_or("");
                let raw_value = parts.next().unwrap_or("");
                (decode_component(raw_key), decode_component(raw_value))
            })
            .collect()
    }

    fn decode_component(input: &str) -> String {
        let replaced = input.replace('+', " ");
        decode(&replaced)
            .unwrap_or_else(|_| std::borrow::Cow::Borrowed(&replaced))
            .into_owned()
    }

    fn scalar_to_value(raw: &str) -> Value {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("null") {
            Value::Null
        } else if trimmed.eq_ignore_ascii_case("true") {
            Value::Bool(true)
        } else if trimmed.eq_ignore_ascii_case("false") {
            Value::Bool(false)
        } else if let Ok(u) = trimmed.parse::<u64>() {
            Value::Number(Number::from(u))
        } else if let Ok(i) = trimmed.parse::<i64>() {
            Value::Number(Number::from(i))
        } else if let Ok(f) = trimmed.parse::<f64>() {
            Number::from_f64(f)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(trimmed.to_string()))
        } else {
            Value::String(trimmed.to_string())
        }
    }

    #[cfg(test)]
    mod tests {
        use axum::{
            body::Body,
            http::{Request, StatusCode, header::CONTENT_TYPE},
        };
        use http_body_util::BodyExt as _;
        use iroha_version::{RawVersioned, UnsupportedVersion, Version};
        use norito::core::{NoritoDeserialize, NoritoSerialize};

        use super::*;

        #[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
        struct Dummy(u32);

        impl Version for Dummy {
            fn version(&self) -> u8 {
                1
            }

            fn supported_versions() -> std::ops::Range<u8> {
                1..2
            }
        }

        impl iroha_version::codec::DecodeVersioned for Dummy {
            fn decode_all_versioned(input: &[u8]) -> iroha_version::error::Result<Self> {
                let (version, rest) = input
                    .split_first()
                    .ok_or(iroha_version::error::Error::NotVersioned)?;
                if *version != 1 {
                    return Err(iroha_version::error::Error::UnsupportedVersion(Box::new(
                        UnsupportedVersion::new(*version, RawVersioned::NoritoBytes(rest.to_vec())),
                    )));
                }
                norito::decode_from_bytes(rest).map_err(Into::into)
            }
        }

        #[tokio::test]
        async fn decodes_versioned_payload() {
            let mut payload = vec![1];
            payload
                .extend_from_slice(&norito::to_bytes(&Dummy(42)).expect("encode versioned dummy"));
            let req = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(payload))
                .unwrap();
            let extracted = NoritoVersioned::<Dummy>::from_request(req, &())
                .await
                .expect("extract versioned");
            assert_eq!(extracted.0, Dummy(42));
        }

        #[tokio::test]
        async fn rejects_missing_content_type() {
            let body = norito::to_bytes(&Dummy(7)).expect("encode bare dummy");
            let req = Request::new(Body::from(body));
            let err = NoritoVersioned::<Dummy>::from_request(req, &())
                .await
                .expect_err("missing content-type should be rejected");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn surfaces_decode_error() {
            let req = Request::builder()
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(vec![2]))
                .expect("request");
            let err = NoritoVersioned::<Dummy>::from_request(req, &())
                .await
                .expect_err("should fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);
            let body_bytes = http_body_util::BodyExt::collect(err.into_body())
                .await
                .expect("collect error body")
                .to_bytes();
            let body_text = String::from_utf8(body_bytes.to_vec()).expect("response text");
            assert!(
                body_text.contains("Could not decode versioned request"),
                "body should describe decode failure: {body_text}"
            );
            assert!(
                body_text.to_ascii_lowercase().contains("version"),
                "body should mention versioned decode reason: {body_text}"
            );
        }

        #[tokio::test]
        async fn content_type_matrix_maps_to_status_codes() {
            let versioned_ok = {
                let mut payload = vec![1];
                payload.extend_from_slice(
                    &norito::to_bytes(&Dummy(11)).expect("encode versioned dummy"),
                );
                payload
            };
            let bare_ok = norito::to_bytes(&Dummy(7)).expect("encode bare dummy");
            let cases = [
                (
                    "missing",
                    None,
                    bare_ok.clone(),
                    Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
                    Some("Content-Type"),
                ),
                (
                    "wrong",
                    Some("text/plain"),
                    bare_ok.clone(),
                    Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
                    Some("Content-Type"),
                ),
                (
                    "versioned decode fail",
                    Some(super::super::NORITO_MIME_TYPE),
                    vec![2_u8],
                    Err(StatusCode::BAD_REQUEST),
                    Some("versioned"),
                ),
                (
                    "bare norito is rejected",
                    Some(super::super::NORITO_MIME_TYPE),
                    bare_ok,
                    Err(StatusCode::BAD_REQUEST),
                    Some("versioned"),
                ),
                (
                    "norito with parameters succeeds",
                    Some("application/x-norito; charset=utf-8"),
                    versioned_ok.clone(),
                    Ok(Dummy(11)),
                    None,
                ),
                (
                    "plain norito succeeds",
                    Some(super::super::NORITO_MIME_TYPE),
                    versioned_ok,
                    Ok(Dummy(11)),
                    None,
                ),
            ];

            for (label, content_type, payload, expected, snippet) in cases {
                let mut builder = Request::builder();
                if let Some(ct) = content_type {
                    builder = builder.header(CONTENT_TYPE, ct);
                }
                let req = builder.body(Body::from(payload)).expect("request");
                let result = NoritoVersioned::<Dummy>::from_request(req, &()).await;
                match expected {
                    Ok(expected_payload) => {
                        let ok = result.unwrap_or_else(|resp| {
                            panic!("{label} should succeed, got {}", resp.status())
                        });
                        assert_eq!(ok.0, expected_payload, "case `{label}` payload mismatch");
                    }
                    Err(expected_status) => {
                        let err = result.expect_err(label);
                        assert_eq!(err.status(), expected_status, "case `{label}`");
                        if let Some(snippet) = snippet {
                            let body = http_body_util::BodyExt::collect(err.into_body())
                                .await
                                .expect("collect body")
                                .to_bytes();
                            let text = String::from_utf8_lossy(&body);
                            assert!(
                                text.to_ascii_lowercase()
                                    .contains(&snippet.to_ascii_lowercase()),
                                "case `{label}` should mention `{snippet}`: {text}"
                            );
                        }
                    }
                }
            }
        }

        #[test]
        fn negotiate_accept_header_prefers_norito() {
            let header =
                HeaderValue::from_static("application/json;q=0.5, application/x-norito;q=0.9");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_honors_json() {
            let header = HeaderValue::from_static("application/json");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_accept_header_defaults_norito() {
            let format = super::super::negotiate_response_format(None).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_wildcard_defaults_norito() {
            let header = HeaderValue::from_static("*/*");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_json_preferred_defaults_json() {
            let format =
                super::super::negotiate_json_preferred_response_format(None).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_json_preferred_wildcard_defaults_json() {
            let header = HeaderValue::from_static("*/*");
            let format = super::super::negotiate_json_preferred_response_format(Some(&header))
                .expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_json_preferred_preserves_explicit_norito() {
            let header = HeaderValue::from_static("application/x-norito");
            let format = super::super::negotiate_json_preferred_response_format(Some(&header))
                .expect("format");
            assert_eq!(format, super::super::ResponseFormat::Norito);
        }

        #[test]
        fn negotiate_accept_header_accepts_vendor_json() {
            let header = HeaderValue::from_static("application/vnd.api+json");
            let format = super::super::negotiate_response_format(Some(&header)).expect("format");
            assert_eq!(format, super::super::ResponseFormat::Json);
        }

        #[test]
        fn negotiate_rejects_unsupported_media_type() {
            let header = HeaderValue::from_static("text/plain");
            let err = super::super::negotiate_response_format(Some(&header)).unwrap_err();
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn negotiate_rejects_invalid_q_value() {
            let header = HeaderValue::from_static("application/json;q=2");
            let err = super::super::negotiate_response_format(Some(&header)).unwrap_err();
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[test]
        fn negotiate_json_only_accepts_missing_json_and_wildcard() {
            assert!(super::super::negotiate_json_only_response(None).is_ok());
            let json = HeaderValue::from_static("application/json");
            assert!(super::super::negotiate_json_only_response(Some(&json)).is_ok());
            let wildcard = HeaderValue::from_static("*/*");
            assert!(super::super::negotiate_json_only_response(Some(&wildcard)).is_ok());
        }

        #[test]
        fn negotiate_json_only_rejects_norito_only() {
            let header = HeaderValue::from_static("application/x-norito");
            let err = super::super::negotiate_json_only_response(Some(&header)).unwrap_err();
            assert_eq!(err.status(), StatusCode::NOT_ACCEPTABLE);
        }

        #[tokio::test]
        async fn norito_json_accepts_binary_body() {
            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::to_bytes(&Payload { value: 42 }).expect("norito encode");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(body_bytes))
                .expect("build request");
            let extracted = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect("extract norito");
            assert_eq!(extracted.0.value, 42);
        }

        #[tokio::test]
        async fn norito_json_accepts_json_body() {
            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::json::to_vec(&Payload { value: 7 }).expect("json encode");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body_bytes))
                .expect("build request");
            let extracted = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect("extract json");
            assert_eq!(extracted.0.value, 7);
        }

        #[tokio::test]
        async fn json_only_accepts_charset_and_suffix_json() {
            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body_bytes = norito::json::to_vec(&Payload { value: 9 }).expect("json encode");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/json; charset=utf-8")
                .body(Body::from(body_bytes.clone()))
                .expect("build request");
            let extracted = JsonOnly::<Payload>::from_request(req, &())
                .await
                .expect("extract json");
            assert_eq!(extracted.0.value, 9);

            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/ld+json")
                .body(Body::from(body_bytes))
                .expect("build request");
            let extracted = JsonOnly::<Payload>::from_request(req, &())
                .await
                .expect("extract json suffix");
            assert_eq!(extracted.0.value, 9);
        }

        #[tokio::test]
        async fn norito_json_rejects_unsupported_content_type() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "text/plain")
                .body(Body::from("hello"))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("unsupported content-type");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn norito_json_rejects_missing_content_type() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let req = Request::builder()
                .method("POST")
                .body(Body::from("{}"))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("missing content-type");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[tokio::test]
        async fn norito_json_rejects_octet_stream_fallback() {
            #[derive(
                Clone,
                Debug,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload;

            let body = norito::to_bytes(&Payload).expect("encode norito payload");
            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(body))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("octet-stream content-type");
            assert_eq!(err.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
        }

        #[cfg(feature = "telemetry")]
        #[tokio::test]
        async fn norito_json_decode_failure_increments_metric() {
            use iroha_telemetry::metrics::global_or_default;

            #[derive(
                Clone,
                Debug,
                PartialEq,
                NoritoSerialize,
                NoritoDeserialize,
                crate::json_macros::JsonSerialize,
                crate::json_macros::JsonDeserialize,
            )]
            struct Payload {
                value: u32,
            }

            let body = vec![0_u8; 4];
            let reason = super::classify_norito_error(
                &norito::decode_from_bytes::<Payload>(&body).expect_err("body must fail"),
            );
            let payload_kind = super::payload_kind_label::<Payload>();
            let metrics = global_or_default();
            let before = metrics
                .torii_norito_decode_failures_total
                .with_label_values(&[payload_kind, reason])
                .get();

            let req = Request::builder()
                .method("POST")
                .header(CONTENT_TYPE, super::super::NORITO_MIME_TYPE)
                .body(Body::from(body))
                .expect("build request");
            let err = NoritoJson::<Payload>::from_request(req, &())
                .await
                .expect_err("decode must fail");
            assert_eq!(err.status(), StatusCode::BAD_REQUEST);

            let after = metrics
                .torii_norito_decode_failures_total
                .with_label_values(&[payload_kind, reason])
                .get();
            assert_eq!(after, before + 1, "decode metric should increment");
        }
    }
}
