//! Tests for the `SoraNet` privacy metrics aggregation pipeline.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use iroha_data_model::soranet::privacy_metrics::{
    SoranetGarAbuseShareV1, SoranetPowFailureCountV1, SoranetPowFailureReasonV1,
    SoranetPrivacyEventActiveSampleV1, SoranetPrivacyEventGarAbuseCategoryV1,
    SoranetPrivacyEventHandshakeFailureV1, SoranetPrivacyEventHandshakeSuccessV1,
    SoranetPrivacyEventKindV1, SoranetPrivacyEventThrottleV1, SoranetPrivacyEventV1,
    SoranetPrivacyEventVerifiedBytesV1, SoranetPrivacyHandshakeFailureV1, SoranetPrivacyModeV1,
    SoranetPrivacyPrioShareV1, SoranetPrivacySuppressionReasonV1, SoranetPrivacyThrottleScopeV1,
};
use iroha_telemetry::privacy::{
    HandshakeFailure, PrivacyBucketConfig, PrivacyThrottleScope, SoranetSecureAggregator,
};
use norito::json;

fn ts(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
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

    aggregator.record_event(&SoranetPrivacyEventV1 {
        timestamp_unix: 60,
        mode,
        kind: SoranetPrivacyEventKindV1::HandshakeSuccess(SoranetPrivacyEventHandshakeSuccessV1 {
            rtt_ms: Some(90),
            active_circuits_after: Some(5),
        }),
    });
    aggregator.record_event(&SoranetPrivacyEventV1 {
        timestamp_unix: 65,
        mode,
        kind: SoranetPrivacyEventKindV1::HandshakeFailure(SoranetPrivacyEventHandshakeFailureV1 {
            reason: SoranetPrivacyHandshakeFailureV1::Pow,
            detail: None,
            rtt_ms: Some(110),
        }),
    });
    aggregator.record_event(&SoranetPrivacyEventV1 {
        timestamp_unix: 66,
        mode,
        kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 {
            scope: SoranetPrivacyThrottleScopeV1::RemoteQuota,
        }),
    });
    aggregator.record_event(&SoranetPrivacyEventV1 {
        timestamp_unix: 68,
        mode,
        kind: SoranetPrivacyEventKindV1::Throttle(SoranetPrivacyEventThrottleV1 {
            scope: SoranetPrivacyThrottleScopeV1::Emergency,
        }),
    });
    aggregator.record_event(&SoranetPrivacyEventV1 {
        timestamp_unix: 70,
        mode,
        kind: SoranetPrivacyEventKindV1::ActiveSample(SoranetPrivacyEventActiveSampleV1 {
            active_circuits: 7,
        }),
    });
    aggregator.record_event(&SoranetPrivacyEventV1 {
        timestamp_unix: 72,
        mode,
        kind: SoranetPrivacyEventKindV1::VerifiedBytes(SoranetPrivacyEventVerifiedBytesV1 {
            bytes: 2_048,
        }),
    });
    aggregator.record_event(&SoranetPrivacyEventV1 {
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
                    detail: None,
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
        .ingest_ndjson(&ndjson)
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

    let mut share1 = SoranetPrivacyPrioShareV1::new(1, 120, 60);
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
        .ingest_prio_share(share1)
        .expect("share ingested");
    assert!(aggregator.drain_ready(ts(180)).is_empty());

    let mut share2 = SoranetPrivacyPrioShareV1::new(2, 120, 60);
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
        .ingest_prio_share(share2)
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

    let mut share1 = SoranetPrivacyPrioShareV1::new(1, 120, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 1;
    share1.active_circuits_sum_share = i64::MAX;
    share1.active_circuits_sample_share = 1;
    share1.active_circuits_max_observed = Some(u64::MAX);

    let mut share2 = share1.clone();
    share2.collector_id = 2;
    share2.active_circuits_sample_share = 0;

    let mut share3 = share1.clone();
    share3.collector_id = 3;
    share3.active_circuits_sample_share = 0;

    aggregator
        .ingest_prio_share(share1)
        .expect("share ingested");
    aggregator
        .ingest_prio_share(share2)
        .expect("share ingested");
    aggregator
        .ingest_prio_share(share3)
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

    let mut share1 = SoranetPrivacyPrioShareV1::new(1, 60, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 2;
    share1.active_circuits_sum_share = 20;
    share1.active_circuits_sample_share = 2;
    share1.verified_bytes_share = 512;
    share1.rtt_bucket_shares = vec![0; 16];

    let mut share2 = SoranetPrivacyPrioShareV1::new(2, 60, 60);
    share2.mode = mode;
    share2.handshake_accept_share = 1;
    share2.handshake_pow_reject_share = 1;
    share2.active_circuits_sum_share = 15;
    share2.active_circuits_sample_share = 1;
    share2.verified_bytes_share = 256;
    share2.rtt_bucket_shares = vec![0; 16];

    aggregator
        .ingest_prio_share(share1)
        .expect("share ingested");
    aggregator
        .ingest_prio_share(share2)
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

    let mut share1 = SoranetPrivacyPrioShareV1::new(1, 60, 60);
    share1.mode = mode;
    share1.handshake_accept_share = 3;
    share1.suppressed = true;

    let mut share2 = SoranetPrivacyPrioShareV1::new(2, 60, 60);
    share2.mode = mode;
    share2.handshake_accept_share = 2;
    share2.suppressed = true;

    aggregator
        .ingest_prio_share(share1)
        .expect("share ingested");
    aggregator
        .ingest_prio_share(share2)
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

    let mut share = SoranetPrivacyPrioShareV1::new(1, 60, 60);
    share.mode = mode;
    share.handshake_accept_share = 1;

    aggregator.ingest_prio_share(share).expect("share ingested");

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
