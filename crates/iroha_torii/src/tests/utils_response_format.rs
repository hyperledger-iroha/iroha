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
#[derive(norito::derive::NoritoSerialize)]
struct LegacyJsonSerializerMustNotRun;
impl norito::json::JsonSerialize for LegacyJsonSerializerMustNotRun {
    fn json_serialize(&self, _out: &mut String) {
        panic!("bounded encoding must not invoke a legacy String serializer");
    }
}
#[tokio::test]
async fn respond_with_format_produces_norito_bytes() {
    let payload = DummyPayload { value: 42 };
    let (parts, body) = respond_with_format(payload.clone(), ResponseFormat::Norito).into_parts();
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
async fn bounded_response_accepts_exact_norito_limit_and_rejects_next_byte() {
    let payload = DummyPayload { value: 99 };
    let exact = norito::core::encoded_frame_len(&payload).expect("count payload frame");
    let (_, body) = respond_with_format_bounded(payload.clone(), ResponseFormat::Norito, exact)
        .expect("the exact body reservation must fit")
        .into_parts();
    assert_eq!(
        body.collect()
            .await
            .expect("collect exact bounded body")
            .to_bytes()
            .len(),
        exact
    );
    let error = match respond_with_format_bounded(payload, ResponseFormat::Norito, exact - 1) {
        Ok(_) => panic!("one byte below the exact frame must fail"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        BoundedResponseEncodeError::BodyTooLarge {
            encoded_bytes: exact,
            max_body_bytes: exact - 1,
        }
    );
}
#[test]
fn bounded_response_rejects_legacy_json_without_invoking_its_string_serializer() {
    let error = match respond_with_format_bounded(
        LegacyJsonSerializerMustNotRun,
        ResponseFormat::Json,
        usize::MAX,
    ) {
        Ok(_) => panic!("legacy JSON must not enter the bounded response path"),
        Err(error) => error,
    };
    assert_eq!(error, BoundedResponseEncodeError::Serialization);
}
#[tokio::test]
async fn bounded_response_accepts_exact_json_limit_and_rejects_next_byte() {
    let payload = DummyPayload { value: 99 };
    let exact = norito::json::to_string(&payload)
        .expect("encode fixture")
        .len();
    let (parts, body) = respond_with_format_bounded(payload.clone(), ResponseFormat::Json, exact)
        .expect("the exact JSON body reservation must fit")
        .into_parts();
    assert_eq!(
        parts.headers.get(CONTENT_TYPE),
        Some(&HeaderValue::from_static(JSON_MIME_TYPE))
    );
    let bytes = body
        .collect()
        .await
        .expect("collect exact bounded JSON body")
        .to_bytes();
    assert_eq!(bytes.len(), exact);
    let decoded: DummyPayload = norito::json::from_slice(&bytes).expect("decode bounded JSON");
    assert_eq!(decoded, payload);
    let error = match respond_with_format_bounded(payload, ResponseFormat::Json, exact - 1) {
        Ok(_) => panic!("one byte below the exact JSON body must fail"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        BoundedResponseEncodeError::JsonBodyTooLarge {
            max_body_bytes: exact - 1,
        }
    );
}
#[test]
fn bounded_json_helper_accepts_exact_limit_and_rejects_one_byte_less() {
    let payload = DummyPayload { value: 7 };
    let ordinary = norito::json::to_string(&payload).expect("ordinary JSON");
    assert_eq!(
        encode_json_bounded(&payload, ordinary.len()).expect("exact JSON cap"),
        ordinary
    );
    assert_eq!(
        encode_json_bounded(&payload, ordinary.len() - 1)
            .expect_err("one byte below exact JSON must fail"),
        norito::json::BoundedJsonError::BodyTooLarge
    );
}
#[test]
fn bounded_json_helper_supports_the_iterable_query_response_envelope() {
    use iroha_data_model::query::{
        QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryResponse,
    };
    let response = QueryResponse::Iterable(QueryOutput {
        batch: QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(vec![
            "bounded".to_owned(),
        ])),
        remaining_items: Some(0),
        has_more: false,
        continue_cursor: None,
    });
    let ordinary = norito::json::to_string(&response).expect("ordinary query JSON");
    assert_eq!(
        encode_json_bounded(&response, ordinary.len()).expect("bounded query JSON"),
        ordinary
    );
}
#[test]
fn typed_error_response_carries_bounded_telemetry_code() {
    let response = respond_with_status_and_format(
        StatusCode::CONFLICT,
        ErrorEnvelope::new("idempotency_key_conflict", "conflict"),
        ResponseFormat::Json,
    );
    assert_eq!(
        response
            .extensions()
            .get::<HttpErrorCode>()
            .map(HttpErrorCode::as_str),
        Some("idempotency_key_conflict")
    );
    let response = respond_with_status_and_format(
        StatusCode::BAD_REQUEST,
        ErrorEnvelope::new("raw/value/from/request", "invalid"),
        ResponseFormat::Norito,
    );
    assert_eq!(
        response
            .extensions()
            .get::<HttpErrorCode>()
            .map(HttpErrorCode::as_str),
        Some("invalid_error_code")
    );
}
#[tokio::test]
async fn unacceptable_representation_uses_typed_json_fallback() {
    let header = HeaderValue::from_static("image/png");
    let response = negotiate_response_format(Some(&header))
        .expect_err("unsupported representation must be rejected");
    let (parts, body) = response.into_parts();
    assert_eq!(parts.status, StatusCode::NOT_ACCEPTABLE);
    assert_eq!(
        parts.headers.get(CONTENT_TYPE),
        Some(&HeaderValue::from_static(JSON_MIME_TYPE))
    );
    let bytes = body.collect().await.expect("collect error body").to_bytes();
    let envelope: iroha_torii_shared::ErrorEnvelope =
        norito::json::from_slice(&bytes).expect("decode typed error envelope");
    assert_eq!(envelope.code(), "response_not_acceptable");
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
#[tokio::test]
async fn respond_json_document_with_format_wraps_json_string_as_norito() {
    let mut object = json::Map::new();
    object.insert("ok".to_owned(), json::Value::Bool(true));
    let (parts, body) = respond_json_document_with_status_and_format(
        StatusCode::ACCEPTED,
        json::Value::Object(object),
        ResponseFormat::Norito,
    )
    .into_parts();
    assert_eq!(parts.status, StatusCode::ACCEPTED);
    assert_eq!(
        parts.headers.get(CONTENT_TYPE),
        Some(&HeaderValue::from_static(NORITO_MIME_TYPE))
    );
    let bytes = body
        .collect()
        .await
        .expect("collect Norito body")
        .to_bytes();
    let json: String = norito::decode_from_bytes(&bytes).expect("decode Norito JSON string");
    let decoded: json::Value = json::from_str(&json).expect("decode JSON document");
    assert_eq!(decoded["ok"].as_bool(), Some(true));
}
#[tokio::test]
async fn respond_json_document_with_format_renders_json() {
    let mut object = json::Map::new();
    object.insert("ok".to_owned(), json::Value::Bool(true));
    let (parts, body) = respond_json_document_with_status_and_format(
        StatusCode::ACCEPTED,
        json::Value::Object(object),
        ResponseFormat::Json,
    )
    .into_parts();
    assert_eq!(parts.status, StatusCode::ACCEPTED);
    assert_eq!(
        parts.headers.get(CONTENT_TYPE),
        Some(&HeaderValue::from_static(JSON_MIME_TYPE))
    );
    let bytes = body.collect().await.expect("collect JSON body").to_bytes();
    let decoded: json::Value = json::from_slice(&bytes).expect("decode JSON document");
    assert_eq!(decoded["ok"].as_bool(), Some(true));
}
