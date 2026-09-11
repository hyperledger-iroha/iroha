//! Adoption gates, scoreboard comparisons and telemetry burn-in regressions.
use super::*;
use norito::json::to_string_pretty;
use std::{collections::HashSet, fs, path::Path};
use tempfile::tempdir;

const TEST_MANIFEST_ID: &str = "fixture-manifest";
const TEST_MANIFEST_CID: &str = "fixture-cid";
fn summary_with_providers_value(providers: &[(&str, u64)]) -> Value {
    let mut receipts: Vec<Value> = Vec::new();
    let mut chunk_index = 0u64;
    let mut direct_providers = HashSet::new();
    let mut gateway_providers = HashSet::new();
    for (provider, successes) in providers {
        if provider.starts_with("gateway-") {
            gateway_providers.insert(provider.to_string());
        } else {
            direct_providers.insert(provider.to_string());
        }
        for _ in 0..*successes {
            receipts.push(norito::json!({
                "provider": provider,
                "chunk_index": chunk_index,
                "attempts": 1u64,
                "bytes": 1u64,
                "latency_ms": 0.0
            }));
            chunk_index += 1;
        }
    }
    let reports: Vec<Value> = providers
        .iter()
        .map(|(provider, successes)| {
            norito::json!({
                "provider": provider,
                "successes": successes,
                "failures": 0u64,
                "disabled": false
            })
        })
        .collect();
    let total_successes: u64 = providers.iter().map(|(_, successes)| *successes).sum();
    let provider_count =
        u64::try_from(direct_providers.len()).expect("convert direct provider count");
    let gateway_provider_count =
        u64::try_from(gateway_providers.len()).expect("convert gateway provider count");
    let provider_mix = provider_mix_label_from_counts(provider_count, gateway_provider_count);
    let gateway_manifest_provided = gateway_provider_count > 0;
    norito::json!({
        "manifest_id": TEST_MANIFEST_ID,
        "manifest_cid": TEST_MANIFEST_CID,
        "chunk_count": total_successes,
        "chunk_attempt_total": total_successes,
        "chunk_receipts": receipts,
        "provider_reports": reports,
        "provider_success_total": total_successes,
        "provider_count": provider_count,
        "gateway_provider_count": gateway_provider_count,
        "provider_mix": provider_mix,
        "gateway_manifest_provided": gateway_manifest_provided,
        "transport_policy": "soranet-first",
        "transport_policy_override": false,
        "transport_policy_override_label": null,
    })
}

fn write_scoreboard_with_weights(path: &Path, entries: &[(&str, f64)]) {
    let rows: Vec<Value> = entries
        .iter()
        .map(|(provider, weight)| {
            norito::json!({
                "provider_id": provider,
                "eligibility": "eligible",
                "normalised_weight": weight,
            })
        })
        .collect();
    let scoreboard = norito::json!({ "entries": rows });
    fs::write(
        path,
        to_string_pretty(&scoreboard).expect("render scoreboard"),
    )
    .expect("write scoreboard");
}
#[test]
fn adoption_transport_policies_require_exact_v1_labels() {
    for canonical in ["soranet-first", "soranet-strict", "direct-only"] {
        require_canonical_transport_policy(canonical, "test policy")
            .expect("canonical transport policy");
    }
    let path = Path::new("selector-fixture.json");
    for alias in [
        "direct_only",
        "DIRECT-ONLY",
        " direct-only",
        "direct-only ",
        "SoraNet-first",
        "unknown",
    ] {
        require_canonical_transport_policy(alias, "test policy")
            .expect_err("transport policy alias must fail");
        let mut summary = summary_with_providers_value(&[("alpha", 1), ("beta", 1)]);
        summary
            .as_object_mut()
            .expect("summary object")
            .insert("transport_policy".into(), Value::from(alias));
        assert!(
            collect_summary_stats(&summary, path).is_err(),
            "summary transport policy alias must fail"
        );
        let metadata = norito::json!({
            "transport_policy": alias,
            "transport_policy_override": false,
            "transport_policy_override_label": null,
        });
        parse_scoreboard_metadata(&metadata, path)
            .expect_err("scoreboard transport policy alias must fail");
        let mut summary = summary_with_providers_value(&[("alpha", 1), ("beta", 1)]);
        summary
            .as_object_mut()
            .expect("summary object")
            .insert("transport_policy_override_label".into(), Value::from(alias));
        assert!(
            collect_summary_stats(&summary, path).is_err(),
            "summary override label alias must fail"
        );
        let metadata = norito::json!({
            "transport_policy": "soranet-first",
            "transport_policy_override": true,
            "transport_policy_override_label": alias,
        });
        parse_scoreboard_metadata(&metadata, path)
            .expect_err("scoreboard override label alias must fail");
    }
}
fn metadata_with_counts(direct: Option<u64>, gateway: Option<u64>) -> ScoreboardMetadata {
    ScoreboardMetadata {
        provider_count: direct,
        gateway_provider_count: gateway,
        ..Default::default()
    }
}
#[test]
fn metadata_single_source_hint_allows_gateway_multi_source() {
    let meta = metadata_with_counts(Some(0), Some(2));
    assert!(
        metadata_single_source_hint(&meta).is_none(),
        "gateway multi-source should not trigger fallback"
    );
}
#[test]
fn metadata_single_source_hint_allows_mixed_provider_classes() {
    let meta = metadata_with_counts(Some(1), Some(1));
    assert!(
        metadata_single_source_hint(&meta).is_none(),
        "mixed direct+gateway providers should pass"
    );
}
#[test]
fn metadata_single_source_hint_flags_single_gateway_provider() {
    let meta = metadata_with_counts(Some(0), Some(1));
    assert_eq!(
        metadata_single_source_hint(&meta),
        Some(("gateway_provider_count", 1))
    );
}
#[test]
fn burn_in_check_accepts_valid_window() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("burnin.log");
    fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"a\" region=\"lab\" job_id=\"job-1\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"a\" region=\"lab\" job_id=\"job-1\" anonymity_ratio=0.98 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
",
        )
        .expect("write log");
    let summary = run_burn_in_check(BurnInCheckOptions {
        log_paths: vec![log_path],
        required_window_days: 30,
        min_pq_ratio: 0.95,
        max_no_provider_errors: 0,
        max_brownout_ratio: 0.05,
        min_fetches: 1,
    })
    .expect("burn-in check should succeed");
    assert_eq!(summary.fetches_total, 1);
    assert_eq!(summary.successful_fetches, 1);
    assert!(
        summary.coverage_days >= 35.0,
        "unexpected coverage: {} days",
        summary.coverage_days
    );
    assert!(
        (summary.pq_ratio_min - 0.98).abs() < f64::EPSILON,
        "pq ratio mismatch"
    );
    assert!(
        (summary.stall_event_ratio - 0.0).abs() < f64::EPSILON,
        "stall event ratio should be zero"
    );
    assert!(
        (summary.stall_chunk_ratio - 0.0).abs() < f64::EPSILON,
        "stall chunk ratio should be zero"
    );
}
#[test]
fn scoreboard_diff_detects_added_and_changed_weights() {
    let temp = tempdir().expect("tempdir");
    let previous = temp.path().join("prev.scoreboard.json");
    let current = temp.path().join("curr.scoreboard.json");
    write_scoreboard_with_weights(&previous, &[("alpha", 0.7), ("beta", 0.3)]);
    write_scoreboard_with_weights(&current, &[("alpha", 0.5), ("beta", 0.3), ("gamma", 0.2)]);
    let report = run_scoreboard_diff(ScoreboardDiffOptions {
        previous_scoreboard: previous,
        current_scoreboard: current,
        threshold_percent: 5.0,
    })
    .expect("diff report");
    assert_eq!(report.total_providers, 3);
    assert_eq!(report.changed_providers.len(), 2);
    let alpha = report
        .changed_providers
        .iter()
        .find(|entry| entry.provider_id == "alpha")
        .expect("alpha delta");
    assert!(!alpha.was_added);
    assert!(!alpha.was_removed);
    assert!(alpha.exceeds_threshold);
    let gamma = report
        .changed_providers
        .iter()
        .find(|entry| entry.provider_id == "gamma")
        .expect("gamma delta");
    assert!(gamma.was_added);
    assert!(!gamma.was_removed);
    assert!(gamma.exceeds_threshold);
}
#[test]
fn scoreboard_diff_marks_removed_providers() {
    let temp = tempdir().expect("tempdir");
    let previous = temp.path().join("prev.scoreboard.json");
    let current = temp.path().join("curr.scoreboard.json");
    write_scoreboard_with_weights(&previous, &[("alpha", 0.4), ("beta", 0.6)]);
    write_scoreboard_with_weights(&current, &[("beta", 1.0)]);
    let report = run_scoreboard_diff(ScoreboardDiffOptions {
        previous_scoreboard: previous,
        current_scoreboard: current,
        threshold_percent: 10.0,
    })
    .expect("diff report");
    assert_eq!(report.changed_providers.len(), 2);
    let alpha = report
        .changed_providers
        .iter()
        .find(|entry| entry.provider_id == "alpha")
        .expect("alpha removal");
    assert!(alpha.was_removed);
    assert!(!alpha.was_added);
    assert!(alpha.exceeds_threshold);
    let beta = report
        .changed_providers
        .iter()
        .find(|entry| entry.provider_id == "beta")
        .expect("beta delta");
    assert!(!beta.was_added);
    assert!(!beta.was_removed);
    assert!(beta.exceeds_threshold);
}
#[test]
fn burn_in_check_rejects_low_pq_ratio() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("burnin_low_ratio.log");
    fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"b\" region=\"lab\" job_id=\"job-low\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"b\" region=\"lab\" job_id=\"job-low\" anonymity_ratio=0.90 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
",
        )
        .expect("write log");
    let result = run_burn_in_check(BurnInCheckOptions {
        log_paths: vec![log_path],
        required_window_days: 30,
        min_pq_ratio: 0.95,
        max_no_provider_errors: 0,
        max_brownout_ratio: 0.05,
        min_fetches: 1,
    });
    assert!(result.is_err(), "expected pq ratio violation");
}
#[test]
fn burn_in_check_rejects_no_provider_errors() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("burnin_errors.log");
    fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"c\" region=\"lab\" job_id=\"job-err\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"c\" region=\"lab\" job_id=\"job-err\" anonymity_ratio=0.98 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
2026-02-06T00:00:00Z telemetry::sorafs.fetch.error reason=\"no_healthy_providers\" manifest=\"c\" region=\"lab\"
",
        )
        .expect("write log");
    let result = run_burn_in_check(BurnInCheckOptions {
        log_paths: vec![log_path],
        required_window_days: 30,
        min_pq_ratio: 0.95,
        max_no_provider_errors: 0,
        max_brownout_ratio: 0.05,
        min_fetches: 1,
    });
    assert!(
        result.is_err(),
        "expected failure when no_healthy_providers errors are present"
    );
}
#[test]
fn burn_in_check_rejects_brownout_ratio() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("burnin_brownout.log");
    fs::write(
            &log_path,
            "\
2026-01-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"start\" status=\"started\" manifest=\"d\" region=\"lab\" job_id=\"job-brownout\" anonymity_ratio=1 anonymity_outcome=\"met\" anonymity_reason=\"none\"
2026-01-15T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"d\" region=\"lab\" job_id=\"job-brownout\" anonymity_ratio=0.99 anonymity_outcome=\"brownout\" anonymity_reason=\"guard-deficit\" stall_count=0
2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"d\" region=\"lab\" job_id=\"job-brownout\" anonymity_ratio=0.99 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=0
",
        )
        .expect("write log");
    let result = run_burn_in_check(BurnInCheckOptions {
        log_paths: vec![log_path],
        required_window_days: 30,
        min_pq_ratio: 0.95,
        max_no_provider_errors: 0,
        max_brownout_ratio: 0.10,
        min_fetches: 1,
    });
    assert!(
        result.is_err(),
        "expected failure when brownout ratio exceeds threshold"
    );
}
#[test]
fn burn_in_check_reports_stall_ratios() {
    let temp = tempfile::tempdir().expect("tempdir");
    let log_path = temp.path().join("burnin_stalls.log");
    fs::write(
            &log_path,
            "\
2026-02-01T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"stall\" region=\"lab\" job_id=\"job-stall\" anonymity_ratio=0.97 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=4
2026-02-02T12:00:00Z telemetry::sorafs.fetch.stall manifest=\"stall\" region=\"lab\" job_id=\"job-stall\" reason=\"timeout\"
2026-02-03T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" manifest=\"stall\" region=\"lab\" job_id=\"job-stall\" anonymity_ratio=0.99 anonymity_outcome=\"met\" anonymity_reason=\"none\" stall_count=2
",
        )
        .expect("write log");
    let summary = run_burn_in_check(BurnInCheckOptions {
        log_paths: vec![log_path],
        required_window_days: 1,
        min_pq_ratio: 0.90,
        max_no_provider_errors: 0,
        max_brownout_ratio: 1.0,
        min_fetches: 1,
    })
    .expect("burn-in summary should succeed");
    assert_eq!(summary.fetches_total, 2);
    assert_eq!(summary.stall_events, 1);
    assert_eq!(summary.stall_chunks, 6);
    assert!(
        (summary.stall_event_ratio - 0.5).abs() < f64::EPSILON,
        "expected stall event ratio for one stall event across two fetches"
    );
    assert!(
        (summary.stall_chunk_ratio - 3.0).abs() < f64::EPSILON,
        "expected stall chunk ratio for six stalled chunks across two fetches"
    );
}
#[test]
fn telemetry_parser_extracts_fields() {
    let line = "2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" anonymity_ratio=0.98 stall_count=0";
    let parsed = super::parse_telemetry_line(line).expect("parser should accept lifecycle line");
    assert_eq!(parsed.target, "sorafs.fetch.lifecycle");
    assert_eq!(
        parsed.fields.get("event").map(String::as_str),
        Some("complete")
    );
    assert_eq!(
        parsed.fields.get("anonymity_ratio").map(String::as_str),
        Some("0.98")
    );
    assert_eq!(
        parsed.fields.get("stall_count").map(String::as_str),
        Some("0")
    );
}
#[test]
fn burn_in_accumulator_tracks_complete_events() {
    let line = "2026-02-05T00:00:00Z telemetry::sorafs.fetch.lifecycle event=\"complete\" status=\"success\" anonymity_ratio=0.98 stall_count=0";
    let event = super::parse_telemetry_line(line).expect("parser should accept lifecycle line");
    let mut accumulator = super::BurnInAccumulator::default();
    accumulator.observe(event);
    assert_eq!(accumulator.fetches_total, 1);
    assert_eq!(accumulator.successful_fetches, 1);
    assert_eq!(accumulator.pq_ratio_samples, 1);
    assert!(
        (accumulator.pq_ratio_min - 0.98).abs() < f64::EPSILON,
        "expected ratio to be recorded (min={}, sum={}, samples={})",
        accumulator.pq_ratio_min,
        accumulator.pq_ratio_sum,
        accumulator.pq_ratio_samples
    );
}

#[derive(Clone, Copy)]
struct AdoptionEntry {
    provider_id: &'static str,
    normalised_weight: f64,
    raw_score: Option<f64>,
}

const fn scored(id: &'static str, weight: f64, raw_score: f64) -> AdoptionEntry {
    AdoptionEntry {
        provider_id: id,
        normalised_weight: weight,
        raw_score: Some(raw_score),
    }
}

const fn weighted(id: &'static str, weight: f64) -> AdoptionEntry {
    AdoptionEntry {
        provider_id: id,
        normalised_weight: weight,
        raw_score: None,
    }
}

const AB_50_12_11: &[AdoptionEntry] = &[scored("alpha", 0.5, 1.2), scored("beta", 0.5, 1.1)];
const AB_50_10_10: &[AdoptionEntry] = &[scored("alpha", 0.5, 1.0), scored("beta", 0.5, 1.0)];
const AB_60_40_10_09: &[AdoptionEntry] = &[scored("alpha", 0.6, 1.0), scored("beta", 0.4, 0.9)];
const AB_60_40_13_11: &[AdoptionEntry] = &[scored("alpha", 0.6, 1.3), scored("beta", 0.4, 1.1)];
const AB_50: &[AdoptionEntry] = &[weighted("alpha", 0.5), weighted("beta", 0.5)];
const AB_60_40: &[AdoptionEntry] = &[weighted("alpha", 0.6), weighted("beta", 0.4)];
const GATEWAY_50: &[AdoptionEntry] = &[weighted("gateway-a", 0.5), weighted("gateway-b", 0.5)];
const GATEWAY_60_40: &[AdoptionEntry] = &[weighted("gateway-a", 0.6), weighted("gateway-b", 0.4)];
const AB_22: &[(&str, u64)] = &[("alpha", 2), ("beta", 2)];
const AB_11: &[(&str, u64)] = &[("alpha", 1), ("beta", 1)];

#[derive(Clone, Copy)]
enum MetadataBase {
    Empty,
    Direct,
    DirectOnly,
    Gateway,
    Mixed,
}

#[derive(Clone, Copy)]
enum MetadataField {
    ProviderCount,
    GatewayProviderCount,
    ProviderMix,
    TransportPolicy,
    TransportPolicyOverrideLabel,
    GatewayManifestId,
    GatewayManifestCid,
}

impl MetadataField {
    const fn key(self) -> &'static str {
        match self {
            Self::ProviderCount => "provider_count",
            Self::GatewayProviderCount => "gateway_provider_count",
            Self::ProviderMix => "provider_mix",
            Self::TransportPolicy => "transport_policy",
            Self::TransportPolicyOverrideLabel => "transport_policy_override_label",
            Self::GatewayManifestId => "gateway_manifest_id",
            Self::GatewayManifestCid => "gateway_manifest_cid",
        }
    }
}

#[derive(Clone, Copy)]
enum MetadataPatch {
    UseScoreboard(bool),
    AllowImplicitMetadata(bool),
    ProviderCount(u64),
    GatewayProviderCount(u64),
    ProviderMix(&'static str),
    MaxParallel(u64),
    MaxPeers(u64),
    TelemetrySource(&'static str),
    TelemetryRegion(&'static str),
    GatewayManifestProvided(bool),
    GatewayManifestId(&'static str),
    GatewayManifestCid(&'static str),
    TransportPolicy(&'static str),
    TransportPolicyOverride(bool),
    TransportPolicyOverrideLabel(Option<&'static str>),
    Version(&'static str),
    Remove(MetadataField),
}

#[derive(Clone, Copy)]
enum MetadataFixture {
    Missing,
    Null,
    Object(MetadataBase, &'static [MetadataPatch]),
}

const fn metadata(base: MetadataBase, patches: &'static [MetadataPatch]) -> MetadataFixture {
    MetadataFixture::Object(base, patches)
}

const DIRECT_METADATA: MetadataFixture = metadata(MetadataBase::Direct, &[]);
const DIRECT_ONLY_METADATA: MetadataFixture = metadata(MetadataBase::DirectOnly, &[]);
const GATEWAY_METADATA: MetadataFixture = metadata(MetadataBase::Gateway, &[]);
const MIXED_METADATA: MetadataFixture = metadata(MetadataBase::Mixed, &[]);
const DIRECT_FIXTURE_TELEMETRY: MetadataFixture = metadata(
    MetadataBase::Direct,
    &[MetadataPatch::TelemetrySource("file:///tmp/fixture.json")],
);
const DIRECT_CI_TELEMETRY: MetadataFixture = metadata(
    MetadataBase::Direct,
    &[MetadataPatch::TelemetrySource("otel::ci")],
);
const DIRECT_CI_IAD_TELEMETRY: MetadataFixture = metadata(
    MetadataBase::Direct,
    &[
        MetadataPatch::TelemetrySource("otel::ci"),
        MetadataPatch::TelemetryRegion("iad-prod"),
    ],
);

fn fixture_insert(root: &mut Value, key: &str, value: impl Into<Value>) {
    root.as_object_mut()
        .expect("fixture object")
        .insert(key.into(), value.into());
}

fn render_metadata(fixture: MetadataFixture) -> Option<Value> {
    let (base, patches) = match fixture {
        MetadataFixture::Missing => return None,
        MetadataFixture::Null => return Some(Value::Null),
        MetadataFixture::Object(base, patches) => (base, patches),
    };
    let mut value = match base {
        MetadataBase::Empty => norito::json!({}),
        MetadataBase::Direct => norito::json!({
            "use_scoreboard": true,
            "provider_count": 2u64,
            "gateway_provider_count": 0u64,
            "provider_mix": "direct-only",
            "transport_policy": "soranet-first",
            "transport_policy_override": false,
            "transport_policy_override_label": null,
        }),
        MetadataBase::DirectOnly => norito::json!({
            "use_scoreboard": true,
            "provider_count": 2u64,
            "gateway_provider_count": 0u64,
            "provider_mix": "direct-only",
            "transport_policy": "direct-only",
            "transport_policy_override": true,
            "transport_policy_override_label": "direct-only",
        }),
        MetadataBase::Gateway => norito::json!({
            "provider_count": 0u64,
            "gateway_provider_count": 2u64,
            "gateway_manifest_id": TEST_MANIFEST_ID,
            "gateway_manifest_cid": TEST_MANIFEST_CID,
            "gateway_manifest_provided": true,
            "use_scoreboard": true,
            "provider_mix": "gateway-only",
            "transport_policy": "soranet-first",
            "transport_policy_override": false,
            "transport_policy_override_label": null,
        }),
        MetadataBase::Mixed => norito::json!({
            "use_scoreboard": true,
            "telemetry_source": "file:///tmp/fixture.json",
            "provider_count": 1u64,
            "gateway_provider_count": 1u64,
            "provider_mix": "mixed",
            "transport_policy": "soranet-first",
            "transport_policy_override": false,
            "transport_policy_override_label": null,
            "gateway_manifest_id": "feedface",
            "gateway_manifest_cid": "c0ffee",
            "gateway_manifest_provided": true,
        }),
    };
    for &patch in patches {
        match patch {
            MetadataPatch::UseScoreboard(v) => fixture_insert(&mut value, "use_scoreboard", v),
            MetadataPatch::AllowImplicitMetadata(v) => {
                fixture_insert(&mut value, "allow_implicit_metadata", v)
            }
            MetadataPatch::ProviderCount(v) => fixture_insert(&mut value, "provider_count", v),
            MetadataPatch::GatewayProviderCount(v) => {
                fixture_insert(&mut value, "gateway_provider_count", v)
            }
            MetadataPatch::ProviderMix(v) => fixture_insert(&mut value, "provider_mix", v),
            MetadataPatch::MaxParallel(v) => fixture_insert(&mut value, "max_parallel", v),
            MetadataPatch::MaxPeers(v) => fixture_insert(&mut value, "max_peers", v),
            MetadataPatch::TelemetrySource(v) => fixture_insert(&mut value, "telemetry_source", v),
            MetadataPatch::TelemetryRegion(v) => fixture_insert(&mut value, "telemetry_region", v),
            MetadataPatch::GatewayManifestProvided(v) => {
                fixture_insert(&mut value, "gateway_manifest_provided", v)
            }
            MetadataPatch::GatewayManifestId(v) => {
                fixture_insert(&mut value, "gateway_manifest_id", v)
            }
            MetadataPatch::GatewayManifestCid(v) => {
                fixture_insert(&mut value, "gateway_manifest_cid", v)
            }
            MetadataPatch::TransportPolicy(v) => fixture_insert(&mut value, "transport_policy", v),
            MetadataPatch::TransportPolicyOverride(v) => {
                fixture_insert(&mut value, "transport_policy_override", v)
            }
            MetadataPatch::TransportPolicyOverrideLabel(v) => fixture_insert(
                &mut value,
                "transport_policy_override_label",
                v.map_or(Value::Null, Value::from),
            ),
            MetadataPatch::Version(v) => fixture_insert(&mut value, "version", v),
            MetadataPatch::Remove(field) => {
                value
                    .as_object_mut()
                    .expect("metadata fixture object")
                    .remove(field.key());
            }
        }
    }
    Some(value)
}

#[derive(Clone, Copy)]
enum SummaryPatch {
    ProviderCount(u64),
    GatewayProviderCount(u64),
    ProviderMixForCounts(u64, u64),
    RemoveProviderMix,
    GatewayManifestProvided(bool),
    RemoveManifestId,
    ManifestCid(&'static str),
    TransportPolicy(&'static str),
    TransportPolicyOverride(bool),
    TransportPolicyOverrideLabel(&'static str),
    TelemetrySource(&'static str),
    TelemetryRegion(&'static str),
    ChunkCount(u64),
    FirstReceiptProvider(&'static str),
    FirstReportProvider(&'static str),
}

const DIRECT_ONLY_SUMMARY: &[SummaryPatch] = &[
    SummaryPatch::TransportPolicy("direct-only"),
    SummaryPatch::TransportPolicyOverride(true),
    SummaryPatch::TransportPolicyOverrideLabel("direct-only"),
];
const CI_SUMMARY: &[SummaryPatch] = &[SummaryPatch::TelemetrySource("otel::ci")];
const FILE_SUMMARY: &[SummaryPatch] = &[SummaryPatch::TelemetrySource("file:/tmp/telemetry.json")];

fn render_summary(providers: &[(&str, u64)], patches: &[SummaryPatch]) -> Value {
    let mut value = summary_with_providers_value(providers);
    for &patch in patches {
        match patch {
            SummaryPatch::ProviderCount(v) => fixture_insert(&mut value, "provider_count", v),
            SummaryPatch::GatewayProviderCount(v) => {
                fixture_insert(&mut value, "gateway_provider_count", v)
            }
            SummaryPatch::ProviderMixForCounts(direct, gateway) => fixture_insert(
                &mut value,
                "provider_mix",
                provider_mix_label_from_counts(direct, gateway),
            ),
            SummaryPatch::RemoveProviderMix => {
                value
                    .as_object_mut()
                    .expect("summary object")
                    .remove("provider_mix");
            }
            SummaryPatch::GatewayManifestProvided(v) => {
                fixture_insert(&mut value, "gateway_manifest_provided", v)
            }
            SummaryPatch::RemoveManifestId => {
                value
                    .as_object_mut()
                    .expect("summary object")
                    .remove("manifest_id");
            }
            SummaryPatch::ManifestCid(v) => fixture_insert(&mut value, "manifest_cid", v),
            SummaryPatch::TransportPolicy(v) => fixture_insert(&mut value, "transport_policy", v),
            SummaryPatch::TransportPolicyOverride(v) => {
                fixture_insert(&mut value, "transport_policy_override", v)
            }
            SummaryPatch::TransportPolicyOverrideLabel(v) => {
                fixture_insert(&mut value, "transport_policy_override_label", v)
            }
            SummaryPatch::TelemetrySource(v) => fixture_insert(&mut value, "telemetry_source", v),
            SummaryPatch::TelemetryRegion(v) => fixture_insert(&mut value, "telemetry_region", v),
            SummaryPatch::ChunkCount(v) => fixture_insert(&mut value, "chunk_count", v),
            SummaryPatch::FirstReceiptProvider(provider) => {
                if let Some(object) = value
                    .get_mut("chunk_receipts")
                    .and_then(Value::as_array_mut)
                    .and_then(|receipts| receipts.first_mut())
                    .and_then(Value::as_object_mut)
                {
                    object.insert("provider".into(), Value::from(provider));
                }
            }
            SummaryPatch::FirstReportProvider(provider) => {
                if let Some(object) = value
                    .get_mut("provider_reports")
                    .and_then(Value::as_array_mut)
                    .and_then(|reports| reports.first_mut())
                    .and_then(Value::as_object_mut)
                {
                    object.insert("provider".into(), Value::from(provider));
                }
            }
        }
    }
    value
}

#[derive(Clone, Copy)]
enum AdoptionGate {
    StrictOne,
    StrictTwo,
    AllowSingleSource,
    AllowSingleSourceDirectOnly,
    RequireTelemetryOne,
    RequireTelemetryTwo,
    AllowImplicitMetadata,
    RequireDirectOnly,
    RequireTelemetryRegion,
    AllowZeroWeight,
}

#[derive(Clone, Copy)]
enum AdoptionExpected {
    Success,
    TotalOne,
    MultiProvider,
    Gateway,
    SingleSourceOverride,
    ImplicitMetadataOverride,
    Error,
    ErrorContains(&'static str),
}

struct AdoptionCase {
    entries: &'static [AdoptionEntry],
    metadata: MetadataFixture,
    providers: &'static [(&'static str, u64)],
    summary_patches: &'static [SummaryPatch],
    gate: AdoptionGate,
    expected: AdoptionExpected,
}

const fn adoption_case(
    entries: &'static [AdoptionEntry],
    metadata: MetadataFixture,
    providers: &'static [(&'static str, u64)],
    summary_patches: &'static [SummaryPatch],
    gate: AdoptionGate,
    expected: AdoptionExpected,
) -> AdoptionCase {
    AdoptionCase {
        entries,
        metadata,
        providers,
        summary_patches,
        gate,
        expected,
    }
}

fn run_adoption_case(name: &str, case: &AdoptionCase) {
    let temp = tempfile::tempdir().expect("tempdir");
    let scoreboard_path = temp.path().join(format!("{name}.scoreboard.json"));
    let summary_path = temp.path().join(format!("{name}.summary.json"));
    let entries: Vec<Value> = case
        .entries
        .iter()
        .map(|entry| {
            let provider_id = entry.provider_id;
            let normalised_weight = entry.normalised_weight;
            let mut value = norito::json!({
                "provider_id": provider_id,
                "normalised_weight": normalised_weight,
                "eligibility": "eligible",
            });
            if let Some(raw_score) = entry.raw_score {
                fixture_insert(&mut value, "raw_score", raw_score);
            }
            value
        })
        .collect();
    let mut scoreboard = norito::json!({ "entries": entries });
    if let Some(metadata) = render_metadata(case.metadata) {
        fixture_insert(&mut scoreboard, "metadata", metadata);
    }
    fs::write(
        &scoreboard_path,
        to_string_pretty(&scoreboard).expect("render scoreboard"),
    )
    .expect("write scoreboard");
    let summary = render_summary(case.providers, case.summary_patches);
    fs::write(
        &summary_path,
        to_string_pretty(&summary).expect("render summary"),
    )
    .expect("write summary");

    let (minimum, positive_weight, single_source, telemetry, implicit, direct_only, region) =
        match case.gate {
            AdoptionGate::StrictOne => (1, true, false, false, false, false, false),
            AdoptionGate::StrictTwo => (2, true, false, false, false, false, false),
            AdoptionGate::AllowSingleSource => (2, true, true, false, false, false, false),
            AdoptionGate::AllowSingleSourceDirectOnly => (2, true, true, false, false, true, false),
            AdoptionGate::RequireTelemetryOne => (1, true, false, true, false, false, false),
            AdoptionGate::RequireTelemetryTwo => (2, true, false, true, false, false, false),
            AdoptionGate::AllowImplicitMetadata => (2, true, false, false, true, false, false),
            AdoptionGate::RequireDirectOnly => (2, true, false, false, false, true, false),
            AdoptionGate::RequireTelemetryRegion => (2, true, false, false, false, false, true),
            AdoptionGate::AllowZeroWeight => (2, false, false, false, false, false, false),
        };
    let result = run_adoption_check(AdoptionCheckOptions {
        scoreboard_paths: vec![scoreboard_path],
        summary_paths: vec![summary_path],
        min_eligible_providers: minimum,
        require_positive_weight: positive_weight,
        allow_single_source_fallback: single_source,
        require_telemetry_source: telemetry,
        allow_implicit_metadata: implicit,
        require_direct_only: direct_only,
        require_telemetry_region: region,
    });
    match case.expected {
        AdoptionExpected::Success => {
            result.unwrap_or_else(|error| panic!("{name} should pass: {error}"));
        }
        AdoptionExpected::TotalOne => {
            let report = result.unwrap_or_else(|error| panic!("{name} should pass: {error}"));
            assert_eq!(report.total_evaluated, 1);
        }
        AdoptionExpected::MultiProvider => {
            let report = result.unwrap_or_else(|error| panic!("{name} should pass: {error}"));
            assert_eq!(report.total_evaluated, 1);
            assert_eq!(
                report
                    .scoreboard_reports
                    .first()
                    .expect("report entry")
                    .eligible_providers,
                2,
            );
        }
        AdoptionExpected::Gateway => {
            let report = result.unwrap_or_else(|error| panic!("{name} should pass: {error}"));
            assert_eq!(report.total_evaluated, 1);
            assert!(!report.single_source_override_used);
        }
        AdoptionExpected::SingleSourceOverride => {
            let report = result.unwrap_or_else(|error| panic!("{name} should pass: {error}"));
            assert!(report.single_source_override_used);
        }
        AdoptionExpected::ImplicitMetadataOverride => {
            let report = result.unwrap_or_else(|error| panic!("{name} should pass: {error}"));
            assert!(report.implicit_metadata_override_used);
        }
        AdoptionExpected::Error => assert!(result.is_err(), "{name} should fail"),
        AdoptionExpected::ErrorContains(needle) => {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(needle),
                "{name} error should contain {needle:?}: {error}",
            );
        }
    }
}

use AdoptionExpected as Expected;
use AdoptionGate as Gate;
use MetadataBase as Base;
use MetadataField as MetaField;
use MetadataPatch as Meta;
use SummaryPatch as Summary;

macro_rules! adoption_cases {
        ($($name:ident => $case:expr),+ $(,)?) => {
            const ADOPTION_CASES: &[(&str, AdoptionCase)] = &[
                $((stringify!($name), $case),)+
            ];
            const _: [(); 55] = [(); ADOPTION_CASES.len()];

            $(
                #[test]
                fn $name() {
                    let mut matches = ADOPTION_CASES
                        .iter()
                        .filter(|(candidate, _)| *candidate == stringify!($name));
                    let item = matches.next().expect("adoption fixture must exist");
                    assert!(matches.next().is_none(), "adoption fixture names must be unique");
                    run_adoption_case(item.0, &item.1);
                }
            )+
        };
    }

adoption_cases! {
    adoption_check_accepts_multi_provider_scoreboard => adoption_case(
        &[scored("fixture-a", 0.6, 1.2), scored("fixture-b", 0.4, 1.0)],
        DIRECT_METADATA, &[("fixture-a", 2), ("fixture-b", 3)], &[],
        Gate::StrictTwo, Expected::MultiProvider),
    adoption_check_rejects_missing_metadata => adoption_case(
        AB_50_12_11, MetadataFixture::Missing, AB_22, &[],
        Gate::StrictTwo, Expected::Error),
    adoption_check_rejects_null_metadata => adoption_case(
        &[scored("solo", 1.0, 1.0)], MetadataFixture::Null, &[("solo", 1)], &[],
        Gate::StrictOne, Expected::Error),
    adoption_check_rejects_missing_provider_totals => adoption_case(
        &[scored("alpha", 0.6, 1.2), scored("beta", 0.4, 1.1)],
        metadata(Base::Direct, &[
            Meta::Remove(MetaField::ProviderCount),
            Meta::Remove(MetaField::GatewayProviderCount),
            Meta::TelemetrySource("file:///tmp/missing_counts.json"),
        ]),
        &[("alpha", 2), ("beta", 1)], &[], Gate::StrictTwo,
        Expected::ErrorContains("provider_count")),
    adoption_check_rejects_missing_provider_mix_metadata_without_counts => adoption_case(
        &[scored("alpha", 0.7, 1.2), scored("beta", 0.3, 1.1)],
        metadata(Base::Direct, &[
            Meta::Remove(MetaField::ProviderCount),
            Meta::Remove(MetaField::GatewayProviderCount),
            Meta::Remove(MetaField::ProviderMix),
            Meta::TelemetrySource("file:///tmp/missing_mix.json"),
        ]),
        AB_22, &[], Gate::StrictTwo, Expected::ErrorContains("provider_count")),
    adoption_check_rejects_provider_count_mismatch => adoption_case(
        AB_50_12_11,
        metadata(Base::Direct, &[
            Meta::TelemetrySource("file:///tmp/fixture.json"),
            Meta::ProviderCount(3),
        ]),
        AB_22, &[], Gate::StrictTwo, Expected::Error),
    adoption_check_rejects_summary_provider_count_mismatch => adoption_case(
        AB_50_12_11, DIRECT_FIXTURE_TELEMETRY, AB_22,
        &[Summary::ProviderCount(1)], Gate::StrictTwo,
        Expected::ErrorContains("provider_count")),
    adoption_check_rejects_summary_gateway_count_mismatch => adoption_case(
        &[scored("alpha", 0.5, 1.2), scored("gateway-a", 0.5, 1.1)],
        MIXED_METADATA, &[("alpha", 1), ("gateway-a", 1)],
        &[Summary::GatewayProviderCount(2), Summary::ProviderMixForCounts(1, 2)],
        Gate::StrictTwo, Expected::ErrorContains("gateway_provider_count")),
    adoption_check_rejects_missing_summary_provider_mix => adoption_case(
        AB_50, DIRECT_FIXTURE_TELEMETRY, AB_22, &[Summary::RemoveProviderMix],
        Gate::StrictTwo, Expected::ErrorContains("provider_mix")),
    adoption_check_rejects_gateway_runs_without_metadata => adoption_case(
        &[weighted("gateway-a", 1.0)], MetadataFixture::Missing, &[("gateway-a", 2)], &[],
        Gate::StrictOne, Expected::ErrorContains("metadata")),
    adoption_check_accepts_matching_provider_counts => adoption_case(
        AB_50, DIRECT_FIXTURE_TELEMETRY, AB_22, &[],
        Gate::StrictTwo, Expected::TotalOne),
    adoption_check_rejects_missing_provider_mix_metadata => adoption_case(
        AB_50,
        metadata(Base::Direct, &[
            Meta::TelemetrySource("file:///tmp/missing_mix.json"),
            Meta::ProviderCount(1),
            Meta::GatewayProviderCount(1),
            Meta::Remove(MetaField::ProviderMix),
        ]),
        AB_22, &[], Gate::StrictTwo, Expected::ErrorContains("provider_mix")),
    adoption_check_rejects_metadata_missing_transport_policy => adoption_case(
        AB_60_40, metadata(Base::Direct, &[Meta::Remove(MetaField::TransportPolicy)]),
        AB_22, &[], Gate::StrictTwo, Expected::ErrorContains("transport_policy")),
    adoption_check_rejects_metadata_transport_policy_mismatch => adoption_case(
        AB_50, metadata(Base::Direct, &[Meta::TransportPolicy("soranet-strict")]),
        AB_22, &[], Gate::StrictTwo, Expected::ErrorContains("metadata.transport_policy")),
    adoption_check_rejects_metadata_transport_override_mismatch => adoption_case(
        AB_60_40,
        metadata(Base::Direct, &[
            Meta::TransportPolicyOverride(true),
            Meta::TransportPolicyOverrideLabel(Some("soranet-strict")),
        ]),
        AB_22, &[], Gate::StrictTwo, Expected::ErrorContains("transport_policy_override")),
    adoption_check_rejects_metadata_missing_override_label => adoption_case(
        AB_60_40,
        metadata(Base::Direct, &[
            Meta::TransportPolicyOverride(true),
            Meta::Remove(MetaField::TransportPolicyOverrideLabel),
        ]),
        AB_22,
        &[
            Summary::TransportPolicyOverride(true),
            Summary::TransportPolicyOverrideLabel("direct-only"),
        ],
        Gate::StrictTwo, Expected::ErrorContains("transport_policy_override_label")),
    adoption_check_rejects_provider_mix_mismatch => adoption_case(
        AB_50,
        metadata(Base::Direct, &[
            Meta::TelemetrySource("file:///tmp/mix_mismatch.json"),
            Meta::ProviderMix("mixed"),
        ]),
        AB_22, &[], Gate::StrictTwo, Expected::ErrorContains("metadata.provider_mix")),
    adoption_check_rejects_gateway_runs_without_manifest_metadata => adoption_case(
        GATEWAY_50,
        metadata(Base::Gateway, &[
            Meta::GatewayManifestProvided(false),
            Meta::Remove(MetaField::GatewayManifestId),
            Meta::Remove(MetaField::GatewayManifestCid),
        ]),
        &[("gateway-a", 2), ("gateway-b", 2)], &[],
        Gate::StrictTwo, Expected::ErrorContains("gateway provider(s)")),
    adoption_check_rejects_gateway_manifest_metadata_without_gateways => adoption_case(
        &[weighted("direct-a", 0.6), weighted("direct-b", 0.4)],
        metadata(Base::Direct, &[
            Meta::GatewayManifestProvided(true),
            Meta::GatewayManifestId(TEST_MANIFEST_ID),
            Meta::GatewayManifestCid(TEST_MANIFEST_CID),
        ]),
        &[("direct-a", 2), ("direct-b", 3)], &[],
        Gate::StrictTwo, Expected::ErrorContains("no gateway providers")),
    adoption_check_accepts_gateway_runs_with_manifest_metadata => adoption_case(
        &[weighted("gateway-a", 0.55), weighted("gateway-b", 0.45)],
        GATEWAY_METADATA, &[("gateway-a", 3), ("gateway-b", 2)], &[],
        Gate::StrictTwo, Expected::Gateway),
    adoption_check_rejects_gateway_runs_without_manifest_identifiers => adoption_case(
        GATEWAY_50,
        metadata(Base::Gateway, &[
            Meta::Remove(MetaField::GatewayManifestId),
            Meta::Remove(MetaField::GatewayManifestCid),
        ]),
        &[("gateway-a", 1), ("gateway-b", 1)], &[],
        Gate::StrictTwo, Expected::ErrorContains("gateway_manifest_id")),
    adoption_check_rejects_summary_manifest_flag_without_metadata => adoption_case(
        &[weighted("direct-a", 0.6), weighted("direct-b", 0.4)],
        metadata(Base::Direct, &[Meta::GatewayManifestProvided(false)]),
        &[("direct-a", 2), ("direct-b", 1)],
        &[Summary::GatewayManifestProvided(true)],
        Gate::StrictTwo, Expected::ErrorContains("gateway_manifest_provided")),
    adoption_check_rejects_gateway_runs_with_summary_manifest_gap => adoption_case(
        GATEWAY_60_40, GATEWAY_METADATA, &[("gateway-a", 2), ("gateway-b", 1)],
        &[Summary::RemoveManifestId], Gate::StrictTwo,
        Expected::ErrorContains("manifest_id")),
    adoption_check_rejects_gateway_runs_with_manifest_mismatch => adoption_case(
        GATEWAY_60_40, GATEWAY_METADATA, &[("gateway-a", 2), ("gateway-b", 2)],
        &[Summary::ManifestCid("different-cid")], Gate::StrictTwo,
        Expected::ErrorContains("manifest_cid")),
    adoption_check_rejects_single_provider_scoreboard => adoption_case(
        &[scored("solo", 1.0, 1.1)],
        metadata(Base::Direct, &[Meta::ProviderCount(1)]),
        &[("solo", 5), ("backup", 1)], &[], Gate::StrictTwo, Expected::Error),
    adoption_check_rejects_metadata_single_source_max_parallel => adoption_case(
        AB_50_10_10, metadata(Base::Direct, &[Meta::MaxParallel(1)]), AB_22, &[],
        Gate::StrictTwo, Expected::Error),
    adoption_check_allows_metadata_single_source_with_override => adoption_case(
        AB_50_10_10, metadata(Base::DirectOnly, &[Meta::MaxParallel(1)]), AB_22,
        DIRECT_ONLY_SUMMARY, Gate::AllowSingleSource, Expected::SingleSourceOverride),
    adoption_check_rejects_direct_only_transport_policy_without_override => adoption_case(
        AB_60_40_10_09, DIRECT_ONLY_METADATA, AB_22, &[],
        Gate::StrictTwo, Expected::Error),
    adoption_check_reports_override_for_direct_only_transport_policy => adoption_case(
        AB_60_40_10_09, DIRECT_ONLY_METADATA, AB_22, DIRECT_ONLY_SUMMARY,
        Gate::AllowSingleSource, Expected::SingleSourceOverride),
    adoption_check_requires_direct_only_when_flag_is_set => adoption_case(
        AB_50_10_10, DIRECT_METADATA, AB_22, &[],
        Gate::RequireDirectOnly, Expected::Error),
    adoption_check_accepts_direct_only_when_required_flag_is_set => adoption_case(
        &[scored("alpha", 0.55, 1.0), scored("beta", 0.45, 0.9)],
        DIRECT_ONLY_METADATA, AB_22, DIRECT_ONLY_SUMMARY,
        Gate::AllowSingleSourceDirectOnly, Expected::SingleSourceOverride),
    adoption_check_rejects_direct_only_summary_transport_without_override => adoption_case(
        AB_50_10_10,
        metadata(Base::Empty, &[
            Meta::UseScoreboard(true),
            Meta::TransportPolicy("soranet-first"),
            Meta::TransportPolicyOverride(false),
            Meta::TransportPolicyOverrideLabel(None),
        ]),
        AB_22,
        &[
            Summary::TransportPolicy("direct-only"),
            Summary::TransportPolicyOverride(true),
            Summary::TransportPolicyOverrideLabel("direct-only"),
            Summary::TransportPolicyOverride(true),
            Summary::TransportPolicyOverrideLabel("direct-only"),
        ],
        Gate::StrictTwo, Expected::Error),
    adoption_check_reports_override_for_direct_only_summary_transport => adoption_case(
        AB_50_10_10, DIRECT_ONLY_METADATA, AB_22, DIRECT_ONLY_SUMMARY,
        Gate::AllowSingleSource, Expected::SingleSourceOverride),
    adoption_check_allows_single_provider_with_override => adoption_case(
        &[scored("solo", 1.0, 1.1)],
        metadata(Base::Direct, &[Meta::ProviderCount(1)]),
        &[("solo", 4)], &[], Gate::AllowSingleSource, Expected::SingleSourceOverride),
    adoption_check_rejects_scoreboard_metadata_opt_out => adoption_case(
        &[scored("fixture-provider", 1.0, 1.0)],
        metadata(Base::Empty, &[
            Meta::Version("test"),
            Meta::UseScoreboard(false),
            Meta::TransportPolicy("soranet-first"),
            Meta::TransportPolicyOverride(false),
            Meta::TransportPolicyOverrideLabel(None),
        ]),
        &[("fixture-provider", 2)], &[], Gate::StrictOne, Expected::Error),
    adoption_check_rejects_single_source_metadata_without_override => adoption_case(
        AB_50_10_10,
        metadata(Base::Empty, &[
            Meta::MaxPeers(1),
            Meta::GatewayProviderCount(2),
            Meta::TransportPolicy("soranet-first"),
            Meta::TransportPolicyOverride(false),
            Meta::TransportPolicyOverrideLabel(None),
        ]),
        &[("alpha", 2), ("beta", 3)], &[], Gate::StrictTwo, Expected::Error),
    adoption_check_reports_override_for_single_source_metadata => adoption_case(
        AB_50_10_10, metadata(Base::DirectOnly, &[Meta::MaxParallel(1)]), AB_22,
        DIRECT_ONLY_SUMMARY, Gate::AllowSingleSource, Expected::SingleSourceOverride),
    adoption_check_rejects_implicit_metadata_without_override => adoption_case(
        AB_50_10_10, metadata(Base::Direct, &[Meta::AllowImplicitMetadata(true)]),
        AB_11, &[], Gate::StrictTwo, Expected::Error),
    adoption_check_reports_override_for_implicit_metadata => adoption_case(
        AB_50_10_10, metadata(Base::Direct, &[Meta::AllowImplicitMetadata(true)]),
        AB_11, &[], Gate::AllowImplicitMetadata, Expected::ImplicitMetadataOverride),
    adoption_check_requires_telemetry_source_when_requested => adoption_case(
        AB_60_40_13_11,
        metadata(Base::Empty, &[
            Meta::MaxParallel(4),
            Meta::UseScoreboard(true),
            Meta::TransportPolicy("soranet-first"),
            Meta::TransportPolicyOverride(false),
            Meta::TransportPolicyOverrideLabel(None),
        ]),
        &[("alpha", 3), ("beta", 2)], FILE_SUMMARY,
        Gate::RequireTelemetryTwo, Expected::Error),
    adoption_check_accepts_present_telemetry_source => adoption_case(
        AB_60_40_13_11,
        metadata(Base::Direct, &[
            Meta::MaxParallel(4),
            Meta::TelemetrySource("file:/tmp/telemetry.json"),
        ]),
        &[("alpha", 3), ("beta", 2)], FILE_SUMMARY,
        Gate::RequireTelemetryTwo, Expected::Success),
    adoption_check_requires_summary_telemetry_label => adoption_case(
        AB_60_40, DIRECT_CI_TELEMETRY, AB_22, &[],
        Gate::RequireTelemetryOne, Expected::ErrorContains("missing `telemetry_source`")),
    adoption_check_rejects_mismatched_summary_telemetry => adoption_case(
        AB_60_40,
        metadata(Base::Direct, &[Meta::TelemetrySource("otel::primary")]),
        AB_22, &[Summary::TelemetrySource("otel::other")],
        Gate::StrictTwo, Expected::ErrorContains("telemetry_source=`otel::other`")),
    adoption_check_requires_summary_telemetry_region_when_metadata_present => adoption_case(
        AB_60_40, DIRECT_CI_IAD_TELEMETRY, AB_22, CI_SUMMARY,
        Gate::StrictTwo, Expected::ErrorContains("telemetry_region")),
    adoption_check_rejects_mismatched_telemetry_region => adoption_case(
        AB_60_40, DIRECT_CI_IAD_TELEMETRY, &[("alpha", 3), ("beta", 1)],
        &[
            Summary::TelemetrySource("otel::ci"),
            Summary::TelemetryRegion("sea-primary"),
        ],
        Gate::StrictTwo, Expected::ErrorContains("telemetry_region=`sea-primary`")),
    adoption_check_accepts_matching_telemetry_region => adoption_case(
        AB_60_40, DIRECT_CI_IAD_TELEMETRY, AB_22,
        &[
            Summary::TelemetrySource("otel::ci"),
            Summary::TelemetryRegion("iad-prod"),
        ],
        Gate::StrictTwo, Expected::Success),
    adoption_check_allows_missing_telemetry_region_without_flag => adoption_case(
        AB_50, DIRECT_CI_TELEMETRY, AB_22, CI_SUMMARY,
        Gate::StrictTwo, Expected::Success),
    adoption_check_requires_telemetry_region_when_requested => adoption_case(
        AB_50, DIRECT_CI_TELEMETRY, AB_22, CI_SUMMARY,
        Gate::RequireTelemetryRegion, Expected::ErrorContains("telemetry_region")),
    adoption_check_rejects_zero_weight_provider_when_required => adoption_case(
        &[scored("zero-weight", 0.0, 1.0), scored("healthy", 1.0, 1.4)],
        MetadataFixture::Missing, &[("zero-weight", 1), ("healthy", 3)], &[],
        Gate::StrictTwo, Expected::Error),
    adoption_check_allows_zero_weight_when_override_enabled => adoption_case(
        &[scored("zero-weight", 0.0, 1.0), scored("backup", 1.0, 1.3)],
        DIRECT_METADATA, &[("zero-weight", 1), ("backup", 2)], &[],
        Gate::AllowZeroWeight, Expected::Success),
    adoption_check_rejects_weight_sum_mismatch => adoption_case(
        &[scored("alpha", 0.4, 1.0), scored("beta", 0.4, 0.9)],
        MetadataFixture::Missing, AB_22, &[], Gate::StrictTwo, Expected::Error),
    adoption_check_rejects_single_active_provider_in_summary => adoption_case(
        AB_50_12_11, MetadataFixture::Missing, &[("alpha", 5), ("beta", 0)], &[],
        Gate::StrictTwo, Expected::Error),
    adoption_check_allows_single_active_provider_in_summary_with_override => adoption_case(
        &[scored("alpha", 0.6, 1.3), scored("beta", 0.4, 1.0)],
        DIRECT_METADATA, &[("alpha", 4), ("beta", 0)], &[],
        Gate::AllowSingleSource, Expected::Success),
    adoption_check_rejects_chunk_count_mismatch => adoption_case(
        &[scored("alpha", 0.5, 1.1), scored("beta", 0.5, 1.0)],
        MetadataFixture::Missing, AB_11, &[Summary::ChunkCount(999)],
        Gate::StrictTwo, Expected::Error),
    adoption_check_rejects_unknown_provider_in_summary => adoption_case(
        &[scored("alpha", 0.6, 1.25), scored("beta", 0.4, 0.95)],
        MetadataFixture::Missing, AB_11,
        &[
            Summary::FirstReceiptProvider("intruder"),
            Summary::FirstReportProvider("intruder"),
        ],
        Gate::StrictTwo, Expected::Error),
}
