#![allow(clippy::restriction)]
use super::*;
use norito::json::{self, Value};
use std::time::Duration;
fn assert_float_metric_eq(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() < f64::EPSILON,
        "{context}: expected {expected}, got {actual}"
    );
}
#[test]
fn quantity_micro_projection_preserves_fractional_xor() {
    let value = "0.0015"
        .parse::<iroha_data_model::prelude::Quantity>()
        .expect("canonical quantity");
    assert_float_metric_eq(
        quantity_to_micro_f64(&value),
        1_500.0,
        "quantity micro projection",
    );
}
#[test]
fn metrics_lifecycle() {
    let metrics = Metrics::default();
    println!(
        "{:?}",
        metrics
            .try_to_string()
            .expect("Should not fail for default")
    );
    println!("{:?}", Status::from(&metrics));
    println!("{:?}", Status::default());
}
#[test]
fn sorafs_pin_resource_usage_exports_only_consensus_summary_values() {
    let metrics = Metrics::default();
    metrics.record_sorafs_pin_resource_usage(17, 4_096);
    let exported = metrics.try_to_string().expect("metrics should serialize");
    assert!(exported.contains("torii_sorafs_pin_retained_manifests 17"));
    assert!(exported.contains("torii_sorafs_pin_live_content_bytes 4096"));
}
#[test]
fn nexus_status_exports_optional_rule_dataspace() {
    let policy = iroha_config::parameters::actual::LaneRoutingPolicy {
        default_lane: iroha_data_model::LaneId::new(2),
        default_dataspace: iroha_data_model::DataSpaceId::new(10),
        rules: vec![
            iroha_config::parameters::actual::LaneRoutingRule {
                lane: iroha_data_model::LaneId::new(3),
                dataspace: Some(iroha_data_model::DataSpaceId::new(11)),
                matcher: iroha_config::parameters::actual::LaneRoutingMatcher {
                    account: Some("alice".to_owned()),
                    instruction: Some("Register".to_owned()),
                    description: Some("explicit dataspace".to_owned()),
                },
            },
            iroha_config::parameters::actual::LaneRoutingRule {
                lane: iroha_data_model::LaneId::new(4),
                dataspace: None,
                matcher: iroha_config::parameters::actual::LaneRoutingMatcher::default(),
            },
        ],
    };
    let status = NexusStatus::from_routing_policy(&policy);
    assert_eq!(status.routing_policy.default_lane, 2);
    assert_eq!(status.routing_policy.default_dataspace, 10);
    assert_eq!(status.routing_policy.rules[0].lane, 3);
    assert_eq!(status.routing_policy.rules[0].dataspace_id, Some(11));
    assert_eq!(
        status.routing_policy.rules[0]
            .matcher
            .description
            .as_deref(),
        Some("explicit dataspace")
    );
    assert_eq!(status.routing_policy.rules[1].lane, 4);
    assert_eq!(status.routing_policy.rules[1].dataspace_id, None);
}
#[test]
fn pacemaker_metrics_are_exported() {
    let metrics = Metrics::default();
    let dump = metrics.try_to_string().expect("metrics text");
    assert!(
        dump.contains("sumeragi_pacemaker_view_timeout_target_ms"),
        "metrics export missing pacemaker view timeout target"
    );
}
#[test]
fn p2p_queue_depth_metric_accepts_updates() {
    let metrics = Metrics::default();
    metrics.p2p_queue_depth.with_label_values(&["High"]).set(12);
    metrics.p2p_queue_depth.with_label_values(&["Low"]).set(7);
    assert_eq!(
        metrics.p2p_queue_depth.with_label_values(&["High"]).get(),
        12
    );
    assert_eq!(metrics.p2p_queue_depth.with_label_values(&["Low"]).get(), 7);
}
#[test]
fn soranet_reward_metrics_record_without_exporter() {
    let metrics = Metrics::default();
    metrics.record_soranet_reward("relay_hex", 0, "rewarded");
    metrics.record_soranet_reward_skip("relay_hex", "insufficient_bond");
    metrics.record_soranet_adjustment("relay_hex", 0, "credit");
    metrics.inc_soranet_dispute("filed");
}
#[test]
fn records_norito_decode_failures() {
    let metrics = Metrics::default();
    metrics.inc_torii_norito_decode_failure("transaction", "checksum_mismatch");
    let counter = metrics
        .torii_norito_decode_failures_total
        .with_label_values(&["transaction", "checksum_mismatch"])
        .get();
    assert_eq!(counter, 1, "decode failure counter increments");
}
#[test]
fn records_norito_rpc_gate_outcomes() {
    let metrics = Metrics::default();
    metrics.inc_torii_norito_rpc_gate("canary", "allowed");
    metrics.inc_torii_norito_rpc_gate("canary", "canary_denied");
    assert_eq!(
        metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&["canary", "allowed"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_norito_rpc_gate_total
            .with_label_values(&["canary", "canary_denied"])
            .get(),
        1
    );
}
#[test]
fn records_api_token_hits_without_exporting_token_material() {
    let metrics = Metrics::default();
    let token = "super-secret-token";
    metrics.inc_torii_api_token_hit("v1/sccp/capabilities", "present");
    assert_eq!(
        metrics
            .torii_api_token_hits_total
            .with_label_values(&["v1/sccp/capabilities", "present"])
            .get(),
        1,
        "API-token hit counter increments"
    );
    let exported = metrics.try_to_string().expect("metrics should serialize");
    assert!(
        exported.contains(
            "torii_api_token_hits_total{endpoint=\"v1/sccp/capabilities\",token_state=\"present\"} 1"
        ),
        "bounded API-token hit labels missing from metrics output: {exported}"
    );
    assert!(
        !exported.contains(token),
        "metrics output must not expose raw API token material"
    );
}
#[test]
fn records_attachment_sanitizer_metrics() {
    let metrics = Metrics::default();
    metrics.inc_torii_attachment_reject("type");
    let counter = metrics
        .torii_attachment_reject_total
        .with_label_values(&["type"])
        .get();
    assert_eq!(counter, 1);
    metrics.observe_torii_attachment_sanitize_ms(12);
    let samples = metrics
        .torii_attachment_sanitize_ms
        .with_label_values::<&str>(&[])
        .get_sample_count();
    assert_eq!(samples, 1);
}
#[test]
fn records_da_chunking_latency() {
    let metrics = Metrics::default();
    metrics.observe_da_chunking_seconds(0.125);
    let samples = metrics.torii_da_chunking_seconds.get_sample_count();
    assert_eq!(samples, 1);
}
#[test]
fn records_operator_auth_metrics() {
    let metrics = Metrics::default();
    metrics.inc_torii_operator_auth("gate", "allowed", "session");
    metrics.inc_torii_operator_auth_lockout("gate", "invalid_session");
    assert_eq!(
        metrics
            .torii_operator_auth_total
            .with_label_values(&["gate", "allowed", "session"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_operator_auth_lockout_total
            .with_label_values(&["gate", "invalid_session"])
            .get(),
        1
    );
}
#[test]
fn records_sns_registrar_status_metrics() {
    let metrics = Metrics::default();
    metrics.inc_sns_registrar_status("ok", "sora");
    metrics.inc_sns_registrar_status("error", "sora");
    assert_eq!(
        metrics
            .sns_registrar_status_total
            .with_label_values(&["ok", "sora"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .sns_registrar_status_total
            .with_label_values(&["error", "sora"])
            .get(),
        1
    );
}
#[test]
fn catalog_retires_global_rbc_metrics_and_keeps_signed_da_metrics() {
    let catalog = include_str!("catalog_v2.tsv");
    assert!(
        !catalog.contains("sumeragi_rbc_"),
        "retired global-RBC metrics must not remain in the public catalog"
    );
    for current in [
        "sumeragi_da_manifest_guard_total",
        "sumeragi_da_manifest_cache_total",
        "sumeragi_da_spool_cache_total",
        "sumeragi_da_pin_intent_spool_total",
        "sumeragi_da_votes_ingested_total",
    ] {
        assert!(
            catalog.lines().any(|line| line.starts_with(current)),
            "signed DA metric `{current}` must remain catalogued"
        );
    }
}
#[test]
fn records_alias_cache_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_alias_cache("success", "fresh", 42.0);
    let refresh_counter = metrics
        .torii_sorafs_alias_cache_refresh_total
        .with_label_values(&["success", "fresh"])
        .get();
    assert_eq!(refresh_counter, 1, "alias cache counter increments");
    let age_samples = metrics
        .torii_sorafs_alias_cache_age_seconds
        .get_sample_count();
    assert_eq!(age_samples, 1, "alias cache age histogram records sample");
}
#[test]
fn records_privacy_suppression_reason_metrics() {
    let metrics = Metrics::default();
    let bucket = SoranetPrivacyBucketMetricsV1::suppressed_with_reason(
        SoranetPrivacyModeV1::Entry,
        1,
        60,
        SoranetPrivacySuppressionReasonV1::CollectorSuppressed,
    );
    metrics.record_soranet_privacy_bucket(&bucket);
    let suppression_gauge = metrics
        .soranet_privacy_bucket_suppressed
        .with_label_values(&["entry"])
        .get();
    assert!(
        (suppression_gauge - 1.0).abs() < f64::EPSILON,
        "suppression gauge toggles"
    );
    let reason_counter = metrics
        .soranet_privacy_suppression_total
        .with_label_values(&["entry", "collector_suppressed"])
        .get();
    assert_eq!(
        reason_counter, 1,
        "suppression counter increments for the reason"
    );
    assert_eq!(
        metrics
            .soranet_privacy_latest_bucket_start_unixtime
            .with_label_values(&["entry"])
            .get(),
        1,
        "latest privacy bucket timestamp"
    );
}
#[test]
fn privacy_bucket_metrics_have_fixed_cardinality_and_count_once() {
    use iroha_data_model::soranet::privacy_metrics::{
        SoranetGarAbuseCountV1, SoranetLatencyPercentileV1,
    };

    let metrics = Metrics::default();
    let mut first = SoranetPrivacyBucketMetricsV1::suppressed_with_reason(
        SoranetPrivacyModeV1::Entry,
        60,
        60,
        SoranetPrivacySuppressionReasonV1::InsufficientContributors,
    );
    first.suppressed = false;
    first.suppression_reason = None;
    first.handshake_accept_total = 2;
    first.throttle_emergency_total = 3;
    first.active_circuits_mean = Some(4);
    first.active_circuits_max = Some(7);
    first.rtt_percentiles_ms = vec![
        SoranetLatencyPercentileV1::new("p50".to_owned(), 25),
        SoranetLatencyPercentileV1::new("attacker-controlled".to_owned(), 999),
    ];
    first.gar_abuse_counts = vec![
        SoranetGarAbuseCountV1::new([1; 8], 2),
        SoranetGarAbuseCountV1::new([2; 8], 3),
    ];
    metrics.record_soranet_privacy_bucket(&first);

    let mut second = first;
    second.bucket_start_unix = 120;
    second.handshake_accept_total = 5;
    second.throttle_emergency_total = 4;
    second.active_circuits_mean = Some(8);
    second.active_circuits_max = Some(11);
    second.rtt_percentiles_ms = vec![
        SoranetLatencyPercentileV1::new("p50".to_owned(), 50),
        SoranetLatencyPercentileV1::new("rotated-attacker-label".to_owned(), 1_000),
    ];
    second.gar_abuse_counts = vec![SoranetGarAbuseCountV1::new([9; 8], 7)];
    metrics.record_soranet_privacy_bucket(&second);

    assert_eq!(
        metrics
            .soranet_privacy_circuit_events_total
            .with_label_values(&["entry", "accepted"])
            .get(),
        7,
        "fixed outcome counter accumulates across buckets"
    );
    assert_eq!(
        metrics
            .soranet_privacy_throttles_total
            .with_label_values(&["entry", "emergency"])
            .get(),
        7,
        "each emergency throttle is counted exactly once"
    );
    assert_eq!(
        metrics
            .soranet_privacy_gar_reports_total
            .with_label_values(&["entry"])
            .get(),
        12,
        "GAR categories aggregate into one fixed-cardinality counter"
    );
    assert_eq!(
        metrics
            .soranet_privacy_latest_bucket_start_unixtime
            .with_label_values(&["entry"])
            .get(),
        120,
        "latest bucket timestamp replaces the previous gauge value"
    );
    assert_float_metric_eq(
        metrics
            .soranet_privacy_active_circuits_max
            .with_label_values(&["entry"])
            .get(),
        11.0,
        "latest active-circuit gauge",
    );
    assert_float_metric_eq(
        metrics
            .soranet_privacy_rtt_millis
            .with_label_values(&["entry", "p50"])
            .get(),
        50.0,
        "latest fixed percentile gauge",
    );

    let rendered = metrics.try_to_string().expect("render privacy metrics");
    assert!(!rendered.contains("bucket_start=\""));
    assert!(!rendered.contains("category_hash=\""));
    assert!(!rendered.contains("attacker-controlled"));
    assert!(!rendered.contains("rotated-attacker-label"));
    assert_eq!(
        metrics.soranet_privacy_rtt_millis.collect()[0]
            .get_metric()
            .len(),
        3,
        "RTT labels are limited to the fixed p50/p90/p99 set"
    );
}
fn sample_privacy_snapshot() -> PrivacyDrainSnapshot {
    let mut snapshot = PrivacyDrainSnapshot {
        drained_buckets: 5,
        evicted_completed: 2,
        ..PrivacyDrainSnapshot::default()
    };
    snapshot.open_buckets.insert(SoranetPrivacyModeV1::Entry, 3);
    snapshot
        .collector_backlog
        .insert(SoranetPrivacyModeV1::Entry, 7);
    snapshot
        .suppressed_counts
        .insert(SoranetPrivacySuppressionReasonV1::CollectorSuppressed, 4);
    snapshot
        .suppressed_by_mode
        .entry(SoranetPrivacyModeV1::Entry)
        .or_default()
        .insert(SoranetPrivacySuppressionReasonV1::CollectorSuppressed, 2);
    snapshot
}
#[test]
fn records_privacy_queue_snapshot_metrics() {
    let metrics = Metrics::default();
    let snapshot = sample_privacy_snapshot();
    metrics.record_soranet_privacy_queue_snapshot(&snapshot);
    let open_entry = metrics
        .soranet_privacy_open_buckets
        .with_label_values(&["entry"])
        .get();
    assert!(
        (open_entry - 3.0).abs() < f64::EPSILON,
        "entry bucket count recorded"
    );
    assert!(
        metrics
            .soranet_privacy_open_buckets
            .with_label_values(&["middle"])
            .get()
            .abs()
            < f64::EPSILON,
        "missing modes should be reset to zero"
    );
    assert_eq!(
        metrics.soranet_privacy_evicted_buckets_total.get(),
        2,
        "evicted counter increments"
    );
    assert_eq!(
        metrics.soranet_privacy_snapshot_drained.get(),
        5,
        "drained gauge reflects snapshot"
    );
    let collector_gauge = metrics
        .soranet_privacy_snapshot_suppressed
        .with_label_values(&["collector_suppressed"])
        .get();
    assert!(
        (collector_gauge - 4.0).abs() < f64::EPSILON,
        "snapshot gauge reflects supplied suppression counts"
    );
    let pending_collectors = metrics
        .soranet_privacy_pending_collectors
        .with_label_values(&["entry"])
        .get();
    assert!(
        (pending_collectors - 7.0).abs() < f64::EPSILON,
        "pending collector gauge reflects backlog"
    );
    assert!(
        metrics
            .soranet_privacy_pending_collectors
            .with_label_values(&["middle"])
            .get()
            .abs()
            < f64::EPSILON,
        "unspecified collector backlog resets to zero"
    );
    assert!(
        (metrics.soranet_privacy_snapshot_suppression_ratio.get() - 0.8).abs() < f64::EPSILON,
        "suppression ratio reflects suppressed/share of drained buckets"
    );
    assert!(
        metrics
            .soranet_privacy_snapshot_suppressed
            .with_label_values(&["forced_flush_window_elapsed"])
            .get()
            .abs()
            < f64::EPSILON,
        "unspecified reasons reset to zero"
    );
    let collector_by_mode = metrics
        .soranet_privacy_snapshot_suppressed_by_mode
        .with_label_values(&["entry", "collector_suppressed"])
        .get();
    assert!(
        (collector_by_mode - 2.0).abs() < f64::EPSILON,
        "per-mode suppression gauge tracks counts"
    );
    assert!(
        metrics
            .soranet_privacy_snapshot_suppressed_by_mode
            .with_label_values(&["middle", "insufficient_contributors"])
            .get()
            .abs()
            < f64::EPSILON,
        "unspecified per-mode suppression resets to zero"
    );
}
#[test]
fn records_tls_metrics() {
    let metrics = Metrics::default();
    metrics.set_sorafs_tls_state(true, Some(Duration::from_secs(90)));
    metrics.record_sorafs_tls_renewal("success");
    let expiry = metrics.torii_sorafs_tls_cert_expiry_seconds.get();
    assert!(
        (expiry - 90.0).abs() < f64::EPSILON,
        "TLS expiry gauge records seconds remaining"
    );
    let ech_enabled = metrics.torii_sorafs_tls_ech_enabled.get();
    assert_eq!(ech_enabled, 1, "ECH gauge reflects enabled state");
    let renewal_total = metrics
        .torii_sorafs_tls_renewal_total
        .with_label_values(&["success"])
        .get();
    assert_eq!(renewal_total, 1, "TLS renewal counter increments");
    metrics.set_sorafs_tls_state(false, None);
    assert_eq!(
        metrics.torii_sorafs_tls_ech_enabled.get(),
        0,
        "ECH gauge resets when disabled"
    );
    assert!(
        metrics.torii_sorafs_tls_cert_expiry_seconds.get().abs() < f64::EPSILON,
        "TLS expiry gauge resets when expiry is unknown"
    );
}
#[test]
fn records_proof_stream_metrics() {
    let metrics = Metrics::default();
    metrics.inc_sorafs_proof_stream_inflight("por");
    metrics.record_sorafs_proof_stream_event("por", "success", None, None, None, Some(42.0));
    metrics.dec_sorafs_proof_stream_inflight("por");
    let inflight = metrics
        .torii_sorafs_proof_stream_inflight
        .with_label_values(&["por"])
        .get();
    assert_eq!(inflight, 0, "inflight gauge returns to zero after dec");
    let total = metrics
        .torii_sorafs_proof_stream_events_total
        .with_label_values(&["por", "success", "ok"])
        .get();
    assert_eq!(total, 1, "proof stream counter increments");
    let samples = metrics
        .torii_sorafs_proof_stream_latency_ms
        .with_label_values(&["por"])
        .get_sample_count();
    assert_eq!(samples, 1, "latency histogram records observation");
}
#[test]
fn records_torii_proof_metrics() {
    let metrics = Metrics::default();
    metrics.record_torii_proof_request("v1/zk/proof", "ok", 128, Duration::from_millis(5));
    metrics.inc_torii_proof_cache_hit("v1/zk/proof");
    metrics.inc_torii_proof_throttled("v1/zk/proof");
    assert_eq!(
        metrics
            .torii_proof_requests_total
            .with_label_values(&["v1/zk/proof", "ok"])
            .get(),
        1,
        "proof request counter increments"
    );
    assert_eq!(
        metrics
            .torii_proof_response_bytes_total
            .with_label_values(&["v1/zk/proof", "ok"])
            .get(),
        128,
        "proof response bytes counter increments"
    );
    assert_eq!(
        metrics
            .torii_proof_request_duration_seconds
            .with_label_values(&["v1/zk/proof", "ok"])
            .get_sample_count(),
        1,
        "proof request latency histogram records observation"
    );
    assert_eq!(
        metrics
            .torii_proof_cache_hits_total
            .with_label_values(&["v1/zk/proof"])
            .get(),
        1,
        "proof cache hits counter increments"
    );
    assert_eq!(
        metrics
            .torii_proof_throttled_total
            .with_label_values(&["v1/zk/proof"])
            .get(),
        1,
        "proof throttle counter increments"
    );
}
#[test]
fn records_torii_explorer_metrics() {
    let metrics = Metrics::default();
    metrics.record_torii_explorer_request(
        "/v1/explorer/transactions",
        "ok",
        Duration::from_millis(4),
    );
    metrics.record_torii_explorer_request(
        "/v1/explorer/transactions",
        "error",
        Duration::from_millis(7),
    );
    assert_eq!(
        metrics
            .torii_explorer_requests_total
            .with_label_values(&["/v1/explorer/transactions", "ok"])
            .get(),
        1,
        "explorer request counter increments for ok outcomes"
    );
    assert_eq!(
        metrics
            .torii_explorer_requests_total
            .with_label_values(&["/v1/explorer/transactions", "error"])
            .get(),
        1,
        "explorer request counter increments for error outcomes"
    );
    assert_eq!(
        metrics
            .torii_explorer_request_duration_seconds
            .with_label_values(&["/v1/explorer/transactions", "ok"])
            .get_sample_count(),
        1,
        "explorer request latency histogram records ok outcomes"
    );
    assert_eq!(
        metrics
            .torii_explorer_request_duration_seconds
            .with_label_values(&["/v1/explorer/transactions", "error"])
            .get_sample_count(),
        1,
        "explorer request latency histogram records error outcomes"
    );
}
#[test]
fn records_sorafs_egress_reconciliation_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_egress_reconciliation("provider-a", 1_000, Some(1_100), Some(900));
    assert_float_metric_eq(
        metrics
            .torii_sorafs_egress_bytes
            .with_label_values(&["provider-a", "billing"])
            .get(),
        1_000.0,
        "billing egress bytes",
    );
    assert_float_metric_eq(
        metrics
            .torii_sorafs_egress_bytes
            .with_label_values(&["provider-a", "gateway"])
            .get(),
        1_100.0,
        "gateway egress bytes",
    );
    assert_float_metric_eq(
        metrics
            .torii_sorafs_egress_bytes
            .with_label_values(&["provider-a", "orchestrator"])
            .get(),
        900.0,
        "orchestrator egress bytes",
    );
    assert!(
        (metrics
            .torii_sorafs_egress_drift_ratio
            .with_label_values(&["provider-a", "gateway"])
            .get()
            - 0.1)
            .abs()
            < f64::EPSILON
    );
    assert!(
        (metrics
            .torii_sorafs_egress_drift_ratio
            .with_label_values(&["provider-a", "orchestrator"])
            .get()
            - 0.1)
            .abs()
            < f64::EPSILON
    );
    assert_float_metric_eq(
        metrics
            .torii_sorafs_egress_drift_ratio
            .with_label_values(&["provider-a", "billing"])
            .get(),
        0.0,
        "billing egress drift ratio",
    );
    let exported = metrics.try_to_string().expect("metrics should serialize");
    assert!(
        exported
            .contains("torii_sorafs_egress_bytes{provider=\"provider-a\",source=\"gateway\"} 1100"),
        "egress bytes should be exported: {exported}"
    );
    assert!(
        exported.contains(
            "torii_sorafs_egress_drift_ratio{provider=\"provider-a\",source=\"gateway\"} 0.1"
        ),
        "egress drift should be exported: {exported}"
    );
}
#[test]
fn records_sorafs_governance_dag_publication_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_governance_dag_publish(
        "deal_settlement",
        "success",
        "filesystem",
        512,
        1_800_000_000,
    );
    metrics.record_sorafs_governance_dag_publish(
        "repair_audit",
        "failure",
        "filesystem",
        256,
        1_800_000_010,
    );
    metrics.set_sorafs_governance_dag_backlog("filesystem", 3);
    metrics.set_sorafs_governance_dag_head_age_seconds("filesystem", 45);
    assert_eq!(
        metrics
            .sorafs_governance_dag_publish_total
            .with_label_values(&["deal_settlement", "success", "filesystem"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_governance_dag_publish_total
            .with_label_values(&["repair_audit", "failure", "filesystem"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_governance_dag_published_bytes_total
            .with_label_values(&["deal_settlement", "filesystem"])
            .get(),
        512
    );
    assert_eq!(
        metrics
            .sorafs_governance_dag_last_publish_timestamp_seconds
            .with_label_values(&["deal_settlement", "filesystem"])
            .get(),
        1_800_000_000
    );
    assert_eq!(
        metrics
            .sorafs_governance_dag_backlog
            .with_label_values(&["filesystem"])
            .get(),
        3
    );
    assert_eq!(
        metrics
            .sorafs_governance_dag_head_age_seconds
            .with_label_values(&["filesystem"])
            .get(),
        45
    );
    let exported = metrics.try_to_string().expect("metrics should serialize");
    for metric_name in [
        "sorafs_governance_dag_publish_total",
        "sorafs_governance_dag_published_bytes_total",
        "sorafs_governance_dag_last_publish_timestamp_seconds",
        "sorafs_governance_dag_backlog",
        "sorafs_governance_dag_head_age_seconds",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing governance DAG metric {metric_name} from export:\n{exported}"
        );
    }
}
#[test]
fn records_sorafs_reputation_snapshot_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_reputation_snapshot(
        100,
        160,
        &[
            ("provider-a", 9_000, false),
            ("provider-b", 1_000, true),
            ("provider-c", 8_000, false),
        ],
    );
    assert_eq!(metrics.sorafs_reputation_ingest_lag_seconds.get(), 60);
    assert_eq!(metrics.sorafs_reputation_snapshot_age_seconds.get(), 60);
    assert_eq!(
        metrics.sorafs_reputation_snapshot_generated_at_unix.get(),
        100
    );
    assert_eq!(metrics.sorafs_reputation_provider_count.get(), 3);
    assert_eq!(metrics.sorafs_reputation_low_score_providers.get(), 1);
    assert_eq!(
        metrics
            .sorafs_reputation_threshold_crossings_total
            .with_label_values(&["low_score"])
            .get(),
        0,
        "initial snapshot seeds threshold state without false crossings"
    );
    metrics.record_sorafs_reputation_snapshot(
        200,
        210,
        &[("provider-a", 1_200, true), ("provider-b", 2_000, false)],
    );
    assert_eq!(
        metrics
            .sorafs_reputation_threshold_crossings_total
            .with_label_values(&["low_score"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_reputation_threshold_crossings_total
            .with_label_values(&["recovered"])
            .get(),
        1
    );
    assert_float_metric_eq(
        metrics
            .sorafs_reputation_score
            .with_label_values(&["provider-a"])
            .get(),
        1_200.0,
        "provider reputation score",
    );
    assert_eq!(
        metrics
            .sorafs_reputation_score_tracked_providers
            .read()
            .expect("tracked reputation score labels")
            .len(),
        2
    );
    let many_providers: Vec<(String, u16, bool)> = (0_u16..105)
        .map(|index| (format!("provider-{index:03}"), 10_000_u16 - index, false))
        .collect();
    let many_provider_refs: Vec<(&str, u16, bool)> = many_providers
        .iter()
        .map(|(provider_id, score_bps, low_score)| (provider_id.as_str(), *score_bps, *low_score))
        .collect();
    metrics.record_sorafs_reputation_snapshot(300, 295, &many_provider_refs);
    assert_eq!(
        metrics
            .sorafs_reputation_score_tracked_providers
            .read()
            .expect("tracked reputation score labels")
            .len(),
        SORAFS_REPUTATION_SCORE_LABEL_LIMIT
    );
    assert_eq!(
        metrics.sorafs_reputation_snapshot_age_seconds.get(),
        0,
        "future-dated snapshots saturate observed age at zero"
    );
    let exported = metrics.try_to_string().expect("metrics should serialize");
    assert!(
        exported.contains("sorafs_reputation_provider_count 105"),
        "reputation provider count should be exported: {exported}"
    );
    assert!(
        exported.contains("sorafs_reputation_score{provider_id=\"provider-000\"} 10000"),
        "bounded reputation score should be exported: {exported}"
    );
}
#[test]
fn records_committed_sorafs_reputation_runtime_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_reputation_runtime_status(SorafsReputationRuntimeMetricSnapshot {
        runtime: SorafsRuntimeHealthMetricSnapshot {
            live: true,
            ready: false,
            external_dependencies_ready: true,
        },
        publication: SorafsReputationPublicationMetricSnapshot {
            journal_transaction_submitter_ready: false,
            material_acknowledged: true,
        },
        latest_finalized_height: 42,
        consecutive_failures: 3,
        provider_count: 17,
    });
    metrics.inc_sorafs_reputation_runtime_tick("success");
    metrics.inc_sorafs_reputation_runtime_tick("failure");
    metrics.inc_sorafs_reputation_runtime_tick("unbounded-input");
    assert_eq!(metrics.sorafs_reputation_runtime_live.get(), 1);
    assert_eq!(metrics.sorafs_reputation_runtime_ready.get(), 0);
    assert_eq!(
        metrics.sorafs_reputation_runtime_dependencies_ready.get(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_reputation_journal_transaction_submitter_ready
            .get(),
        0
    );
    assert_eq!(metrics.sorafs_reputation_runtime_finalized_height.get(), 42);
    assert_eq!(
        metrics.sorafs_reputation_runtime_consecutive_failures.get(),
        3
    );
    assert_eq!(
        metrics
            .sorafs_reputation_runtime_material_acknowledged
            .get(),
        1
    );
    assert_eq!(metrics.sorafs_reputation_runtime_provider_count.get(), 17);
    for result in ["success", "failure", "unknown"] {
        assert_eq!(
            metrics
                .sorafs_reputation_runtime_ticks_total
                .with_label_values(&[result])
                .get(),
            1
        );
    }
}
#[test]
fn records_committed_sorafs_hedging_billing_runtime_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_hedging_billing_runtime_status(
        SorafsHedgingBillingRuntimeMetricSnapshot {
            runtime: SorafsRuntimeHealthMetricSnapshot {
                live: true,
                ready: false,
                external_dependencies_ready: true,
            },
            projection: SorafsHedgingBillingProjectionMetricSnapshot {
                automatic_execution_enabled: false,
                last_tick_fresh: true,
                finalized_projection_ready: false,
            },
            finalized_height: 42,
            finalized_head_height: 45,
            finalized_lag_blocks: 3,
            next_event_sequence: 88,
            ready_for_signing: 1,
            ready_for_publication: 2,
            publication_ambiguous: 3,
            published: 4,
            acknowledged: 5,
            dead_letter: 6,
            hedge_intents: 7,
        },
    );
    metrics.inc_sorafs_hedging_billing_runtime_tick("success");
    metrics.inc_sorafs_hedging_billing_runtime_tick("failure");
    metrics.inc_sorafs_hedging_billing_runtime_tick("unbounded-input");
    assert_eq!(metrics.sorafs_hedging_billing_runtime_live.get(), 1);
    assert_eq!(metrics.sorafs_hedging_billing_runtime_ready.get(), 0);
    assert_eq!(
        metrics
            .sorafs_hedging_billing_runtime_dependencies_ready
            .get(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_hedging_billing_automatic_execution_enabled
            .get(),
        0
    );
    assert_eq!(metrics.sorafs_hedging_billing_last_tick_fresh.get(), 1);
    assert_eq!(
        metrics
            .sorafs_hedging_billing_finalized_projection_ready
            .get(),
        0
    );
    assert_eq!(metrics.sorafs_hedging_billing_finalized_height.get(), 42);
    assert_eq!(
        metrics.sorafs_hedging_billing_finalized_head_height.get(),
        45
    );
    assert_eq!(metrics.sorafs_hedging_billing_finalized_lag_blocks.get(), 3);
    assert_eq!(metrics.sorafs_hedging_billing_next_event_sequence.get(), 88);
    assert_eq!(metrics.sorafs_hedging_billing_ready_for_signing.get(), 1);
    assert_eq!(
        metrics.sorafs_hedging_billing_ready_for_publication.get(),
        2
    );
    assert_eq!(
        metrics.sorafs_hedging_billing_publication_ambiguous.get(),
        3
    );
    assert_eq!(metrics.sorafs_hedging_billing_published.get(), 4);
    assert_eq!(metrics.sorafs_hedging_billing_acknowledged.get(), 5);
    assert_eq!(metrics.sorafs_hedging_billing_dead_letter.get(), 6);
    assert_eq!(metrics.sorafs_hedging_billing_hedge_intents.get(), 7);
    for result in ["success", "failure", "unknown"] {
        assert_eq!(
            metrics
                .sorafs_hedging_billing_runtime_ticks_total
                .with_label_values(&[result])
                .get(),
            1
        );
    }
}
#[test]
fn records_sorafs_por_scheduler_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_por_ingestion_backlog("provider-a", "manifest-a", 3);
    metrics.record_sorafs_por_ingestion_failures("provider-a", "manifest-a", 2);
    metrics.record_sorafs_por_scheduler_challenge(false, 0);
    metrics.record_sorafs_por_scheduler_challenge(true, 4);
    metrics.record_sorafs_por_scheduler_failure();
    assert_eq!(
        metrics
            .torii_sorafs_por_challenges_total
            .with_label_values(&["scheduled"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_por_challenges_total
            .with_label_values(&["forced"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_por_challenges_total
            .with_label_values(&["failed"])
            .get(),
        1
    );
    assert_eq!(metrics.torii_sorafs_por_forced_challenges_total.get(), 1);
    assert_eq!(metrics.torii_sorafs_por_sampling_duplicates_total.get(), 4);
    let exported = metrics.try_to_string().expect("metrics should serialize");
    assert!(
        exported.contains("torii_sorafs_por_challenges_total{result=\"forced\"} 1"),
        "forced challenge counter should be exported: {exported}"
    );
    assert!(
        exported.contains("torii_sorafs_por_forced_challenges_total 1"),
        "forced challenge total should be exported: {exported}"
    );
    assert!(
        exported.contains("torii_sorafs_por_sampling_duplicates_total 4"),
        "duplicate sample counter should be exported: {exported}"
    );
    assert!(
        exported.contains(
            "torii_sorafs_por_ingest_backlog{manifest=\"manifest-a\",provider=\"provider-a\"} 3"
        ),
        "PoR ingestion backlog gauge should be exported: {exported}"
    );
}
#[test]
fn records_gateway_fixture_version() {
    let metrics = Metrics::default();
    metrics.set_sorafs_gateway_fixture_version("1.0.0");
    let initial = metrics
        .torii_sorafs_gateway_fixture_version
        .with_label_values(&["1.0.0"])
        .get();
    assert_eq!(initial, 1, "fixture version gauge set for current version");
    metrics.set_sorafs_gateway_fixture_version("1.0.1");
    let updated = metrics
        .torii_sorafs_gateway_fixture_version
        .with_label_values(&["1.0.1"])
        .get();
    assert_eq!(updated, 1, "fixture version gauge switches to new version");
    let previous = metrics
        .torii_sorafs_gateway_fixture_version
        .with_label_values(&["1.0.0"])
        .get();
    assert_eq!(previous, 0, "previous version gauge resets");
}
#[test]
fn records_lane_settlement_snapshot_metrics() {
    let metrics = Metrics::default();
    metrics.record_lane_settlement_snapshot(LaneSettlementSnapshot {
        lane_id: "lane-1",
        dataspace_id: "ds-42",
        xor_due_micro: 1_000,
        variance_micro: 250,
        haircut_bps: 25,
        swapline: Some(LaneSwaplineSnapshot {
            profile: "tier1-deep",
            utilisation_micro: 1_000,
        }),
        buffer: Some(LaneSettlementBuffer {
            remaining: 500.0,
            capacity: 1_500.0,
            status: 1.0,
        }),
    });
    let buffer = metrics
        .settlement_buffer_xor
        .with_label_values(&["lane-1", "ds-42"])
        .get();
    assert!(
        (buffer - 500.0).abs() < f64::EPSILON,
        "buffer gauge captures remaining headroom"
    );
    let capacity = metrics
        .settlement_buffer_capacity_xor
        .with_label_values(&["lane-1", "ds-42"])
        .get();
    assert!(
        (capacity - 1_500.0).abs() < f64::EPSILON,
        "buffer capacity gauge records configured capacity"
    );
    let status = metrics
        .settlement_buffer_status
        .with_label_values(&["lane-1", "ds-42"])
        .get();
    assert!(
        (status - 1.0).abs() < f64::EPSILON,
        "buffer status gauge encodes alert/throttle/xor-only/halt states"
    );
    let pnl = metrics
        .settlement_pnl_xor
        .with_label_values(&["lane-1", "ds-42"])
        .get();
    assert!(
        (pnl - u128_to_f64(250)).abs() < f64::EPSILON,
        "pnl gauge captures variance"
    );
    let haircut = metrics
        .settlement_haircut_bp
        .with_label_values(&["lane-1", "ds-42"])
        .get();
    assert!(
        (haircut - 25.0).abs() < f64::EPSILON,
        "haircut gauge captures epsilon bps"
    );
    let swapline = metrics
        .settlement_swapline_utilisation
        .with_label_values(&["lane-1", "ds-42", "tier1-deep"])
        .get();
    assert!(
        (swapline - u128_to_f64(1_000)).abs() < f64::EPSILON,
        "swapline gauge records utilisation"
    );
}
#[test]
fn settlement_conversion_and_haircut_totals_increment() {
    let metrics = Metrics::default();
    metrics.inc_settlement_conversion_total("lane-1", "ds-7", "61CtjvNd9T3THAR65GsMVHr82Bjc", 4);
    metrics.inc_settlement_haircut_total("lane-1", "ds-7", 3_500_000);
    let conversions = metrics
        .settlement_conversion_total
        .with_label_values(&["lane-1", "ds-7", "61CtjvNd9T3THAR65GsMVHr82Bjc"])
        .get();
    assert_eq!(conversions, 4);
    let haircut = metrics
        .settlement_haircut_total
        .with_label_values(&["lane-1", "ds-7"])
        .get();
    assert!(
        (haircut - (3_500_000_f64 / 1_000_000.0)).abs() < f64::EPSILON,
        "haircut counter tracks XOR totals"
    );
}
#[test]
fn records_chunk_range_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_chunk_range("car_range", 206, 4_096, None, None, None, None, None);
    let request_counter = metrics
        .torii_sorafs_chunk_range_requests_total
        .with_label_values(&["car_range", "206"])
        .get();
    assert_eq!(request_counter, 1, "chunk-range request counter increments");
    let bytes_counter = metrics
        .torii_sorafs_chunk_range_bytes_total
        .with_label_values(&["car_range"])
        .get();
    assert_eq!(
        bytes_counter, 4_096,
        "chunk-range bytes counter tracks payload"
    );
    metrics.set_sorafs_provider_range_capability("providers", 2);
    let provider_total = metrics
        .torii_sorafs_provider_range_capability_total
        .with_label_values(&["providers"])
        .get();
    assert_eq!(provider_total, 2, "provider capability gauge updates");
    metrics.inc_sorafs_routing_authority_cache("hit");
    metrics.inc_sorafs_routing_authority_cache("stale_rejected");
    metrics.inc_sorafs_routing_authority_cache("fork_rejected");
    metrics.inc_sorafs_routing_authority_cache("unbounded-runtime-value");
    assert_eq!(
        metrics
            .torii_sorafs_routing_authority_cache_total
            .with_label_values(&["hit"])
            .get(),
        1,
        "routing authority cache hit counter increments"
    );
    assert_eq!(
        metrics
            .torii_sorafs_routing_authority_cache_total
            .with_label_values(&["stale_rejected"])
            .get(),
        1,
        "routing authority stale rejection counter increments"
    );
    assert_eq!(
        metrics
            .torii_sorafs_routing_authority_cache_total
            .with_label_values(&["fork_rejected"])
            .get(),
        1,
        "routing authority fork rejection counter increments"
    );
    assert_eq!(
        metrics
            .torii_sorafs_routing_authority_cache_total
            .with_label_values(&["invalid"])
            .get(),
        1,
        "routing authority cache labels remain bounded"
    );
    metrics.inc_sorafs_range_fetch_throttle("concurrency");
    let throttle_total = metrics
        .torii_sorafs_range_fetch_throttle_events_total
        .with_label_values(&["concurrency"])
        .get();
    assert_eq!(throttle_total, 1, "throttle counter increments");
    metrics.inc_sorafs_range_fetch_concurrency();
    assert_eq!(
        metrics.torii_sorafs_range_fetch_concurrency_current.get(),
        1,
        "concurrency gauge increments"
    );
    metrics.dec_sorafs_range_fetch_concurrency();
    assert_eq!(
        metrics.torii_sorafs_range_fetch_concurrency_current.get(),
        0,
        "concurrency gauge decrements"
    );
}
#[test]
fn records_sorafs_gc_metrics() {
    let metrics = Metrics::default();
    metrics.inc_sorafs_gc_runs("success");
    metrics.inc_sorafs_gc_evictions("retention_expired");
    metrics.add_sorafs_gc_freed_bytes("retention_expired", 2_048);
    metrics.inc_sorafs_gc_blocked("repair_active");
    metrics.set_sorafs_gc_expired_snapshot(3, 120);
    assert_eq!(
        metrics
            .torii_sorafs_gc_runs_total
            .with_label_values(&["success"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gc_evictions_total
            .with_label_values(&["retention_expired"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gc_bytes_freed_total
            .with_label_values(&["retention_expired"])
            .get(),
        2_048
    );
    assert_eq!(
        metrics
            .torii_sorafs_gc_blocked_total
            .with_label_values(&["repair_active"])
            .get(),
        1
    );
    assert_eq!(metrics.torii_sorafs_gc_expired_manifests.get(), 3);
    assert_eq!(
        metrics.torii_sorafs_gc_oldest_expired_age_seconds.get(),
        120
    );
}
#[test]
fn records_sorafs_reconciliation_metrics() {
    let metrics = Metrics::default();
    metrics.inc_sorafs_reconciliation_runs("success");
    metrics.set_sorafs_reconciliation_divergence_count(7);
    assert_eq!(
        metrics
            .torii_sorafs_reconciliation_runs_total
            .with_label_values(&["success"])
            .get(),
        1
    );
    assert_eq!(
        metrics.torii_sorafs_reconciliation_divergence_count.get(),
        7
    );
}
#[test]
fn records_orderbook_metrics_used_by_dashboard_and_alerts() {
    let metrics = Metrics::default();
    metrics.record_sorafs_orderbook_finalized_projection(
        42,
        1_800_000_000,
        [1, 2, 0, 3, 0, 0, 0, 4],
        [[42, 5], [6, 7], [8, 9]],
        1,
        7,
        120,
        86_400,
        12,
        11,
    );
    metrics.record_sorafs_orderbook_api_request("/v1/sorafs/orderbook/orders", false);
    metrics.record_sorafs_orderbook_api_request("/v1/sorafs/orderbook/orders", true);
    assert_eq!(
        metrics
            .torii_sorafs_orderbook_finalized_events_total
            .with_label_values(&["trade_matched"])
            .get(),
        3
    );
    assert_eq!(metrics.torii_sorafs_orderbook_settlement_backlog.get(), 7);
    assert_eq!(
        metrics
            .torii_sorafs_orderbook_finalized_projection_ready
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_orderbook_api_requests_total
            .with_label_values(&["orders", "error"])
            .get(),
        1
    );
    let exported = metrics.try_to_string().expect("metrics text");
    for metric_name in [
        "torii_sorafs_orderbook_finalized_events_total",
        "torii_sorafs_orderbook_open_depth_gib",
        "torii_sorafs_orderbook_matcher_lag_seconds",
        "torii_sorafs_orderbook_settlement_backlog",
        "torii_sorafs_orderbook_oldest_settlement_age_seconds",
        "torii_sorafs_orderbook_escrow_runway_seconds",
        "torii_sorafs_orderbook_finalized_projection_ready",
        "torii_sorafs_orderbook_finalized_projection_height",
        "torii_sorafs_orderbook_finalized_projection_timestamp_seconds",
        "torii_sorafs_orderbook_finalized_projection_failures_total",
        "torii_sorafs_orderbook_book_revision",
        "torii_sorafs_orderbook_matcher_scan_book_revision",
        "torii_sorafs_orderbook_api_requests_total",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing orderbook metric {metric_name} from export:\n{exported}"
        );
    }
}
#[test]
fn gateway_compliance_metrics_are_registered_and_exposable() {
    let metrics = Metrics::default();
    metrics
        .torii_sorafs_gateway_compliance_requests_total
        .with_label_values(&["status", "success"])
        .inc();
    metrics
        .torii_sorafs_gateway_compliance_serving_decisions_total
        .with_label_values(&["cid", "deny", "legal_safety_hold"])
        .inc();
    metrics
        .torii_sorafs_gateway_compliance_failures_total
        .with_label_values(&["serving", "expired_catalog"])
        .inc();
    metrics
        .torii_sorafs_gateway_compliance_serving_catalog_sequence
        .set(7);
    metrics
        .torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds
        .set(1_800_000_000);
    metrics.torii_sorafs_gateway_compliance_ready.set(1);
    let exported = metrics.try_to_string().expect("metrics text");
    for metric_name in [
        "torii_sorafs_gateway_compliance_requests_total",
        "torii_sorafs_gateway_compliance_serving_decisions_total",
        "torii_sorafs_gateway_compliance_failures_total",
        "torii_sorafs_gateway_compliance_serving_catalog_sequence",
        "torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds",
        "torii_sorafs_gateway_compliance_ready",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing gateway compliance metric {metric_name} from export:\n{exported}"
        );
    }
}
#[test]
fn orderbook_metric_labels_are_bounded_and_fail_closed() {
    let metrics = Metrics::default();
    metrics.record_sorafs_orderbook_api_request("/attacker/controlled/path", true);
    metrics.record_sorafs_orderbook_finalized_projection_failure("attacker-controlled-reason");
    assert_eq!(
        metrics
            .torii_sorafs_orderbook_api_requests_total
            .with_label_values(&["other", "error"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_orderbook_finalized_projection_failures_total
            .with_label_values(&["other"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_orderbook_finalized_projection_ready
            .get(),
        0
    );
}
#[test]
fn records_gateway_compliance_metrics_used_by_dashboard_and_alerts() {
    let metrics = Metrics::default();
    metrics.record_sorafs_gateway_compliance_request("promote", "success");
    metrics.record_sorafs_gateway_compliance_serving_decision(
        "manifest_digest",
        "deny",
        "legal_safety_hold",
    );
    metrics.record_sorafs_gateway_compliance_failure("serving", "expired_catalog");
    metrics.record_sorafs_gateway_compliance_serving_catalog(Some(42), Some(1_800_003_600), true);
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_requests_total
            .with_label_values(&["promote", "success"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_serving_decisions_total
            .with_label_values(&["manifest_digest", "deny", "legal_safety_hold"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_failures_total
            .with_label_values(&["serving", "expired_catalog"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_serving_catalog_sequence
            .get(),
        42
    );
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds
            .get(),
        1_800_003_600
    );
    assert_eq!(metrics.torii_sorafs_gateway_compliance_ready.get(), 1);
    let exported = metrics.try_to_string().expect("metrics text");
    for metric_name in [
        "torii_sorafs_gateway_compliance_requests_total",
        "torii_sorafs_gateway_compliance_serving_decisions_total",
        "torii_sorafs_gateway_compliance_failures_total",
        "torii_sorafs_gateway_compliance_serving_catalog_sequence",
        "torii_sorafs_gateway_compliance_serving_catalog_valid_until_seconds",
        "torii_sorafs_gateway_compliance_ready",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing gateway compliance metric {metric_name} from export:\n{exported}"
        );
    }
}
#[test]
fn gateway_compliance_metric_labels_are_bounded_and_state_fails_closed() {
    let metrics = Metrics::default();
    metrics.record_sorafs_gateway_compliance_request(
        "/attacker/controlled/path",
        "attacker-controlled-outcome",
    );
    metrics.record_sorafs_gateway_compliance_serving_decision(
        "attacker-controlled-kind",
        "attacker-controlled-disposition",
        "attacker-controlled-source",
    );
    metrics.record_sorafs_gateway_compliance_failure(
        "attacker-controlled-surface",
        "attacker-controlled-class",
    );
    metrics.record_sorafs_gateway_compliance_serving_catalog(Some(9), Some(10), true);
    metrics.mark_sorafs_gateway_compliance_unready();
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_requests_total
            .with_label_values(&["other", "internal_error"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_serving_decisions_total
            .with_label_values(&["other", "other", "other"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_gateway_compliance_failures_total
            .with_label_values(&["other", "internal"])
            .get(),
        1
    );
    assert_eq!(metrics.torii_sorafs_gateway_compliance_ready.get(), 0);
}
#[test]
fn records_hedging_billing_metrics_used_by_dashboard_and_alerts() {
    let metrics = Metrics::default();
    metrics.set_sorafs_hedging_reference_price_micro_usd("localnet", 2_000_000);
    metrics.set_sorafs_hedging_feed_lag_seconds("localnet", "primary", 120);
    metrics.set_sorafs_hedging_feed_divergence_bps("localnet", "primary", 75);
    metrics.set_sorafs_hedging_exposure_drift_bps("localnet", "xor", 250);
    metrics.record_sorafs_billing_statement_generation("localnet", "provider", true);
    metrics.record_sorafs_billing_statement_generation("localnet", "provider", false);
    metrics.set_sorafs_billing_statement_ack_backlog("localnet", 9);
    metrics.set_sorafs_billing_escrow_runway_seconds("localnet", "provider", 172_800);
    assert_eq!(
        metrics
            .torii_sorafs_hedging_xor_usd_reference_price_micro_usd
            .with_label_values(&["localnet"])
            .get(),
        2_000_000
    );
    assert_eq!(
        metrics
            .torii_sorafs_hedging_feed_lag_seconds
            .with_label_values(&["localnet", "primary"])
            .get(),
        120
    );
    assert_eq!(
        metrics
            .torii_sorafs_billing_statement_generation_total
            .with_label_values(&["localnet", "provider"])
            .get(),
        2
    );
    assert_eq!(
        metrics
            .torii_sorafs_billing_statement_failure_total
            .with_label_values(&["localnet", "provider"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_billing_statement_ack_backlog
            .with_label_values(&["localnet"])
            .get(),
        9
    );
    let exported = metrics.try_to_string().expect("metrics text");
    for metric_name in [
        "torii_sorafs_hedging_xor_usd_reference_price_micro_usd",
        "torii_sorafs_hedging_feed_lag_seconds",
        "torii_sorafs_hedging_feed_divergence_bps",
        "torii_sorafs_hedging_exposure_drift_bps",
        "torii_sorafs_billing_statement_generation_total",
        "torii_sorafs_billing_statement_failure_total",
        "torii_sorafs_billing_statement_ack_backlog",
        "torii_sorafs_billing_escrow_runway_seconds",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing hedging/billing metric {metric_name} from export:\n{exported}"
        );
    }
}
fn record_sample_sorafs_reserve_finalized_projection(metrics: &Metrics) {
    metrics.record_sorafs_reserve_finalized_projection(&SorafsReserveFinalizedProjection {
        finalized_height: 42,
        lifecycle_stage_counts: [2, 0, 0, 0, 1],
        credit_principal_micro_xor: [120_000_000, 0, 0, 0, 7_000_000],
        credit_shortfall_micro_xor: [5_000_000, 0, 0, 0, 1_000_000],
        accrued_interest_micro_xor: [45_000, 0, 0, 0, 9_000],
        open_appeals: 3,
        custody_counts: [1, 2, 1],
        chain_reconciled_counts: [2, 1],
    });
}
#[test]
fn records_sorafs_reserve_finalized_projection_metrics() {
    let metrics = Metrics::default();
    record_sample_sorafs_reserve_finalized_projection(&metrics);
    assert_eq!(
        metrics
            .torii_sorafs_reserve_lifecycle_stage_providers
            .with_label_values(&["active"])
            .get(),
        2
    );
    assert_eq!(metrics.torii_sorafs_reserve_defaulted_providers.get(), 1);
    assert_eq!(metrics.torii_sorafs_reserve_appeal_backlog.get(), 3);
    assert_eq!(
        metrics
            .torii_sorafs_reserve_custody_movements
            .with_label_values(&["approved"])
            .get(),
        2
    );
    assert_eq!(
        metrics
            .torii_sorafs_reserve_chain_reconciled_movements
            .with_label_values(&["approved"])
            .get(),
        2
    );
    let active_credit_draw = metrics
        .torii_sorafs_reserve_credit_draw_micro_xor
        .with_label_values(&["active"])
        .get();
    assert_eq!(
        active_credit_draw.to_bits(),
        120_000_000_f64.to_bits(),
        "the integer micro-XOR value must be represented exactly"
    );
    assert_eq!(
        metrics
            .torii_sorafs_reserve_finalized_projection_ready
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_reserve_finalized_projection_height
            .get(),
        42
    );
}
#[test]
fn bounds_sorafs_reserve_service_labels_and_records_failures() {
    let metrics = Metrics::default();
    metrics.record_sorafs_reserve_service_request("top_up", "accepted");
    metrics.inc_sorafs_reserve_service_rate_limit("top_up", "quota");
    assert_eq!(
        metrics
            .torii_sorafs_reserve_service_requests_total
            .with_label_values(&["top_up", "accepted"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_reserve_service_rate_limit_total
            .with_label_values(&["top_up", "quota"])
            .get(),
        1
    );
    metrics.record_sorafs_reserve_service_request("attacker-controlled", "also-controlled");
    assert_eq!(
        metrics
            .torii_sorafs_reserve_service_requests_total
            .with_label_values(&["unknown", "unknown"])
            .get(),
        1
    );
    record_sample_sorafs_reserve_finalized_projection(&metrics);
    metrics.record_sorafs_reserve_finalized_projection_failure();
    assert_eq!(
        metrics
            .torii_sorafs_reserve_finalized_projection_ready
            .get(),
        0
    );
    assert_eq!(
        metrics
            .torii_sorafs_reserve_finalized_projection_failure_total
            .get(),
        1
    );
}
#[test]
fn exports_sorafs_reserve_metric_families() {
    let metrics = Metrics::default();
    record_sample_sorafs_reserve_finalized_projection(&metrics);
    metrics.record_sorafs_reserve_service_request("top_up", "accepted");
    metrics.inc_sorafs_reserve_service_rate_limit("top_up", "quota");
    let exported = metrics.try_to_string().expect("metrics text");
    for metric_name in [
        "torii_sorafs_reserve_lifecycle_stage_providers",
        "torii_sorafs_reserve_credit_draw_micro_xor",
        "torii_sorafs_reserve_credit_shortfall_micro_xor",
        "torii_sorafs_reserve_accrued_interest_micro_xor",
        "torii_sorafs_reserve_defaulted_providers",
        "torii_sorafs_reserve_appeal_backlog",
        "torii_sorafs_reserve_custody_movements",
        "torii_sorafs_reserve_chain_reconciled_movements",
        "torii_sorafs_reserve_finalized_projection_ready",
        "torii_sorafs_reserve_finalized_projection_height",
        "torii_sorafs_reserve_finalized_projection_failure_total",
        "torii_sorafs_reserve_service_requests_total",
        "torii_sorafs_reserve_service_rate_limit_total",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing reserve metric {metric_name} from export:\n{exported}"
        );
    }
}
#[test]
fn records_sorafs_repair_metrics() {
    let metrics = Metrics::default();
    metrics.inc_sorafs_repair_tasks("queued");
    metrics.observe_sorafs_repair_latency("completed", 12.5);
    metrics.record_sorafs_repair_queue_depths(&[
        ("provider-a".to_string(), 2),
        ("provider-b".to_string(), 1),
    ]);
    metrics.set_sorafs_repair_backlog_oldest_age_seconds(300);
    metrics.inc_sorafs_repair_lease_expired("requeued");
    metrics.inc_sorafs_slash_proposals("submitted");
    assert_eq!(
        metrics
            .torii_sorafs_repair_tasks_total
            .with_label_values(&["queued"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_repair_queue_depth
            .with_label_values(&["provider-a"])
            .get(),
        2
    );
    assert_eq!(
        metrics
            .torii_sorafs_repair_queue_depth
            .with_label_values(&["provider-b"])
            .get(),
        1
    );
    assert_eq!(
        metrics.torii_sorafs_repair_backlog_oldest_age_seconds.get(),
        300
    );
    assert_eq!(
        metrics
            .torii_sorafs_repair_lease_expired_total
            .with_label_values(&["requeued"])
            .get(),
        1
    );
    assert_eq!(
        metrics
            .torii_sorafs_slash_proposals_total
            .with_label_values(&["submitted"])
            .get(),
        1
    );
}
#[test]
fn repair_otel_handles_noop_without_exporter() {
    let otel = SorafsRepairOtel::new();
    otel.record_task_transition("queued");
    otel.record_latency(1.0, "completed");
    otel.record_backlog_oldest_age_seconds(10.0);
    otel.record_queue_depth(2, "provider-a");
    otel.record_lease_expired("requeued");
    otel.record_slash_proposal("submitted");
    let _ = global_sorafs_repair_otel();
}
#[test]
fn gc_otel_handles_noop_without_exporter() {
    let otel = SorafsGcOtel::new();
    otel.record_run("success");
    otel.record_eviction("retention_expired", 512);
    otel.record_blocked("repair_active");
    let _ = global_sorafs_gc_otel();
}
#[test]
fn records_gar_violation_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_gar_violation("provider", "missing_id");
    let total = metrics
        .torii_sorafs_gar_violations_total
        .with_label_values(&["provider", "missing_id"])
        .get();
    assert_eq!(total, 1, "GAR violation counter increments");
}
#[test]
fn records_gateway_refusal_metrics() {
    let metrics = Metrics::default();
    metrics.record_sorafs_gateway_refusal(
        406,
        "unsupported_chunker",
        "sorafs.sf1@1.0.0",
        "provider123",
        "/v1/sorafs/storage/car/range",
    );
    let total = metrics
        .torii_sorafs_gateway_refusals_total
        .with_label_values(&[
            "unsupported_chunker",
            "sorafs.sf1@1.0.0",
            "provider123",
            "/v1/sorafs/storage/car/range",
        ])
        .get();
    assert_eq!(total, 1, "gateway refusal counter increments");
}
#[test]
fn gateway_otel_handles_noop_without_exporter() {
    let otel = SorafsGatewayOtel::new();
    let request = SorafsGatewayRequestMetricLabels {
        endpoint: "/v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}",
        method: "GET",
        variant: "chunk",
        chunker: "unknown",
        profile: "unknown",
    };
    let response = SorafsGatewayResponseMetricLabels {
        request,
        result: "success",
        status: 206,
        error_code: "none",
    };
    otel.request_started_detailed(request);
    otel.request_completed_detailed(response);
    otel.record_ttfb_detailed(response, 42.0);
    otel.record_proof_verification("sf1", "success", "none", 12.0);
    let _ = global_sorafs_gateway_otel();
}
#[test]
fn exports_canonical_gateway_metric_families() {
    let metrics = Metrics::default();
    let request = SorafsGatewayRequestMetricLabels {
        endpoint: "/v1/sorafs/storage/chunk/{manifest_id}/{chunk_digest}",
        method: "GET",
        variant: "chunk",
        chunker: "unknown",
        profile: "unknown",
    };
    metrics.start_sorafs_gateway_request(request);
    assert_eq!(
        metrics
            .sorafs_gateway_active
            .with_label_values(&[
                request.endpoint,
                request.method,
                request.variant,
                request.chunker,
                request.profile,
            ])
            .get(),
        1
    );
    let response = SorafsGatewayResponseMetricLabels {
        request,
        result: "success",
        status: 206,
        error_code: "none",
    };
    metrics.finish_sorafs_gateway_request(response, 42.0);
    metrics.record_sorafs_gateway_proof_verification("sf1", "failure", "sequence_invalid", 12.0);
    let response_labels = [
        request.endpoint,
        request.method,
        request.variant,
        request.chunker,
        request.profile,
        response.result,
        "206",
        response.error_code,
    ];
    assert_eq!(
        metrics
            .sorafs_gateway_responses_total
            .with_label_values(&response_labels)
            .get(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_gateway_ttfb_ms
            .with_label_values(&response_labels)
            .get_sample_count(),
        1
    );
    assert_eq!(
        metrics
            .sorafs_gateway_proof_verifications_total
            .with_label_values(&["sf1", "failure", "sequence_invalid"])
            .get(),
        1
    );
    let exported = metrics.try_to_string().expect("gateway metrics text");
    for metric_name in [
        "sorafs_gateway_active",
        "sorafs_gateway_responses_total",
        "sorafs_gateway_ttfb_ms_bucket",
        "sorafs_gateway_proof_verifications_total",
        "sorafs_gateway_proof_duration_ms_bucket",
    ] {
        assert!(
            exported.contains(metric_name),
            "missing canonical gateway metric {metric_name} from export"
        );
    }
    for retired_name in [
        "sorafs_gateway_requests_total",
        "sorafs_gateway_proof_events_total",
        "sorafs_gateway_proof_latency_ms",
    ] {
        assert!(
            !exported.contains(retired_name),
            "retired gateway metric {retired_name} must not be exported"
        );
    }
}
#[test]
fn node_otel_handles_noop_without_exporter() {
    let otel = SorafsNodeOtel::new();
    otel.record_storage("provider123", 512, 1_024, 10, 1);
    otel.record_storage("provider123", 768, 1_024, 12, 2);
    let expected_charge = "0.85".parse().expect("canonical quantity");
    let client_debit = "0.6".parse().expect("canonical quantity");
    let zero = Quantity::zero();
    otel.record_deal_settlement(
        "provider123",
        "completed",
        &expected_charge,
        &client_debit,
        &zero,
        &zero,
    );
    let _ = global_sorafs_node_otel();
}
#[test]
fn records_gateway_fixture_metadata() {
    let metrics = Metrics::default();
    metrics.set_sorafs_gateway_fixture_metadata("v1", "sf1", "deadbeef", 123);
    let gauge = metrics
        .torii_sorafs_gateway_fixture_info
        .with_label_values(&["v1", "sf1", "deadbeef"])
        .get();
    assert_eq!(
        gauge, 123,
        "fixture metadata gauge stores release timestamp"
    );
}
#[allow(clippy::too_many_lines)]
fn sample_status() -> Status {
    Status {
        build: BuildStatus {
            version: "2.0.0-rc.test".to_owned(),
            git_commit_sha: "deadbeef".to_owned(),
            dpn_validator_release_commit: "feedface".to_owned(),
            cargo_features: "telemetry,zk-halo2".to_owned(),
            target_triple: "aarch64-apple-darwin".to_owned(),
        },
        observed_at_ms: 1_234_999,
        peers: 4,
        blocks: 5,
        blocks_non_empty: 3,
        commit_time_ms: 130,
        txs_approved: 31,
        txs_rejected: 3,
        last_rejection_at_ms: Some(1_234_890),
        txs_rejected_recent_5m: 2,
        uptime: Uptime(Duration::new(5, 937_000_000)),
        view_changes: 2,
        queue_size: 18,
        queue_queued: 11,
        queue_inflight: 7,
        last_block_committed_at_ms: 1_234_777,
        last_non_empty_block_committed_at_ms: 1_234_555,
        time_since_last_block_ms: 222,
        time_since_last_non_empty_block_ms: 444,
        crypto: CryptoStatus {
            sm_helpers_available: true,
            sm_openssl_preview_enabled: false,
            halo2: Halo2Status {
                enabled: true,
                curve: "pasta".to_string(),
                backend: "ipa".to_string(),
                max_k: 21,
                verifier_budget_ms: 350,
                verifier_max_batch: 8,
            },
        },
        stack: StackStatus {
            requested_scheduler_bytes: 1_048_576,
            requested_prover_bytes: 1_048_576,
            requested_guest_bytes: 1_048_576,
            scheduler_bytes: 1_048_576,
            prover_bytes: 1_048_576,
            guest_bytes: 1_048_576,
            gas_to_stack_multiplier: 4,
            scheduler_clamped: false,
            prover_clamped: false,
            guest_clamped: false,
            pool_fallback_total: 0,
            budget_hit_total: 0,
        },
        offline: None,
        sumeragi: Some(SumeragiConsensusStatus {
            mode_tag: PERMISSIONED_TAG.to_string(),
            leader_index: 1,
            highest_qc_height: 10,
            locked_qc_height: 9,
            locked_qc_view: 3,
            commit_signatures_present: 6,
            commit_signatures_counted: 5,
            commit_signatures_set_b: 2,
            commit_signatures_required: 5,
            commit_qc_height: 12,
            commit_qc_view: 4,
            commit_qc_epoch: 1,
            commit_qc_signatures_total: 5,
            commit_qc_validator_set_len: 7,
            gossip_fallback_total: 2,
            block_created_dropped_by_lock_total: 1,
            block_created_hint_mismatch_total: 0,
            block_created_proposal_mismatch_total: 0,
            tx_queue_depth: 5,
            tx_queue_capacity: 20,
            tx_queue_retained_bytes: 1_024,
            tx_queue_max_retained_bytes: 65_536,
            tx_queue_saturated: false,
            tx_queue_saturated_by_count: false,
            tx_queue_saturated_by_bytes: false,
            tx_queue_saturated_by_age: false,
            tx_queue_oldest_queued_age_ms: 250,
            epoch_length_blocks: 0,
            epoch_commit_deadline_offset: 0,
            epoch_reveal_deadline_offset: 0,
            view_change_proof_accepted_total: 4,
            view_change_proof_stale_total: 1,
            view_change_proof_rejected_total: 0,
            view_change_install_total: 0,
            view_change_suggest_total: 0,
            prf_epoch_seed: Some("cafebabe42".to_string()),
            prf_height: 11,
            prf_view: 2,
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            ..SumeragiConsensusStatus::default()
        }),
        governance: GovernanceStatus {
            proposals: GovernanceProposalCounters {
                proposed: 2,
                approved: 1,
                rejected: 0,
                enacted: 1,
            },
            protected_namespace: GovernanceProtectedNamespaceCounters {
                total_checks: 5,
                allowed: 4,
                rejected: 1,
            },
            manifest_admission: GovernanceManifestAdmissionCounters {
                total_checks: 6,
                allowed: 4,
                missing_manifest: 1,
                non_validator_authority: 0,
                quorum_rejected: 1,
                protected_namespace_rejected: 0,
                runtime_hook_rejected: 0,
            },
            manifest_quorum: GovernanceManifestQuorumCounters {
                total_checks: 4,
                satisfied: 3,
                rejected: 1,
            },
            recent_manifest_activations: vec![GovernanceManifestActivation {
                contract_address: "xorc1qyqqqqqqqqqqqq9a5v7f58jgm40m0w7esnqg2pxj68d3f8a2l9ja3s"
                    .to_string(),
                code_hash_hex: "deadbeef".to_string(),
                abi_hash_hex: Some("cafebabe".to_string()),
                height: 42,
                activated_at_ms: 1_234_567,
            }],
            sealed_lanes_total: 0,
            sealed_lane_aliases: Vec::new(),
            citizens_total: 0,
        },
        teu_lane_commit: Vec::new(),
        teu_dataspace_backlog: Vec::new(),
        dataspace_catalog: Vec::new(),
        nexus: None,
        tx_gossip: TxGossipSnapshot {
            caps: TxGossipCaps {
                frame_cap_bytes: 0,
                public_target_cap: None,
                restricted_target_cap: None,
                public_target_reshuffle_ms: None,
                restricted_target_reshuffle_ms: None,
                drop_unknown_dataspace: false,
                restricted_fallback: "drop".to_string(),
                restricted_public_policy: "refuse".to_string(),
            },
            targets: Vec::new(),
        },
        sorafs_micropayments: Vec::new(),
        taikai_alias_rotations: Vec::new(),
        taikai_ingest: Vec::new(),
        da_receipt_cursors: Vec::new(),
    }
}
#[test]
fn build_sumeragi_status_uses_cached_immutable_mode() {
    let metrics = Metrics::default();
    metrics.set_sumeragi_mode_tag("custom-mode");
    let status = build_sumeragi_status(&metrics);
    assert_eq!(status.mode_tag, "custom-mode");
}
#[test]
fn build_sumeragi_status_promotes_stale_qc_gauges_to_commit_qc() {
    let metrics = Metrics::default();
    metrics.sumeragi_highest_qc_height.set(3_052);
    metrics.sumeragi_locked_qc_height.set(3_052);
    metrics.sumeragi_locked_qc_view.set(1);
    metrics.sumeragi_commit_qc_height.set(4_468);
    metrics.sumeragi_commit_qc_view.set(7);
    let status = build_sumeragi_status(&metrics);
    assert_eq!(status.commit_qc_height, 4_468);
    assert_eq!(status.commit_qc_view, 7);
    assert_eq!(status.highest_qc_height, 4_468);
    assert_eq!(status.locked_qc_height, 4_468);
    assert_eq!(status.locked_qc_view, 7);
}
#[test]
fn build_sumeragi_status_preserves_qc_gauges_newer_than_commit_qc() {
    let metrics = Metrics::default();
    metrics.sumeragi_highest_qc_height.set(4_470);
    metrics.sumeragi_locked_qc_height.set(4_469);
    metrics.sumeragi_locked_qc_view.set(2);
    metrics.sumeragi_commit_qc_height.set(4_468);
    metrics.sumeragi_commit_qc_view.set(7);
    let status = build_sumeragi_status(&metrics);
    assert_eq!(status.highest_qc_height, 4_470);
    assert_eq!(status.locked_qc_height, 4_469);
    assert_eq!(status.locked_qc_view, 2);
}
#[test]
fn build_sumeragi_status_includes_tx_queue_pressure_causes() {
    let metrics = Metrics::default();
    metrics.sumeragi_tx_queue_depth.set(31);
    metrics.sumeragi_tx_queue_capacity.set(64);
    metrics.sumeragi_tx_queue_retained_bytes.set(98_304);
    metrics.sumeragi_tx_queue_max_retained_bytes.set(131_072);
    metrics.sumeragi_tx_queue_saturated.set(1);
    metrics.sumeragi_tx_queue_saturated_by_count.set(0);
    metrics.sumeragi_tx_queue_saturated_by_bytes.set(1);
    metrics.sumeragi_tx_queue_saturated_by_age.set(1);
    metrics.sumeragi_tx_queue_oldest_queued_age_ms.set(7_500);
    let status = build_sumeragi_status(&metrics);
    assert_eq!(status.tx_queue_depth, 31);
    assert_eq!(status.tx_queue_capacity, 64);
    assert_eq!(status.tx_queue_retained_bytes, 98_304);
    assert_eq!(status.tx_queue_max_retained_bytes, 131_072);
    assert!(status.tx_queue_saturated);
    assert!(!status.tx_queue_saturated_by_count);
    assert!(status.tx_queue_saturated_by_bytes);
    assert!(status.tx_queue_saturated_by_age);
    assert_eq!(status.tx_queue_oldest_queued_age_ms, 7_500);
}
#[test]
#[allow(clippy::too_many_lines)]
fn serialize_status_json() {
    let value = sample_status();
    let actual = json::to_json_pretty(&value).expect("Sample is valid");
    let actual_value: Value = json::from_json(&actual).expect("pretty JSON should parse");
    let expected_value = norito::json!({
        "build": {
            "version": "2.0.0-rc.test",
            "git_commit_sha": "deadbeef",
            "dpn_validator_release_commit": "feedface",
            "cargo_features": "telemetry,zk-halo2",
            "target_triple": "aarch64-apple-darwin"
        },
        "observed_at_ms": 1_234_999,
        "peers": 4,
        "blocks": 5,
        "blocks_non_empty": 3,
        "commit_time_ms": 130,
        "txs_approved": 31,
        "txs_rejected": 3,
        "last_rejection_at_ms": 1_234_890,
        "txs_rejected_recent_5m": 2,
        "uptime": {
            "secs": 5,
            "nanos": 937_000_000
        },
        "view_changes": 2,
        "queue_size": 18,
        "queue_queued": 11,
        "queue_inflight": 7,
        "last_block_committed_at_ms": 1_234_777,
        "last_non_empty_block_committed_at_ms": 1_234_555,
        "time_since_last_block_ms": 222,
        "time_since_last_non_empty_block_ms": 444,
        "crypto": {
            "sm_helpers_available": true,
            "sm_openssl_preview_enabled": false,
            "halo2": {
                "enabled": true,
                "curve": "pasta",
                "backend": "ipa",
                "max_k": 21,
                "verifier_budget_ms": 350,
                "verifier_max_batch": 8
            }
        },
        "stack": {
            "requested_scheduler_bytes": 1_048_576,
            "requested_prover_bytes": 1_048_576,
            "requested_guest_bytes": 1_048_576,
            "scheduler_bytes": 1_048_576,
            "prover_bytes": 1_048_576,
            "guest_bytes": 1_048_576,
            "gas_to_stack_multiplier": 4,
            "scheduler_clamped": false,
            "prover_clamped": false,
            "guest_clamped": false,
            "pool_fallback_total": 0,
            "budget_hit_total": 0
        },
        "sumeragi": {
            "mode_tag": "iroha2-consensus::permissioned-sumeragi@v2",
            "leader_index": 1,
            "highest_qc_height": 10,
            "locked_qc_height": 9,
            "locked_qc_view": 3,
            "commit_signatures_present": 6,
            "commit_signatures_counted": 5,
            "commit_signatures_set_b": 2,
            "commit_signatures_required": 5,
            "commit_qc_height": 12,
            "commit_qc_view": 4,
            "commit_qc_epoch": 1,
            "commit_qc_signatures_total": 5,
            "commit_qc_validator_set_len": 7,
            "gossip_fallback_total": 2,
            "block_created_dropped_by_lock_total": 1,
            "block_created_hint_mismatch_total": 0,
            "block_created_proposal_mismatch_total": 0,
            "tx_queue_depth": 5,
            "tx_queue_capacity": 20,
            "tx_queue_retained_bytes": 1_024,
            "tx_queue_max_retained_bytes": 65_536,
            "tx_queue_saturated": false,
            "tx_queue_saturated_by_count": false,
            "tx_queue_saturated_by_bytes": false,
            "tx_queue_saturated_by_age": false,
            "tx_queue_oldest_queued_age_ms": 250,
            "epoch_length_blocks": 0,
            "epoch_commit_deadline_offset": 0,
            "epoch_reveal_deadline_offset": 0,
            "prf_epoch_seed": "cafebabe42",
            "prf_height": 11,
            "prf_view": 2,
            "view_change_proof_accepted_total": 4,
            "view_change_proof_stale_total": 1,
            "view_change_proof_rejected_total": 0,
            "view_change_suggest_total": 0,
            "view_change_install_total": 0,
            "lane_governance_sealed_total": 0,
            "lane_governance_sealed_aliases": []
        },
        "governance": {
            "proposals": {
                "proposed": 2,
                "approved": 1,
                "rejected": 0,
                "enacted": 1
            },
            "protected_namespace": {
                "total_checks": 5,
                "allowed": 4,
                "rejected": 1
            },
            "manifest_admission": {
                "total_checks": 6,
                "allowed": 4,
                "missing_manifest": 1,
                "non_validator_authority": 0,
                "quorum_rejected": 1,
                "protected_namespace_rejected": 0,
                "runtime_hook_rejected": 0
            },
            "manifest_quorum": {
                "total_checks": 4,
                "satisfied": 3,
                "rejected": 1
            },
            "recent_manifest_activations": [{
                "contract_address": "xorc1qyqqqqqqqqqqqq9a5v7f58jgm40m0w7esnqg2pxj68d3f8a2l9ja3s",
                "code_hash_hex": "deadbeef",
                "abi_hash_hex": "cafebabe",
                "height": 42,
                "activated_at_ms": 1_234_567
            }],
            "sealed_lanes_total": 0,
            "sealed_lane_aliases": [],
            "citizens_total": 0
        },
        "teu_lane_commit": [],
        "teu_dataspace_backlog": [],
        "tx_gossip": {
            "caps": {
                "frame_cap_bytes": 0,
                "drop_unknown_dataspace": false,
                "restricted_fallback": "drop",
                "restricted_public_policy": "refuse"
            },
            "targets": []
        },
        "sorafs_micropayments": [],
        "taikai_alias_rotations": [],
        "taikai_ingest": [],
        "da_receipt_cursors": []
    });
    assert_eq!(actual_value, expected_value);
    let actual = actual
        .replace("[\n    \n  ]", "[\n  ]")
        .replace("[\n      \n    ]", "[\n  ]");
    // CAUTION: if this is outdated, make sure to update the documentation:
    // https://docs.iroha.tech/reference/torii-endpoints.html#status-and-metrics
    expect_test::expect_file!["fixtures/status_snapshot.v1.json"].assert_eq(&format!("{actual}\n"));
}
#[test]
fn status_from_metrics_includes_queue_and_block_liveness() {
    let metrics = Metrics::default();
    let now = current_unix_time_ms();
    metrics.queue_size.set(8);
    metrics.queue_queued.set(5);
    metrics.queue_inflight.set(3);
    metrics
        .last_block_committed_at_ms
        .set(now.saturating_sub(250));
    metrics
        .last_non_empty_block_committed_at_ms
        .set(now.saturating_sub(500));
    let status = Status::from(&metrics);
    assert!(status.observed_at_ms >= now);
    assert_eq!(status.queue_size, 8);
    assert_eq!(status.queue_queued, 5);
    assert_eq!(status.queue_inflight, 3);
    assert_eq!(status.last_block_committed_at_ms, now.saturating_sub(250));
    assert_eq!(
        status.last_non_empty_block_committed_at_ms,
        now.saturating_sub(500)
    );
    assert!(status.time_since_last_block_ms >= 250);
    assert!(status.time_since_last_non_empty_block_ms >= 500);
}
