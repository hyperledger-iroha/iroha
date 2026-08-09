"""Static source contract for the SoraFS hedging/billing freshness fence."""

from __future__ import annotations

import re


def assert_shipped_hedging_billing_freshness_contract(
    *, service_source: str, runtime_source: str
) -> None:
    """Pin live-head readiness and ACK pre-commit fencing in shipped sources."""
    daemon_status_struct = service_source[
        service_source.index("pub struct HedgingBillingDaemonStatusV1") :
        service_source.index("pub struct HedgingBillingDaemonMetricsV1")
    ]
    for required_field in (
        "pub anchor: HedgingBillingProjectionAnchorV1",
        "pub last_tick_fresh: bool",
        "pub finalized_projection_ready: bool",
        "pub ready: bool",
    ):
        assert required_field in daemon_status_struct

    runtime_status = runtime_source[
        runtime_source.index("pub fn status(") : runtime_source.index("pub fn metrics(")
    ]
    normalized_runtime_status = re.sub(r"\s+", " ", runtime_status)
    for readiness_guard in (
        "last_tick_fresh",
        "finalized_projection_ready",
        "&& last_tick_fresh",
        "&& finalized_projection_ready",
    ):
        assert readiness_guard in normalized_runtime_status
    assert "READINESS_STALE_TICK_MULTIPLIER_V1" in runtime_source
    assert "*freshness_guard = Some(Instant::now());" in runtime_source

    projection_freshness_start = runtime_source.index("fn projection_is_fresh_at_head(")
    projection_freshness = runtime_source[
        projection_freshness_start : runtime_source.index(
            "fn reconcile_once(", projection_freshness_start
        )
    ]
    normalized_projection_freshness = re.sub(r"\s+", " ", projection_freshness)
    for readiness_guard in (
        "cursor.height != 0",
        "cursor.height == service_finalized_height",
        "projection_at_or_before_head(Some(cursor), finalized_head)",
        "finalized_head.height.saturating_sub(cursor.height) <= max_finalized_lag_blocks",
    ):
        assert readiness_guard in normalized_projection_freshness

    runtime_api = runtime_source[
        runtime_source.index(
            "impl HedgingBillingRuntimeApiV1 for HedgingBillingRuntimeHandleV1"
        ) : runtime_source.index(
            "/// Assemble and start the committed hedging/billing runtime."
        )
    ]
    assert runtime_api.count("self.with_fresh_projection(") == 7
    freshness_fence = runtime_source[
        runtime_source.index("fn require_fresh_projection(") :
        runtime_source.index("fn verify_projection_at_or_before_head(")
    ]
    for required_fence in (
        "probe_finalized_head_for_api()?",
        "last_successful_tick_is_fresh()",
        "projection_is_fresh_at_head(",
        "!= expected_head",
    ):
        assert required_fence in freshness_fence

    acknowledgement_api = service_source[
        service_source.index("pub fn api_acknowledge_statement(") :
        service_source.index("pub fn api_exposure_page(")
    ]
    assert "api_acknowledge_statement_with_precommit_fence" in acknowledgement_api
    assert service_source.count("pre_commit_fence()?") == 2
    acknowledgement_mutators = service_source[
        service_source.index("fn acknowledge_statement_at_fingerprint_with_precommit_fence(") :
        service_source.index("pub fn api_projection_anchor(")
    ]
    assert acknowledgement_mutators.count(
        "self.commit_locked_with_precommit_fence("
    ) == 2
    fenced_commit = service_source[
        service_source.index("fn commit_locked_with_precommit_fence(") :
        service_source.index("fn reconcile_authoritative_delivery_state(")
    ]
    assert fenced_commit.index("pre_commit_fence()?") < fenced_commit.index(
        "self.store.commit_bytes("
    )
