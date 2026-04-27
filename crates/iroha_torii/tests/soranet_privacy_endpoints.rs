#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration coverage for Torii `SoraNet` privacy ingestion endpoints.
#![cfg(feature = "telemetry")]

use std::time::{SystemTime, UNIX_EPOCH};

use axum::{http::StatusCode, response::IntoResponse};
use http_body_util::BodyExt;
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_data_model::soranet::privacy_metrics::{
    SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1, SoranetPrivacyEventV1,
    SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1,
};
use iroha_torii::{
    MaybeTelemetry, NoritoJson, RecordSoranetPrivacyEventDto, RecordSoranetPrivacyShareDto,
    handle_post_soranet_privacy_event, handle_post_soranet_privacy_share,
};
use norito::json::Value;

async fn norito_json_response_value(response: axum::response::Response) -> Value {
    let body_bytes = BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body_json: String = norito::decode_from_bytes(&body_bytes).unwrap();
    norito::json::from_str(&body_json).unwrap()
}

#[tokio::test]
async fn privacy_event_endpoint_rejects_when_disabled() {
    let telemetry = MaybeTelemetry::disabled();
    let event = SoranetPrivacyEventV1 {
        timestamp_unix: 1_720_000_060,
        mode: SoranetPrivacyModeV1::Middle,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: Some(120),
            active_circuits_after: Some(42),
        }),
    };
    let dto = RecordSoranetPrivacyEventDto {
        event,
        source: Some("disabled-test".into()),
    };

    let result = handle_post_soranet_privacy_event(telemetry, NoritoJson(dto)).await;
    let err = match result {
        Ok(_) => panic!("disabled telemetry must reject privacy events"),
        Err(err) => err,
    };
    assert_eq!(
        err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn privacy_event_endpoint_accepts_payload() {
    let telemetry = MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
    let timestamp = 1_720_000_123;
    let event = SoranetPrivacyEventV1 {
        timestamp_unix: timestamp,
        mode: SoranetPrivacyModeV1::Entry,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: Some(88),
            active_circuits_after: Some(7),
        }),
    };
    let dto = RecordSoranetPrivacyEventDto {
        event,
        source: None,
    };

    let response = handle_post_soranet_privacy_event(telemetry.clone(), NoritoJson(dto))
        .await
        .unwrap()
        .into_response();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let body = norito_json_response_value(response).await;
    assert_eq!(body.get("status").and_then(Value::as_str), Some("accepted"));
    let bucket_secs = telemetry
        .telemetry()
        .unwrap()
        .soranet_privacy()
        .config()
        .bucket_secs;
    let bucket_start = (timestamp / bucket_secs) * bucket_secs;
    assert_eq!(
        body.get("bucket_start_unix").and_then(Value::as_u64),
        Some(bucket_start)
    );
}

fn make_share(
    collector_id: u16,
    bucket_start: u64,
    bucket_duration_secs: u32,
    mode: SoranetPrivacyModeV1,
    handshake_share: i64,
) -> SoranetPrivacyPrioShareV1 {
    let mut share =
        SoranetPrivacyPrioShareV1::new(collector_id, bucket_start, bucket_duration_secs);
    share.mode = mode;
    share.handshake_accept_share = handshake_share;
    share.active_circuits_sum_share = 6 * handshake_share;
    share.active_circuits_sample_share = handshake_share;
    share.active_circuits_max_observed = Some(9);
    share.verified_bytes_share = 512;
    share
}

#[tokio::test]
async fn privacy_share_endpoint_combines_shares() {
    let telemetry = MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
    let privacy_config = telemetry.telemetry().unwrap().soranet_privacy().config();
    let bucket_secs =
        u32::try_from(privacy_config.bucket_secs).expect("bucket duration fits within u32");
    let bucket_secs_u64 = u64::from(bucket_secs);
    let now_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after UNIX epoch")
        .as_secs();
    let bucket_start = (now_unix / bucket_secs_u64) * bucket_secs_u64;
    let handshake_share = i64::try_from((privacy_config.min_contributors / 2).saturating_add(1))
        .expect("minimum contributor threshold fits within i64");
    let expected_accepted_total =
        u64::try_from(handshake_share).expect("handshake share is non-negative") * 2;
    let mode = SoranetPrivacyModeV1::Middle;

    let share_a = make_share(1, bucket_start, bucket_secs, mode, handshake_share);
    let share_b = make_share(2, bucket_start, bucket_secs, mode, handshake_share);

    for (share, fwd) in [
        (share_a, Some("collector-a".to_string())),
        (share_b, Some("collector-b".to_string())),
    ] {
        let dto = RecordSoranetPrivacyShareDto {
            share,
            forwarded_by: fwd,
        };
        let response = handle_post_soranet_privacy_share(telemetry.clone(), NoritoJson(dto))
            .await
            .unwrap()
            .into_response();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    let metrics = telemetry.metrics().await;
    let bucket_label = bucket_start.to_string();

    let suppressed = metrics
        .soranet_privacy_bucket_suppressed
        .get_metric_with_label_values(&["middle", bucket_label.as_str()])
        .unwrap()
        .get();
    assert!(
        (suppressed - 0.0).abs() < f64::EPSILON,
        "bucket should not be suppressed (value {suppressed})"
    );

    let accepted_total = metrics
        .soranet_privacy_circuit_events_total
        .get_metric_with_label_values(&["middle", bucket_label.as_str(), "accepted"])
        .unwrap()
        .get();
    assert_eq!(accepted_total, expected_accepted_total);

    let verified_bytes = metrics
        .soranet_privacy_verified_bytes_total
        .get_metric_with_label_values(&["middle", bucket_label.as_str()])
        .unwrap()
        .get();
    assert_eq!(verified_bytes, 1024);

    let avg_circuits = metrics
        .soranet_privacy_active_circuits_avg
        .get_metric_with_label_values(&["middle", bucket_label.as_str()])
        .unwrap()
        .get();
    assert!(avg_circuits > 0.0);
}
