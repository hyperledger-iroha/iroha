//! Tests for the `SoraNet` privacy metrics aggregation pipeline.
use iroha_data_model::soranet::privacy_metrics::{
    SoranetGarAbuseShareV1, SoranetPowFailureCountV1, SoranetPowFailureReasonV1,
    SoranetPrivacyEventActiveSampleV1, SoranetPrivacyEventGarAbuseCategoryV1,
    SoranetPrivacyEventHandshakeFailureV1, SoranetPrivacyEventHandshakeSuccessV1,
    SoranetPrivacyEventKindV1, SoranetPrivacyEventThrottleV1, SoranetPrivacyEventV1,
    SoranetPrivacyEventVerifiedBytesV1, SoranetPrivacyHandshakeFailureV1, SoranetPrivacyModeV1,
    SoranetPrivacyPrioShareV1, SoranetPrivacySuppressionReasonV1, SoranetPrivacyThrottleScopeV1,
};
use iroha_telemetry::privacy::{
    HandshakeFailure, MAX_PRIVACY_BUCKET_BACKLOG_V1, MAX_PRIVACY_BUCKET_WINDOW_V1,
    MAX_PRIVACY_COLLECTOR_SHARES_V1, PrivacyBucketConfig, PrivacyEventError, PrivacyShareError,
    PrivacyThrottleScope, SoranetSecureAggregator,
};
use norito::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
fn ts(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}
fn collector_id(value: u8) -> [u8; 32] {
    [value; 32]
}
#[test]
fn emits_bucket_once_min_contributors_met() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 3,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 16,
        expected_shares: 1,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let base = ts(120);
    aggregator.record_handshake_success(mode, base, Some(80), Some(5));
    aggregator.record_throttle(mode, base, PrivacyThrottleScope::Congestion);
    aggregator.record_throttle(mode, base, PrivacyThrottleScope::DescriptorReplay);
    aggregator.record_throttle(mode, base, PrivacyThrottleScope::Emergency);
    aggregator.record_verified_bytes(mode, base, 4_096);
    aggregator.record_gar_category(mode, base, "Policy::Spam");
    aggregator.record_handshake_failure(
        mode,
        base + Duration::from_secs(5),
        HandshakeFailure::Pow {
            reason: SoranetPowFailureReasonV1::Replay,
        },
        Some(150),
    );
    aggregator.record_handshake_success(mode, base + Duration::from_secs(10), Some(120), Some(6));
    aggregator.record_active_sample(mode, base + Duration::from_secs(30), 8);
    let buckets = aggregator.drain_ready(ts(180));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert_eq!(bucket.bucket_start_unix, 120);
    assert_eq!(bucket.bucket_duration_secs, 60);
    assert!(!bucket.is_suppressed());
    assert!(bucket.suppression_reason.is_none());
    assert_eq!(bucket.contributor_count, 3);
    assert_eq!(bucket.handshake_accept_total, 2);
    assert_eq!(bucket.handshake_pow_reject_total, 1);
    assert_eq!(bucket.pow_rejects_by_reason.len(), 1);
    assert_eq!(
        bucket.pow_rejects_by_reason[0],
        SoranetPowFailureCountV1 {
            reason: SoranetPowFailureReasonV1::Replay,
            count: 1
        }
    );
    assert_eq!(bucket.handshake_downgrade_total, 0);
    assert_eq!(bucket.handshake_timeout_total, 0);
    assert_eq!(bucket.throttle_congestion_total, 1);
    assert_eq!(bucket.throttle_remote_total, 0);
    assert_eq!(bucket.throttle_descriptor_replay_total, 1);
    assert_eq!(bucket.throttle_emergency_total, 1);
    assert_eq!(bucket.verified_bytes_total, 4_096);
    assert_eq!(bucket.active_circuits_mean, Some(6));
    assert_eq!(bucket.active_circuits_max, Some(8));
    let labels: Vec<_> = bucket
        .rtt_percentiles_ms
        .iter()
        .map(|percentile| percentile.label.as_str())
        .collect();
    assert!(
        labels.contains(&"p50") && labels.contains(&"p90") && labels.contains(&"p99"),
        "missing percentile labels: {labels:?}"
    );
    assert_eq!(bucket.gar_abuse_counts.len(), 1);
    assert_eq!(bucket.gar_abuse_counts[0].count, 1);
    assert_eq!(bucket.throttle_descriptor_total, 0);
}
#[test]
fn force_flush_emits_suppressed_bucket() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 2,
        flush_delay_buckets: 1,
        force_flush_buckets: 2,
        max_completed_buckets: 8,
        expected_shares: 1,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let base = ts(60);
    aggregator.record_handshake_success(mode, base, None, None);
    let buckets = aggregator.drain_ready(ts(240));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert!(bucket.is_suppressed());
    assert_eq!(
        bucket.suppression_reason,
        Some(SoranetPrivacySuppressionReasonV1::ForcedFlushWindowElapsed)
    );
    assert_eq!(bucket.contributor_count, 0);
    assert_eq!(bucket.handshake_events_total(), 0);
    assert_eq!(bucket.verified_bytes_total, 0);
    assert!(bucket.rtt_percentiles_ms.is_empty());
    assert!(bucket.gar_abuse_counts.is_empty());
}
#[test]
fn aggregates_relay_and_replay_reasons() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 2,
        max_completed_buckets: 8,
        expected_shares: 1,
        max_share_lag_buckets: 4,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Entry;
    let base = ts(120);
    aggregator.record_handshake_failure(
        mode,
        base,
        HandshakeFailure::Pow {
            reason: SoranetPowFailureReasonV1::Replay,
        },
        None,
    );
    aggregator.record_handshake_failure(
        mode,
        base + Duration::from_secs(5),
        HandshakeFailure::Pow {
            reason: SoranetPowFailureReasonV1::RelayMismatch,
        },
        None,
    );
    let buckets = aggregator.drain_ready(ts(240));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.handshake_pow_reject_total, 2);
    assert_eq!(bucket.pow_rejects_by_reason.len(), 2);
    let mut reasons = bucket.pow_rejects_by_reason.clone();
    reasons.sort_by_key(|entry| entry.reason);
    assert_eq!(
        reasons[0],
        SoranetPowFailureCountV1 {
            reason: SoranetPowFailureReasonV1::RelayMismatch,
            count: 1
        }
    );
    assert_eq!(
        reasons[1],
        SoranetPowFailureCountV1 {
            reason: SoranetPowFailureReasonV1::Replay,
            count: 1
        }
    );
}
#[test]
fn gar_categories_are_hashed_and_counted() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 0,
        force_flush_buckets: 1,
        max_completed_buckets: 4,
        expected_shares: 1,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let base = ts(60);
    aggregator.record_handshake_success(mode, base, None, None);
    aggregator.record_gar_category(mode, base, "Abuse::Spam");
    aggregator.record_gar_category(mode, base, "Abuse::Spam");
    aggregator.record_gar_category(mode, base, "Abuse::Fraud");
    let buckets = aggregator.drain_ready(ts(120));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert!(!bucket.gar_abuse_counts.is_empty());
    assert_eq!(bucket.gar_abuse_counts.len(), 2);
    let mut counts: Vec<_> = bucket
        .gar_abuse_counts
        .iter()
        .map(|entry| entry.count)
        .collect();
    counts.sort_unstable();
    assert_eq!(counts, vec![1, 2]);
    for entry in &bucket.gar_abuse_counts {
        assert_ne!(entry.category_hash, [0u8; 8]);
    }
}
#[test]
fn record_event_api_routes_to_expected_counters() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 2,
        flush_delay_buckets: 0,
        force_flush_buckets: 2,
        max_completed_buckets: 8,
        expected_shares: 1,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let record = |event| {
        aggregator
            .record_historical_event(&event)
            .expect("historical event ingested");
    };
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 60,
        mode,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: Some(90),
            active_circuits_after: Some(5),
        }),
    });
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 65,
        mode,
        kind: SoranetPrivacyEventKindV1::HandshakeFailure(SoranetPrivacyEventHandshakeFailureV1 {
            reason: SoranetPrivacyHandshakeFailureV1::Pow,
            pow_reason: Some(SoranetPowFailureReasonV1::InvalidSolution),
            rtt_ms: Some(110),
        }),
    });
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 66,
        mode,
        kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 {
            scope: SoranetPrivacyThrottleScopeV1::RemoteQuota,
        }),
    });
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 68,
        mode,
        kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 {
            scope: SoranetPrivacyThrottleScopeV1::Emergency,
        }),
    });
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 70,
        mode,
        kind: SoranetPrivacyEventKindV1::ActiveSample(SoranetPrivacyEventActiveSampleV1 {
            active_circuits: 7,
        }),
    });
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 72,
        mode,
        kind: SoranetPrivacyEventKindV1::VerifiedBytes(SoranetPrivacyEventVerifiedBytesV1 {
            bytes: 2_048,
        }),
    });
    record(SoranetPrivacyEventV1 {
        timestamp_unix: 75,
        mode,
        kind: SoranetPrivacyEventKindV1::GarAbuseCategory(SoranetPrivacyEventGarAbuseCategoryV1 {
            label: "Policy::Spam".to_string(),
        }),
    });
    let buckets = aggregator.drain_ready(ts(120));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert!(!bucket.is_suppressed());
    assert!(bucket.suppression_reason.is_none());
    assert_eq!(bucket.contributor_count, 2);
    assert_eq!(bucket.handshake_accept_total, 1);
    assert_eq!(bucket.handshake_pow_reject_total, 1);
    assert_eq!(bucket.pow_rejects_by_reason.len(), 1);
    assert_eq!(
        bucket.pow_rejects_by_reason[0],
        SoranetPowFailureCountV1 {
            reason: SoranetPowFailureReasonV1::InvalidSolution,
            count: 1
        }
    );
    assert_eq!(bucket.throttle_remote_total, 1);
    assert_eq!(bucket.throttle_emergency_total, 1);
    assert_eq!(bucket.verified_bytes_total, 2_048);
    assert_eq!(bucket.gar_abuse_counts.len(), 1);
    assert_eq!(bucket.gar_abuse_counts[0].count, 1);
    assert_eq!(bucket.active_circuits_mean, Some(6));
    assert_eq!(bucket.active_circuits_max, Some(7));
    assert!(!bucket.rtt_percentiles_ms.is_empty());
}
#[test]
#[allow(clippy::too_many_lines)]
fn ndjson_feed_rehydrates_events() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 2,
        flush_delay_buckets: 1,
        force_flush_buckets: 1,
        max_completed_buckets: 8,
        expected_shares: 1,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Entry;
    let events = [
        SoranetPrivacyEventV1 {
            timestamp_unix: 120,
            mode,
            kind: SoranetPrivacyEventKindV1::HandshakeSuccess(
                SoranetPrivacyEventHandshakeSuccessV1 {
                    rtt_ms: Some(75),
                    active_circuits_after: Some(4),
                },
            ),
        },
        SoranetPrivacyEventV1 {
            timestamp_unix: 122,
            mode,
            kind: SoranetPrivacyEventKindV1::HandshakeFailure(
                SoranetPrivacyEventHandshakeFailureV1 {
                    reason: SoranetPrivacyHandshakeFailureV1::Pow,
                    pow_reason: Some(SoranetPowFailureReasonV1::InvalidSolution),
                    rtt_ms: Some(150),
                },
            ),
        },
        SoranetPrivacyEventV1 {
            timestamp_unix: 124,
            mode,
            kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 {
                scope: SoranetPrivacyThrottleScopeV1::Cooldown,
            }),
        },
        SoranetPrivacyEventV1 {
            timestamp_unix: 126,
            mode,
            kind: SoranetPrivacyEventKindV1::VerifiedBytes(SoranetPrivacyEventVerifiedBytesV1 {
                bytes: 1_024,
            }),
        },
        SoranetPrivacyEventV1 {
            timestamp_unix: 128,
            mode,
            kind: SoranetPrivacyEventKindV1::ActiveSample(SoranetPrivacyEventActiveSampleV1 {
                active_circuits: 6,
            }),
        },
        SoranetPrivacyEventV1 {
            timestamp_unix: 130,
            mode,
            kind: SoranetPrivacyEventKindV1::GarAbuseCategory(
                SoranetPrivacyEventGarAbuseCategoryV1 {
                    label: "Policy::Spam".to_string(),
                },
            ),
        },
    ];
    let ndjson = events
        .iter()
        .map(|event| {
            let value = json::to_value(event).expect("serialize event");
            json::to_string(&value).expect("stringify event")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let ingested = aggregator
        .ingest_historical_ndjson(&ndjson)
        .expect("ingest ndjson payload");
    assert_eq!(ingested, events.len());
    let buckets = aggregator.drain_ready(ts(240));
    assert_eq!(buckets.len(), 1, "expected exactly one bucket: {buckets:?}");
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert!(
        !bucket.is_suppressed(),
        "ndjson bucket should not be suppressed"
    );
    assert!(bucket.suppression_reason.is_none());
    assert_eq!(bucket.handshake_accept_total, 1);
    assert_eq!(bucket.handshake_pow_reject_total, 1);
    assert_eq!(bucket.pow_rejects_by_reason.len(), 1);
    assert_eq!(
        bucket.pow_rejects_by_reason[0],
        SoranetPowFailureCountV1 {
            reason: SoranetPowFailureReasonV1::InvalidSolution,
            count: 1
        }
    );
    assert_eq!(bucket.throttle_cooldown_total, 1);
    assert_eq!(bucket.verified_bytes_total, 1_024);
    assert_eq!(bucket.active_circuits_mean, Some(5));
    assert_eq!(bucket.active_circuits_max, Some(6));
    assert_eq!(bucket.gar_abuse_counts.len(), 1);
    assert_eq!(bucket.gar_abuse_counts[0].count, 1);
}

#[test]
fn event_ingress_rejects_noncanonical_typed_pow_reasons_without_opening_bucket() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 1,
        max_completed_buckets: 8,
        expected_shares: 1,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let invalid = [
        SoranetPrivacyEventHandshakeFailureV1 {
            reason: SoranetPrivacyHandshakeFailureV1::Pow,
            pow_reason: None,
            rtt_ms: None,
        },
        SoranetPrivacyEventHandshakeFailureV1 {
            reason: SoranetPrivacyHandshakeFailureV1::Timeout,
            pow_reason: Some(SoranetPowFailureReasonV1::ClockError),
            rtt_ms: None,
        },
    ];
    for payload in invalid {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: 60,
            mode: SoranetPrivacyModeV1::Entry,
            kind: SoranetPrivacyEventKindV1::HandshakeFailure(payload),
        };
        assert!(matches!(
            aggregator.record_historical_event(&event),
            Err(PrivacyEventError::InvalidHandshakeFailureReason)
        ));
    }
    assert!(
        aggregator.drain_ready(ts(180)).is_empty(),
        "rejected events must not create an empty bucket"
    );
}
#[test]
fn prio_shares_combine_into_bucket() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 3,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 16,
        expected_shares: 2,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let mut share1 = SoranetPrivacyPrioShareV1::new(collector_id(1), 120, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 2;
    share1.handshake_pow_reject_share = 1;
    share1.active_circuits_sum_share = 30;
    share1.active_circuits_sample_share = 2;
    share1.active_circuits_max_observed = Some(12);
    share1.verified_bytes_share = 1_024;
    share1.rtt_bucket_shares = vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    share1.gar_abuse_shares = vec![SoranetGarAbuseShareV1::new([1u8; 8], 1)];
    aggregator
        .ingest_historical_prio_share(share1)
        .expect("share ingested");
    assert!(aggregator.drain_ready(ts(180)).is_empty());
    let mut share2 = SoranetPrivacyPrioShareV1::new(collector_id(2), 120, 60);
    share2.mode = mode;
    share2.handshake_accept_share = 1;
    share2.handshake_downgrade_share = 1;
    share2.throttle_congestion_share = 1;
    share2.throttle_remote_share = 1;
    share2.throttle_descriptor_share = 1;
    share2.throttle_emergency_share = 1;
    share2.active_circuits_sum_share = 45;
    share2.active_circuits_sample_share = 3;
    share2.active_circuits_max_observed = Some(18);
    share2.verified_bytes_share = 2_048;
    share2.rtt_bucket_shares = vec![0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    share2.gar_abuse_shares = vec![
        SoranetGarAbuseShareV1::new([1u8; 8], 2),
        SoranetGarAbuseShareV1::new([2u8; 8], 1),
    ];
    aggregator
        .ingest_historical_prio_share(share2)
        .expect("share ingested");
    let buckets = aggregator.drain_ready(ts(240));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert!(!bucket.is_suppressed());
    assert!(bucket.suppression_reason.is_none());
    assert_eq!(bucket.bucket_start_unix, 120);
    assert_eq!(bucket.bucket_duration_secs, 60);
    assert_eq!(bucket.handshake_accept_total, 3);
    assert_eq!(bucket.handshake_pow_reject_total, 1);
    assert!(bucket.pow_rejects_by_reason.is_empty());
    assert_eq!(bucket.handshake_downgrade_total, 1);
    assert_eq!(bucket.handshake_timeout_total, 0);
    assert_eq!(bucket.handshake_other_failure_total, 0);
    assert_eq!(bucket.handshake_events_total(), 5);
    assert_eq!(bucket.contributor_count, 5);
    assert_eq!(bucket.throttle_congestion_total, 1);
    assert_eq!(bucket.throttle_remote_total, 1);
    assert_eq!(bucket.throttle_descriptor_total, 1);
    assert_eq!(bucket.throttle_descriptor_replay_total, 0);
    assert_eq!(bucket.throttle_emergency_total, 1);
    assert_eq!(bucket.verified_bytes_total, 3_072);
    assert_eq!(bucket.active_circuits_mean, Some(15));
    assert_eq!(bucket.active_circuits_max, Some(18));
    assert_eq!(bucket.rtt_percentiles_ms.len(), 3);
    assert_eq!(bucket.rtt_percentiles_ms[0].value_ms, 10);
    assert_eq!(bucket.rtt_percentiles_ms[1].value_ms, 25);
    assert_eq!(bucket.rtt_percentiles_ms[2].value_ms, 25);
    assert_eq!(bucket.gar_abuse_counts.len(), 2);
    let mut gar_counts: Vec<_> = bucket
        .gar_abuse_counts
        .iter()
        .map(|entry| (entry.category_hash, entry.count))
        .collect();
    gar_counts.sort_unstable_by_key(|entry| entry.0);
    assert_eq!(gar_counts[0].1, 3);
    assert_eq!(gar_counts[1].1, 1);
}
#[test]
fn prio_shares_active_average_saturates() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 8,
        expected_shares: 3,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let mut share1 = SoranetPrivacyPrioShareV1::new(collector_id(1), 120, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 1;
    share1.active_circuits_sum_share = i64::MAX;
    share1.active_circuits_sample_share = 1;
    share1.active_circuits_max_observed = Some(u64::MAX);
    let mut share2 = share1.clone();
    share2.collector_id = collector_id(2);
    share2.active_circuits_sample_share = 0;
    let mut share3 = share1.clone();
    share3.collector_id = collector_id(3);
    share3.active_circuits_sample_share = 0;
    aggregator
        .ingest_historical_prio_share(share1)
        .expect("share ingested");
    aggregator
        .ingest_historical_prio_share(share2)
        .expect("share ingested");
    aggregator
        .ingest_historical_prio_share(share3)
        .expect("share ingested");
    let buckets = aggregator.drain_ready(ts(240));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert_eq!(bucket.mode, mode);
    assert!(!bucket.is_suppressed());
    assert!(bucket.suppression_reason.is_none());
    assert_eq!(
        bucket.active_circuits_mean,
        Some(u64::MAX),
        "mean should saturate at u64::MAX when the summed share exceeds the representable range"
    );
    assert_eq!(
        bucket.active_circuits_max,
        Some(u64::MAX),
        "maximum should honour saturated share inputs"
    );
}
#[test]
fn prio_shares_respect_min_contributors() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 6,
        flush_delay_buckets: 1,
        force_flush_buckets: 2,
        max_completed_buckets: 16,
        expected_shares: 2,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Middle;
    let mut share1 = SoranetPrivacyPrioShareV1::new(collector_id(1), 60, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 2;
    share1.active_circuits_sum_share = 20;
    share1.active_circuits_sample_share = 2;
    share1.verified_bytes_share = 512;
    share1.rtt_bucket_shares = vec![0; 16];
    let mut share2 = SoranetPrivacyPrioShareV1::new(collector_id(2), 60, 60);
    share2.mode = mode;
    share2.handshake_accept_share = 1;
    share2.handshake_pow_reject_share = 1;
    share2.active_circuits_sum_share = 15;
    share2.active_circuits_sample_share = 1;
    share2.verified_bytes_share = 256;
    share2.rtt_bucket_shares = vec![0; 16];
    aggregator
        .ingest_historical_prio_share(share1)
        .expect("share ingested");
    aggregator
        .ingest_historical_prio_share(share2)
        .expect("share ingested");
    let buckets = aggregator.drain_ready(ts(180));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert!(bucket.is_suppressed());
    assert_eq!(
        bucket.suppression_reason,
        Some(SoranetPrivacySuppressionReasonV1::InsufficientContributors)
    );
    assert_eq!(bucket.mode, mode);
    assert_eq!(bucket.bucket_start_unix, 60);
    assert_eq!(bucket.bucket_duration_secs, 60);
    assert_eq!(bucket.handshake_events_total(), 0);
    assert_eq!(bucket.verified_bytes_total, 0);
    assert!(bucket.gar_abuse_counts.is_empty());
}
#[test]
fn prio_shares_surface_collector_suppression_reason() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 2,
        max_completed_buckets: 8,
        expected_shares: 2,
        max_share_lag_buckets: 12,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Entry;
    let mut share1 = SoranetPrivacyPrioShareV1::new(collector_id(1), 60, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 3;
    share1.suppressed = true;
    let mut share2 = SoranetPrivacyPrioShareV1::new(collector_id(2), 60, 60);
    share2.mode = mode;
    share2.handshake_accept_share = 2;
    share2.suppressed = true;
    aggregator
        .ingest_historical_prio_share(share1)
        .expect("share ingested");
    aggregator
        .ingest_historical_prio_share(share2)
        .expect("share ingested");
    let buckets = aggregator.drain_ready(ts(120));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert!(bucket.is_suppressed());
    assert_eq!(
        bucket.suppression_reason,
        Some(SoranetPrivacySuppressionReasonV1::CollectorSuppressed)
    );
}
#[test]
fn stale_collector_shares_emit_suppressed_bucket() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 8,
        expected_shares: 2,
        max_share_lag_buckets: 1,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mode = SoranetPrivacyModeV1::Exit;
    let mut share = SoranetPrivacyPrioShareV1::new(collector_id(1), 60, 60);
    share.mode = mode;
    share.handshake_accept_share = 1;
    aggregator
        .ingest_historical_prio_share(share)
        .expect("share ingested");
    let buckets = aggregator.drain_ready(ts(180));
    assert_eq!(buckets.len(), 1);
    let bucket = &buckets[0];
    assert!(bucket.is_suppressed());
    assert_eq!(
        bucket.suppression_reason,
        Some(SoranetPrivacySuppressionReasonV1::CollectorWindowElapsed)
    );
    assert_eq!(bucket.mode, mode);
    assert_eq!(bucket.bucket_start_unix, 60);
}

#[test]
fn live_event_ingress_rejects_future_stale_and_excess_buckets() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 1,
        expected_shares: 2,
        max_share_lag_buckets: 3,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let event = |timestamp_unix, mode| SoranetPrivacyEventV1 {
        timestamp_unix,
        mode,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: None,
            active_circuits_after: None,
        }),
    };

    assert!(matches!(
        aggregator.record_event_at(&event(660, SoranetPrivacyModeV1::Entry), ts(600)),
        Err(PrivacyEventError::FutureBucket { .. })
    ));
    assert!(matches!(
        aggregator.record_event_at(&event(420, SoranetPrivacyModeV1::Entry), ts(600)),
        Err(PrivacyEventError::StaleBucket { .. })
    ));
    aggregator
        .record_event_at(&event(600, SoranetPrivacyModeV1::Entry), ts(600))
        .expect("current event accepted");
    let share = SoranetPrivacyPrioShareV1::new(collector_id(1), 600, 60);
    assert!(matches!(
        aggregator.ingest_prio_share_at(share, ts(600)),
        Err(PrivacyShareError::EventInputConflict { .. })
    ));
    assert!(matches!(
        aggregator.record_event_at(&event(600, SoranetPrivacyModeV1::Exit), ts(600)),
        Err(PrivacyEventError::EventBacklogFull { capacity: 1 })
    ));
}

#[test]
fn live_event_ingress_flushes_ready_state_before_enforcing_backlog() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 1,
        expected_shares: 2,
        max_share_lag_buckets: 3,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let event = |timestamp_unix| SoranetPrivacyEventV1 {
        timestamp_unix,
        mode: SoranetPrivacyModeV1::Entry,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: None,
            active_circuits_after: None,
        }),
    };

    aggregator
        .record_event_at(&event(540), ts(540))
        .expect("first bucket accepted");
    aggregator
        .record_event_at(&event(600), ts(600))
        .expect("ready first bucket must not keep the live backlog full");
    let drained = aggregator.drain_ready(ts(600));
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].bucket_start_unix, 540);
}

#[test]
fn finalized_event_bucket_cannot_be_reopened() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 4,
        expected_shares: 2,
        max_share_lag_buckets: 3,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let event = SoranetPrivacyEventV1 {
        timestamp_unix: 540,
        mode: SoranetPrivacyModeV1::Entry,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: None,
            active_circuits_after: None,
        }),
    };
    aggregator
        .record_event_at(&event, ts(600))
        .expect("previous bucket accepted before finalization");
    assert_eq!(aggregator.drain_ready(ts(600)).len(), 1);
    assert!(matches!(
        aggregator.record_event_at(&event, ts(600)),
        Err(PrivacyEventError::BucketAlreadyFinalized { .. })
    ));
}

#[test]
fn live_share_ingress_is_time_bounded_and_finalization_is_replay_safe() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 4,
        expected_shares: 1,
        max_share_lag_buckets: 3,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mut share = SoranetPrivacyPrioShareV1::new(collector_id(1), 600, 60);
    share.handshake_accept_share = 1;

    let mut future = share.clone();
    future.bucket_start_unix = 660;
    assert!(matches!(
        aggregator.ingest_prio_share_at(future, ts(600)),
        Err(PrivacyShareError::FutureBucket { .. })
    ));
    let mut stale = share.clone();
    stale.bucket_start_unix = 420;
    assert!(matches!(
        aggregator.ingest_prio_share_at(stale, ts(600)),
        Err(PrivacyShareError::StaleBucket { .. })
    ));

    aggregator
        .ingest_prio_share_at(share.clone(), ts(600))
        .expect("current share accepted");
    assert!(matches!(
        aggregator.ingest_prio_share_at(share, ts(600)),
        Err(PrivacyShareError::BucketAlreadyFinalized { .. })
    ));
    assert_eq!(aggregator.drain_ready(ts(600)).len(), 1);
}

#[test]
fn collector_backlog_and_replacement_are_bounded() {
    let config = PrivacyBucketConfig {
        bucket_secs: 60,
        min_contributors: 1,
        flush_delay_buckets: 1,
        force_flush_buckets: 3,
        max_completed_buckets: 1,
        expected_shares: 2,
        max_share_lag_buckets: 3,
    };
    let aggregator = SoranetSecureAggregator::new(config).expect("config valid");
    let mut oversized = SoranetPrivacyPrioShareV1::new(collector_id(6), 600, 60);
    oversized.gar_abuse_shares = vec![SoranetGarAbuseShareV1::new([0; 8], 0); 257];
    assert!(matches!(
        aggregator.ingest_prio_share_at(oversized, ts(600)),
        Err(PrivacyShareError::TooManyGarCategories {
            maximum: 256,
            received: 257
        })
    ));
    let mut first = SoranetPrivacyPrioShareV1::new(collector_id(7), 600, 60);
    first.handshake_accept_share = 1;
    aggregator
        .ingest_prio_share_at(first.clone(), ts(600))
        .expect("first collector share accepted");
    aggregator
        .ingest_prio_share_at(first.clone(), ts(600))
        .expect("exact retry is idempotent");
    let event = SoranetPrivacyEventV1 {
        timestamp_unix: 600,
        mode: SoranetPrivacyModeV1::Entry,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: None,
            active_circuits_after: None,
        }),
    };
    assert!(matches!(
        aggregator.record_event_at(&event, ts(600)),
        Err(PrivacyEventError::CollectorInputConflict { .. })
    ));

    let mut conflicting = first;
    conflicting.handshake_accept_share = 2;
    assert!(matches!(
        aggregator.ingest_prio_share_at(conflicting, ts(600)),
        Err(PrivacyShareError::ConflictingCollectorShare { collector_id: id })
            if id == collector_id(7)
    ));

    let mut other_bucket = SoranetPrivacyPrioShareV1::new(collector_id(8), 600, 60);
    other_bucket.mode = SoranetPrivacyModeV1::Exit;
    assert!(matches!(
        aggregator.ingest_prio_share_at(other_bucket, ts(600)),
        Err(PrivacyShareError::CollectorBacklogFull { capacity: 1 })
    ));
}

#[test]
fn privacy_config_and_event_category_cardinality_are_bounded() {
    for invalid in [
        PrivacyBucketConfig {
            max_completed_buckets: MAX_PRIVACY_BUCKET_BACKLOG_V1 + 1,
            ..PrivacyBucketConfig::default()
        },
        PrivacyBucketConfig {
            force_flush_buckets: MAX_PRIVACY_BUCKET_WINDOW_V1 + 1,
            ..PrivacyBucketConfig::default()
        },
        PrivacyBucketConfig {
            flush_delay_buckets: 0,
            force_flush_buckets: 0,
            ..PrivacyBucketConfig::default()
        },
        PrivacyBucketConfig {
            max_share_lag_buckets: MAX_PRIVACY_BUCKET_WINDOW_V1 + 1,
            ..PrivacyBucketConfig::default()
        },
        PrivacyBucketConfig {
            expected_shares: MAX_PRIVACY_COLLECTOR_SHARES_V1 + 1,
            ..PrivacyBucketConfig::default()
        },
    ] {
        assert!(
            SoranetSecureAggregator::new(invalid).is_err(),
            "unbounded privacy configuration must fail closed"
        );
    }

    let aggregator = SoranetSecureAggregator::new(PrivacyBucketConfig::default())
        .expect("default config is bounded");
    for index in 0..256 {
        let event = SoranetPrivacyEventV1 {
            timestamp_unix: 600,
            mode: SoranetPrivacyModeV1::Entry,
            kind: SoranetPrivacyEventKindV1::GarAbuseCategory(
                SoranetPrivacyEventGarAbuseCategoryV1 {
                    label: format!("category-{index}"),
                },
            ),
        };
        aggregator
            .record_event_at(&event, ts(600))
            .expect("category within bound");
    }
    let excess = SoranetPrivacyEventV1 {
        timestamp_unix: 600,
        mode: SoranetPrivacyModeV1::Entry,
        kind: SoranetPrivacyEventKindV1::GarAbuseCategory(SoranetPrivacyEventGarAbuseCategoryV1 {
            label: "category-excess".to_owned(),
        }),
    };
    assert!(matches!(
        aggregator.record_event_at(&excess, ts(600)),
        Err(PrivacyEventError::TooManyGarCategories { maximum: 256 })
    ));
}
