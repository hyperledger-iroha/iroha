"""Tests for scripts/check_sorafs_orderbook_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_orderbook_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_orderbook_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TEST_DIR = Path(__file__).resolve().parent
if str(TEST_DIR) not in sys.path:
    sys.path.insert(0, str(TEST_DIR))
from sorafs_rollout_runner_test_support import TopologyBoundChecker  # noqa: E402


NOW_UNIX = 1_800_200_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32
DIGEST_3 = "ef" * 32
DEPLOYMENT_ID = "orderbook-production-a"
ENVIRONMENT = "production"
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="orderbook-checker",
)


def order_refs(count: int) -> list[str]:
    return [f"orderbook-order-{index:02d}" for index in range(count)]


def channel_refs(count: int) -> list[str]:
    return [f"orderbook-channel-{index:02d}" for index in range(count)]


def receipt_refs(count: int) -> list[str]:
    return [f"orderbook-receipt-{index:02d}" for index in range(count)]


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
    }


def contract_surface(*, raw_contract_state: bool = False) -> dict:
    payload = base("sorafs.orderbook.contract_surface_canary.v1")
    payload.update(
        {
            "contract_deployed": True,
            "deterministic_matching_verified": True,
            "escrow_enforced": True,
            "pause_control_configured": True,
            "fee_policy_config_bound": True,
            "capability_policy_configured": True,
            "contract_state_source": "on-chain",
            "contract_digest_hex": DIGEST,
            "policy_digest_hex": DIGEST,
            "raw_contract_state_included": raw_contract_state,
        }
    )
    return payload


def matcher_service(*, lag_ms: int = 100) -> dict:
    payload = base("sorafs.orderbook.matcher_service_canary.v1")
    payload.update(
        {
            "daemonized": True,
            "contract_forwarding_enabled": True,
            "price_time_priority_verified": True,
            "finalized_cursor_replay_verified": True,
            "committed_state_reconciliation_verified": True,
            "local_book_authority_absent": True,
            "durable_checkpoint_verified": True,
            "contract_digest_hex": DIGEST,
            "matcher_lag_ms": lag_ms,
            "accepted_order_count": 12,
            "accepted_orders": order_refs(12),
            "matched_order_count": 8,
            "matched_orders": order_refs(8),
            "rejected_invalid_order_count": 2,
            "rejected_invalid_orders": [
                "orderbook-order-invalid-00",
                "orderbook-order-invalid-01",
            ],
            "raw_ledger_included": False,
        }
    )
    return payload


def settlement_service() -> dict:
    payload = base("sorafs.orderbook.settlement_service_canary.v1")
    payload.update(
        {
            "daemonized": True,
            "contract_digest_hex": DIGEST,
            "escrow_custody_mutation_verified": True,
            "receipt_authorization_verified": True,
            "non_overlapping_ranges_enforced": True,
            "governance_receipts_published": True,
            "open_channel_count": 3,
            "open_channels": channel_refs(3),
            "settled_receipt_count": 5,
            "settled_receipts": receipt_refs(5),
            "settlement_backlog_count": 1,
            "settlement_backlog_channels": ["orderbook-channel-backlog-00"],
            "raw_receipts_included": False,
        }
    )
    return payload


def api_gateway(*, authz: bool = True, latency_ms: int = 200) -> dict:
    routes = [
        {
            "name": name,
            "passed": True,
            "status_code": 200 if name.endswith("_get") else 202,
            "body_blake3_hex": DIGEST_2,
            "latency_ms": latency_ms,
            "authz_enforced": authz,
            "signature_verified": True,
        }
        for name in (
            "orders_post",
            "cancel_post",
            "receipts_post",
            "book_get",
            "trades_get",
            "channels_get",
            "receipts_get",
            "events_get",
        )
    ]
    payload = base("sorafs.orderbook.api_gateway_canary.v1")
    payload.update(
        {
            "route_count": len(routes),
            "passed_route_count": len(routes),
            "contract_digest_hex": DIGEST,
            "canonical_request_auth_enforced": True,
            "owner_account_binding_verified": True,
            "provider_role_binding_verified": True,
            "capability_policy_enforced": True,
            "response_bodies_included": False,
            "routes": routes,
        }
    )
    return payload


def event_streams(*, lag_ms: int = 250) -> dict:
    streams = [
        {
            "name": name,
            "passed": True,
            "backlog_replay_verified": True,
            "live_delivery_verified": True,
            "contract_backed": True,
            "lag_ms": lag_ms,
        }
        for name in ("sse_orderbook_events", "websocket_orderbook_events")
    ]
    payload = base("sorafs.orderbook.event_streams_canary.v1")
    payload.update(
        {
            "contract_digest_hex": DIGEST,
            "stream_count": len(streams),
            "streams": streams,
            "response_bodies_included": False,
        }
    )
    return payload


def sdk_release() -> dict:
    payload = base("sorafs.orderbook.sdk_release_canary.v1")
    payload.update(
        {
            "artifact_hashes_verified": True,
            "contract_digest_hex": DIGEST,
            "live_smoke_passed": True,
            "submitter_helpers_verified": True,
            "debug_artifacts": False,
            "language_count": 6,
            "languages": [
                {"name": "rust"},
                {"name": "javascript"},
                {"name": "python"},
                {"name": "kotlin-jvm"},
                {"name": "java-android"},
                {"name": "swift"},
            ],
            "artifact_count": 2,
            "artifacts": [
                {"id": "rust-orderbook", "sha256": DIGEST},
                {"id": "javascript-orderbook", "sha256": "ab" * 32},
                {"id": "python-orderbook", "sha256": "cd" * 32},
                {"id": "kotlin-jvm-orderbook", "sha256": "ef" * 32},
                {"id": "java-android-orderbook", "sha256": "12" * 32},
                {"id": "swift-orderbook", "sha256": "34" * 32},
            ],
        }
    )
    payload["artifact_count"] = len(payload["artifacts"])
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.orderbook.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "metrics_scraped_at_unix": GENERATED_AT,
            "contract_digest_hex": DIGEST,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "live_dashboard_wired": True,
            "critical_alerts_firing": critical,
            "finalized_projection_ready": True,
            "finalized_projection_height": 42,
            "finalized_projection_timestamp_seconds": GENERATED_AT,
            "finalized_projection_failure_delta": 0,
            "metrics": [
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
            ],
            "metric_count": len(MODULE.REQUIRED_METRICS),
            "response_bodies_included": False,
        }
    )
    return payload


def reconciliation(*, peer_count: int = 4, mismatch_count: int = 0) -> dict:
    peers = [{"name": f"orderbook-peer-{index:02d}"} for index in range(peer_count)]
    payload = base("sorafs.orderbook.reconciliation_canary.v1")
    payload.update(
        {
            "peer_count": peer_count,
            "peers": peers,
            "contract_digest_hex": DIGEST,
            "source_count": 5,
            "sources": [
                {"name": "finalized-ledger"},
                {"name": "matcher-worker"},
                {"name": "torii-finalized-projection"},
                {"name": "settlement-worker"},
                {"name": "governance-dag"},
            ],
            "finalized_projection_reconciliation_passed": True,
            "replica_finalized_state_equal": True,
            "evidence_dag_published": True,
            "mismatch_count": mismatch_count,
            "unreconciled_event_count": 0,
            "raw_ledger_included": False,
        }
    )
    return payload


def governance_approval(*, contract_digest: str = DIGEST) -> dict:
    payload = base("sorafs.orderbook.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "orderbook_activation_governed": True,
            "emergency_pause_tested": True,
            "capability_policy_bound": True,
            "treasury_policy_bound": True,
            "contract_digest_hex": contract_digest,
            "config_source": "iroha_config",
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "contract-surface.json", contract_surface())
    write_json(root / "matcher-service.json", matcher_service())
    write_json(root / "settlement-service.json", settlement_service())
    write_json(root / "api-gateway.json", api_gateway())
    write_json(root / "event-streams.json", event_streams())
    write_json(root / "sdk-release.json", sdk_release())
    write_json(root / "observability.json", observability())
    write_json(root / "reconciliation.json", reconciliation())
    write_json(root / "governance-approval.json", governance_approval())


CONTRACT_BOUND_FIXTURES = (
    ("matcher_service", "matcher-service.json", matcher_service),
    ("settlement_service", "settlement-service.json", settlement_service),
    ("api_gateway", "api-gateway.json", api_gateway),
    ("event_streams", "event-streams.json", event_streams),
    ("sdk_release", "sdk-release.json", sdk_release),
    ("observability", "observability.json", observability),
    ("reconciliation", "reconciliation.json", reconciliation),
    ("governance_approval", "governance-approval.json", governance_approval),
)

POLICY_BOUND_FIXTURES = (
    ("governance_approval", "governance-approval.json", governance_approval),
)


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.orderbook.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["contract_surface"]["valid"] is True
    assert payload["valid_contract_digests"] == [DIGEST]
    assert payload["valid_policy_digests"] == [DIGEST]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    observability_artifact = payload["required"]["observability"]["artifacts"][0]
    assert observability_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics_scraped_at_unix"] == GENERATED_AT
    assert observability_artifact["fingerprint"]["finalized_projection_ready"] is True
    assert observability_artifact["fingerprint"]["finalized_projection_height"] == 42
    assert (
        observability_artifact["fingerprint"][
            "finalized_projection_timestamp_seconds"
        ]
        == GENERATED_AT
    )
    assert observability_artifact["fingerprint"]["finalized_projection_failure_delta"] == 0


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in CONTRACT_BOUND_FIXTURES)
        == MODULE.CONTRACT_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in POLICY_BOUND_FIXTURES)
        == MODULE.POLICY_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(route["name"] for route in api_gateway()["routes"]) == (
        MODULE.REQUIRED_API_ROUTES
    )
    assert tuple(stream["name"] for stream in event_streams()["streams"]) == (
        MODULE.REQUIRED_STREAMS
    )
    assert tuple(observability()["metrics"]) == MODULE.REQUIRED_METRICS
    assert tuple(source["name"] for source in reconciliation()["sources"]) == (
        MODULE.REQUIRED_RECONCILIATION_SOURCES
    )


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        (
            "contract_surface",
            "contract-surface.json",
            contract_surface,
            "raw_contract_state_included",
        ),
        (
            "matcher_service",
            "matcher-service.json",
            matcher_service,
            "raw_ledger_included",
        ),
        (
            "settlement_service",
            "settlement-service.json",
            settlement_service,
            "raw_receipts_included",
        ),
        (
            "api_gateway",
            "api-gateway.json",
            api_gateway,
            "response_bodies_included",
        ),
        (
            "event_streams",
            "event-streams.json",
            event_streams,
            "response_bodies_included",
        ),
        (
            "sdk_release",
            "sdk-release.json",
            sdk_release,
            "debug_artifacts",
        ),
        (
            "observability",
            "observability.json",
            observability,
            "critical_alerts_firing",
        ),
        (
            "observability",
            "observability.json",
            observability,
            "response_bodies_included",
        ),
        (
            "reconciliation",
            "reconciliation.json",
            reconciliation,
            "raw_ledger_included",
        ),
    )
    for kind, filename, factory, field in cases:
        root = tmp_path / f"{kind}-{field}"
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        del payload[field]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "orderbook.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert CHECKER([f"@{args}"]) == 0


def test_missing_matcher_service_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "matcher-service.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_contract_surface_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "contract-surface.json", payload)

    assert run_gate(tmp_path) == 1


def test_dev_environment_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["environment"] = "dev"
    write_json(tmp_path / "contract-surface.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["contract_surface"]["artifacts"][0]
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "environment must be one of ['prod', 'production', 'release', 'staging']"
    ]


def test_dev_deployment_id_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["deployment_id"] = "orderbook-dev-a"
    write_json(tmp_path / "contract-surface.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["contract_surface"]["artifacts"][0]
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
    ]


def test_payload_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["raw_order"] = {"order_id": "leaked"}
    write_json(tmp_path / "contract-surface.json", payload)

    assert run_gate(tmp_path) == 1


def test_matcher_requires_contract_digest_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    del payload["contract_digest_hex"]
    write_json(tmp_path / "matcher-service.json", payload)

    assert run_gate(tmp_path) == 1


def test_contract_surface_requires_policy_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = contract_surface()
    del payload["policy_digest_hex"]
    write_json(tmp_path / "contract-surface.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["contract_surface"]["artifacts"][0]
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert result["valid_policy_digests"] == []


def test_contract_bound_artifact_must_match_contract_surface_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = api_gateway()
    payload["contract_digest_hex"] = DIGEST_2
    write_json(tmp_path / "api-gateway.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["api_gateway"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "api_gateway contract_digest_hex must reference a valid contract_surface contract_digest_hex"
    ]


def test_all_contract_bound_artifacts_reject_contract_surface_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in CONTRACT_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["contract_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} contract_digest_hex must reference a valid "
            "contract_surface contract_digest_hex"
        ) in artifact["errors"]


def test_integer_unit_latency_and_lag_fields_reject_fractional_or_negative_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    matcher_payload = matcher_service()
    matcher_payload["matcher_lag_ms"] = 12.5
    write_json(tmp_path / "matcher-service.json", matcher_payload)
    api_payload = api_gateway()
    api_payload["routes"][0]["latency_ms"] = -1
    write_json(tmp_path / "api-gateway.json", api_payload)
    stream_payload = event_streams()
    stream_payload["streams"][0]["lag_ms"] = 250.5
    write_json(tmp_path / "event-streams.json", stream_payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    matcher_errors = result["required"]["matcher_service"]["artifacts"][0]["errors"]
    api_errors = result["required"]["api_gateway"]["artifacts"][0]["errors"]
    stream_errors = result["required"]["event_streams"]["artifacts"][0]["errors"]
    assert "matcher_lag_ms must be a non-negative integer" in matcher_errors
    assert "routes[0].latency_ms must be a non-negative integer" in api_errors
    assert "streams[0].lag_ms must be a non-negative integer" in stream_errors


def test_matcher_accepted_order_count_must_match_unique_orders(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["accepted_order_count"] += 1
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert "accepted_order_count must match unique accepted_orders count" in artifact[
        "errors"
    ]


def test_matcher_accepted_orders_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["accepted_orders"].append(payload["accepted_orders"][0])
    payload["accepted_order_count"] = len(payload["accepted_orders"])
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert "accepted_orders must not contain duplicate values" in artifact["errors"]
    assert "accepted_order_count must match unique accepted_orders count" in artifact[
        "errors"
    ]


def test_matcher_matched_orders_must_be_accepted(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["matched_orders"][-1] = "orderbook-order-outside-accepted-set"
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert "matched_orders must be a subset of accepted_orders" in artifact["errors"]


def test_matcher_order_ids_must_use_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["accepted_orders"][0] = "order_alpha"
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert (
        "accepted_orders[] must match canonical lowercase `orderbook-order-*`"
        in artifact["errors"]
    )


def test_matcher_order_ids_reject_non_orderbook_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["accepted_orders"][0] = "order-00"
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert (
        "accepted_orders[] must match canonical lowercase `orderbook-order-*`"
        in artifact["errors"]
    )


def test_matcher_order_ids_reject_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["accepted_orders"][0] = "orderbook-order-placeholder"
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert (
        "accepted_orders[] must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_matcher_rejected_invalid_order_count_must_match_unique_orders(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["rejected_invalid_order_count"] += 1
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert (
        "rejected_invalid_order_count must match unique rejected_invalid_orders count"
        in artifact["errors"]
    )


def test_matcher_rejected_invalid_orders_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["rejected_invalid_orders"].append(payload["rejected_invalid_orders"][0])
    payload["rejected_invalid_order_count"] = len(payload["rejected_invalid_orders"])
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert "rejected_invalid_orders must not contain duplicate values" in artifact[
        "errors"
    ]
    assert (
        "rejected_invalid_order_count must match unique rejected_invalid_orders count"
        in artifact["errors"]
    )


def test_matcher_rejected_invalid_orders_must_use_reviewed_labels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["rejected_invalid_orders"][0] = "order-invalid-00"
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert (
        "rejected_invalid_orders[] must match canonical lowercase `orderbook-order-*`"
        in artifact["errors"]
    )


def test_matcher_rejected_invalid_orders_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = matcher_service()
    payload["rejected_invalid_orders"][0] = "orderbook-order-placeholder"
    write_json(tmp_path / "matcher-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["matcher_service"]["artifacts"][0]
    assert (
        "rejected_invalid_orders[] must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


def test_settlement_open_channel_count_must_match_unique_channels(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["open_channel_count"] += 1
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert "open_channel_count must match unique open_channels count" in artifact[
        "errors"
    ]


def test_settlement_settled_receipts_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["settled_receipts"].append(payload["settled_receipts"][0])
    payload["settled_receipt_count"] = len(payload["settled_receipts"])
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert "settled_receipts must not contain duplicate values" in artifact["errors"]
    assert "settled_receipt_count must match unique settled_receipts count" in artifact[
        "errors"
    ]


def test_settlement_ids_must_use_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["open_channels"][0] = "channel_alpha"
    payload["settled_receipts"][0] = "receipt_alpha"
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert (
        "open_channels[] must match canonical lowercase `orderbook-channel-*`"
        in artifact["errors"]
    )
    assert (
        "settled_receipts[] must match canonical lowercase `orderbook-receipt-*`"
        in artifact["errors"]
    )


def test_settlement_ids_reject_non_orderbook_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["open_channels"][0] = "channel-00"
    payload["settled_receipts"][0] = "receipt-00"
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert (
        "open_channels[] must match canonical lowercase `orderbook-channel-*`"
        in artifact["errors"]
    )
    assert (
        "settled_receipts[] must match canonical lowercase `orderbook-receipt-*`"
        in artifact["errors"]
    )


def test_settlement_ids_reject_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["open_channels"][0] = "orderbook-channel-placeholder"
    payload["settled_receipts"][0] = "orderbook-receipt-placeholder"
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert (
        "open_channels[] must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )
    assert (
        "settled_receipts[] must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_settlement_backlog_count_must_match_unique_channels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["settlement_backlog_count"] += 1
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert (
        "settlement_backlog_count must match unique settlement_backlog_channels count"
        in artifact["errors"]
    )


def test_settlement_backlog_channels_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["settlement_backlog_channels"].append(
        payload["settlement_backlog_channels"][0]
    )
    payload["settlement_backlog_count"] = len(payload["settlement_backlog_channels"])
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert "settlement_backlog_channels must not contain duplicate values" in artifact[
        "errors"
    ]
    assert (
        "settlement_backlog_count must match unique settlement_backlog_channels count"
        in artifact["errors"]
    )


def test_settlement_backlog_channels_must_use_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["settlement_backlog_channels"][0] = "channel-backlog-00"
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert (
        "settlement_backlog_channels[] must match canonical lowercase "
        "`orderbook-channel-*`"
    ) in artifact["errors"]


def test_settlement_backlog_channels_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = settlement_service()
    payload["settlement_backlog_channels"][0] = "orderbook-channel-placeholder"
    write_json(tmp_path / "settlement-service.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["settlement_service"]["artifacts"][0]
    assert (
        "settlement_backlog_channels[] must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


def test_api_gateway_route_count_must_match_unique_routes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = api_gateway()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "api-gateway.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["api_gateway"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_api_gateway_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = api_gateway()
    payload["routes"].append(dict(payload["routes"][0]))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "api-gateway.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["api_gateway"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_api_gateway_routes_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = api_gateway()
    payload["routes"].append(
        {
            "name": "debug_backdoor",
            "passed": True,
            "status_code": 200,
            "body_blake3_hex": DIGEST_2,
            "latency_ms": 200,
            "authz_enforced": True,
            "signature_verified": True,
        }
    )
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "api-gateway.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["api_gateway"]["artifacts"][0]
    assert "routes must not include unknown values" in artifact["errors"]


def test_api_gateway_route_body_hash_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = api_gateway()
    del payload["routes"][0]["body_blake3_hex"]
    write_json(tmp_path / "api-gateway.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["api_gateway"]["artifacts"][0]
    assert "routes[0].body_blake3_hex must be a non-empty string" in artifact["errors"]


def test_event_streams_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = event_streams()
    payload["streams"].append(dict(payload["streams"][0]))
    payload["stream_count"] = len(payload["streams"])
    write_json(tmp_path / "event-streams.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["event_streams"]["artifacts"][0]
    assert "streams must not contain duplicate values" in artifact["errors"]
    assert "stream_count must match unique streams count" in artifact["errors"]


def test_event_streams_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = event_streams()
    payload["streams"].append(
        {
            "name": "long_poll_orderbook_events",
            "passed": True,
            "backlog_replay_verified": True,
            "live_delivery_verified": True,
            "contract_backed": True,
            "lag_ms": 250,
        }
    )
    payload["stream_count"] = len(payload["streams"])
    write_json(tmp_path / "event-streams.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["event_streams"]["artifacts"][0]
    assert "streams must not include unknown values" in artifact["errors"]


def test_event_stream_count_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = event_streams()
    del payload["stream_count"]
    write_json(tmp_path / "event-streams.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["event_streams"]["artifacts"][0]
    assert "stream_count must be a positive integer" in artifact["errors"]


def test_sdk_artifacts_must_not_duplicate_id(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["artifacts"].append(dict(payload["artifacts"][0]))
    payload["artifact_count"] = len(payload["artifacts"])
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "artifacts must not contain duplicate values" in artifact["errors"]
    assert "artifact_count must match unique artifacts count" in artifact["errors"]


def test_sdk_artifact_count_must_cover_reviewed_languages(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["artifacts"].pop()
    payload["artifact_count"] = len(payload["artifacts"])
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "artifact_count must be at least 6" in artifact["errors"]


def test_sdk_artifacts_must_cover_every_reviewed_language(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["artifacts"] = [
        {"id": f"rust-orderbook-{index:02d}", "sha256": DIGEST}
        for index in range(len(MODULE.REQUIRED_SDK_LANGUAGES))
    ]
    payload["artifact_count"] = len(payload["artifacts"])
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert (
        "artifacts must include at least one SDK release artifact for every "
        "reviewed language"
    ) in artifact["errors"]


def test_sdk_artifacts_must_use_reviewed_language_prefixes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["artifacts"][0]["id"] = "go-orderbook-private-key-placeholder"
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    errors = "\n".join(artifact["errors"])
    assert "artifacts[].id must start with a reviewed SDK language prefix" in errors
    assert "go-orderbook-private-key-placeholder" not in errors


def test_sdk_release_debug_artifacts_flag_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    del payload["debug_artifacts"]
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "debug_artifacts must be false" in artifact["errors"]


def test_sdk_languages_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["languages"].append(dict(payload["languages"][0]))
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "languages must not contain duplicate values" in artifact["errors"]


def test_sdk_languages_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["languages"].append({"name": "go"})
    payload["language_count"] = len(payload["languages"])
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "languages must not include unknown values" in artifact["errors"]


def test_sdk_language_count_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    del payload["language_count"]
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "language_count must be a positive integer" in artifact["errors"]
    assert "language_count must be at least 6" in artifact["errors"]


def test_sdk_language_count_must_match_inventory(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = sdk_release()
    payload["language_count"] += 1
    write_json(tmp_path / "sdk-release.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["sdk_release"]["artifacts"][0]
    assert "language_count must match unique languages count" in artifact["errors"]


def test_observability_metrics_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append(payload["metrics"][0])
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_observability_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append("torii_sorafs_orderbook_debug_metric")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


def test_observability_requires_ready_fresh_finalized_projection(tmp_path: Path) -> None:
    for field, value, expected_error in (
        (
            "finalized_projection_ready",
            False,
            "finalized_projection_ready must be true",
        ),
        (
            "finalized_projection_failure_delta",
            1,
            "finalized_projection_failure_delta must be 0",
        ),
        (
            "metrics_scraped_at_unix",
            NOW_UNIX - MODULE.DEFAULT_MAX_METRICS_SCRAPE_AGE_SECS - 1,
            "metrics_scraped_at_unix is older than",
        ),
        (
            "finalized_projection_timestamp_seconds",
            NOW_UNIX - MODULE.DEFAULT_MAX_METRICS_SCRAPE_AGE_SECS - 1,
            "finalized_projection_timestamp_seconds is older than",
        ),
    ):
        root = tmp_path / field
        root.mkdir()
        write_complete_evidence(root)
        payload = observability()
        payload[field] = value
        write_json(root / "observability.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--summary-out", str(summary)) == 1
        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["observability"]["artifacts"][0]
        assert any(expected_error in error for error in artifact["errors"])


def test_observability_rejects_projection_newer_than_scrape(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["finalized_projection_timestamp_seconds"] = (
        payload["metrics_scraped_at_unix"] + 1
    )
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert (
        "finalized_projection_timestamp_seconds must not be after metrics_scraped_at_unix"
        in artifact["errors"]
    )


def test_governance_approval_must_match_contract_surface_digest(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "governance-approval.json",
        governance_approval(contract_digest=DIGEST_2),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval contract_digest_hex must reference a valid "
        "contract_surface contract_digest_hex"
    ]


def test_governance_approval_must_match_contract_surface_policy_digest(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    required = result["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert result["valid_policy_digests"] == [DIGEST]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval policy_digest_hex must reference a valid "
        "contract_surface policy_digest_hex"
    ]


def test_all_policy_bound_artifacts_reject_contract_policy_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in POLICY_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["policy_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_policy_digests"] == [DIGEST]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} policy_digest_hex must reference a valid "
            "contract_surface policy_digest_hex"
        ) in artifact["errors"]


def test_multiple_valid_contract_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["contract_digest_hex"] = DIGEST_3
    write_json(tmp_path / "contract-surface-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_contract_digests"] == []
    assert (
        "valid_contract_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_multiple_valid_policy_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["policy_digest_hex"] = DIGEST_3
    write_json(tmp_path / "contract-surface-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_policy_digests"] == []
    assert (
        "valid_policy_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_reconciliation_source_count_must_match_unique_sources(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["source_count"] += 1
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "source_count must match unique sources count" in artifact["errors"]


def test_reconciliation_sources_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["sources"].append(dict(payload["sources"][0]))
    payload["source_count"] = len(payload["sources"])
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "sources must not contain duplicate values" in artifact["errors"]
    assert "source_count must match unique sources count" in artifact["errors"]


def test_reconciliation_sources_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["sources"].append({"name": "shadow-indexer"})
    payload["source_count"] = len(payload["sources"])
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "sources must not include unknown values" in artifact["errors"]


def test_reconciliation_peer_count_must_match_unique_peers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["peer_count"] += 1
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "peer_count must match unique peers count" in artifact["errors"]


def test_reconciliation_peers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["peers"].append(dict(payload["peers"][0]))
    payload["peer_count"] = len(payload["peers"])
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "peers must not contain duplicate values" in artifact["errors"]
    assert "peer_count must match unique peers count" in artifact["errors"]


def test_reconciliation_peer_names_must_use_reviewed_labels(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["peers"][0]["name"] = "peer_alpha"
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "peers[].name must match canonical lowercase `orderbook-peer-*`" in artifact[
        "errors"
    ]


def test_reconciliation_peer_names_reject_non_orderbook_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["peers"][0]["name"] = "peer-00"
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert "peers[].name must match canonical lowercase `orderbook-peer-*`" in artifact[
        "errors"
    ]


def test_reconciliation_peer_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = reconciliation()
    payload["peers"][0]["name"] = "orderbook-peer-placeholder"
    write_json(tmp_path / "reconciliation.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["reconciliation"]["artifacts"][0]
    assert (
        "peers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_policy_bound_subset_requires_contract_surface_anchor(tmp_path: Path) -> None:
    write_json(tmp_path / "governance-approval.json", governance_approval())
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-kind",
            "governance_approval",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["governance_approval"]["artifacts"][0]
    assert result["valid_policy_digests"] == []
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "governance_approval contract_digest_hex requires a valid "
        "contract_surface contract_digest_hex",
        "governance_approval policy_digest_hex requires a valid contract_surface "
        "policy_digest_hex",
    ]


def test_invalid_contract_surface_does_not_anchor_contract_bound_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = contract_surface()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "contract-surface.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["matcher_service"]
    artifact = required["artifacts"][0]
    assert payload["valid_contract_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "matcher_service contract_digest_hex requires a valid contract_surface contract_digest_hex"
    ]


def test_api_route_without_authz_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "api-gateway.json", api_gateway(authz=False))

    assert run_gate(tmp_path) == 1


def test_matcher_lag_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "matcher-service.json", matcher_service(lag_ms=5_000))

    assert run_gate(tmp_path) == 1


def test_stream_lag_above_threshold_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "event-streams.json", event_streams(lag_ms=10_000))

    assert run_gate(tmp_path) == 1


def test_reconciliation_requires_four_peers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "reconciliation.json", reconciliation(peer_count=3))

    assert run_gate(tmp_path) == 1


def test_reconciliation_mismatch_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "reconciliation.json", reconciliation(mismatch_count=1))

    assert run_gate(tmp_path) == 1


def test_observability_critical_alert_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.orderbook.unknown.v1"})

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "contract-surface.json", contract_surface())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.orderbook.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "contract_surface") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "contract-surface.json", contract_surface())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "contract_surface") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert CHECKER(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
