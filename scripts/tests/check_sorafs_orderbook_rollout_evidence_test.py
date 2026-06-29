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


NOW_UNIX = 1_800_200_000
GENERATED_AT = NOW_UNIX - 120
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "orderbook-staging-a",
        "environment": "staging",
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
            "replay_snapshot_verified": True,
            "durable_checkpoint_verified": True,
            "contract_digest_hex": DIGEST,
            "divergence_detected": False,
            "matcher_lag_ms": lag_ms,
            "accepted_order_count": 12,
            "matched_order_count": 8,
            "rejected_invalid_order_count": 2,
            "raw_snapshot_included": False,
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
            "settled_receipt_count": 5,
            "settlement_backlog_count": 1,
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
                {"id": "iroha-js-sorafs-orderbook", "sha256": DIGEST},
                {"id": "iroha-python-sorafs-orderbook", "sha256": "cd" * 32},
            ],
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.orderbook.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "contract_digest_hex": DIGEST,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "live_dashboard_wired": True,
            "critical_alerts_firing": critical,
            "metrics": [
                "torii_sorafs_orderbook_order_flow_total",
                "torii_sorafs_orderbook_open_depth",
                "torii_sorafs_orderbook_matcher_lag_ms",
                "torii_sorafs_orderbook_settlement_backlog",
                "torii_sorafs_orderbook_api_error_ratio",
                "torii_sorafs_orderbook_escrow_runway_seconds",
                "torii_sorafs_orderbook_contract_mirror_divergence",
            ],
            "response_bodies_included": False,
        }
    )
    return payload


def reconciliation(*, peer_count: int = 4, mismatch_count: int = 0) -> dict:
    payload = base("sorafs.orderbook.reconciliation_canary.v1")
    payload.update(
        {
            "peer_count": peer_count,
            "contract_digest_hex": DIGEST,
            "source_count": 5,
            "sources": [
                {"name": "contract"},
                {"name": "matcher"},
                {"name": "torii-mirror"},
                {"name": "settlement-service"},
                {"name": "governance-dag"},
            ],
            "contract_mirror_reconciliation_passed": True,
            "evidence_dag_published": True,
            "contract_mirror_divergence": False,
            "mismatch_count": mismatch_count,
            "unreconciled_event_count": 0,
            "raw_ledger_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
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


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.orderbook.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["contract_surface"]["valid"] is True


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "orderbook.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


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

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_unknown_directory_artifact_is_ignored_for_subset(tmp_path: Path) -> None:
    write_json(tmp_path / "contract-surface.json", contract_surface())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.orderbook.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "contract_surface") == 0


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "contract-surface.json", contract_surface())
    write_json(tmp_path / "observability.json", observability(critical=True))

    assert run_gate(tmp_path, "--require-kind", "contract_surface") == 1


def test_unknown_required_kind_returns_usage_error(tmp_path: Path) -> None:
    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "unknown"]) == 2
