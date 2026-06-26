"""Tests for scripts/check_sorafs_gateway_compliance_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_gateway_compliance_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_gateway_compliance_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_900_000
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
        "deployment_id": "gateway-compliance-staging-a",
        "environment": "staging",
    }


def route(name: str, *, latency_ms: int = 120) -> dict:
    return {
        "name": name,
        "passed": True,
        "status_code": 200,
        "latency_ms": latency_ms,
        "authz_enforced": True,
    }


def feed_promotion(*, ack_count: int = 3) -> dict:
    payload = base("sorafs.gateway_compliance.feed_promotion_canary.v1")
    payload.update(
        {
            "external_feeds_normalized": True,
            "feed_signature_verified": True,
            "bundle_pack_verified": True,
            "bundle_diff_reviewed": True,
            "merkle_root_bound": True,
            "update_history_persisted": True,
            "gateway_ack_count": ack_count,
            "denylist_entry_count": 5,
            "bundle_digest_hex": DIGEST,
            "raw_feeds_included": False,
            "feed_payloads_included": False,
        }
    )
    return payload


def gateway_reload(*, reload_latency_ms: int = 1_000) -> dict:
    payload = base("sorafs.gateway_compliance.gateway_reload_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "reload_ack_count": 3,
            "max_reload_latency_ms": reload_latency_ms,
            "hot_reload_verified": True,
            "cache_version_bound": True,
            "denylist_catalog_readback_verified": True,
            "persistence_path_configured": True,
            "stale_bundle_rejected": True,
            "rollback_plan_verified": True,
            "raw_catalog_included": False,
        }
    )
    return payload


def enforcement_probe(*, reasons: list[str] | None = None) -> dict:
    payload = base("sorafs.gateway_compliance.enforcement_probe_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "denial_reasons_observed": list(MODULE.REQUIRED_DENIAL_REASONS)
            if reasons is None
            else reasons,
            "structured_error_labels_verified": True,
            "telemetry_labels_stable": True,
            "fail_closed_missing_envelope": True,
            "fail_closed_unadmitted_provider": True,
            "rate_limit_verified": True,
            "geofence_verified": True,
            "proof_token_required": True,
            "response_bodies_included": False,
            "routes": [route("manifest"), route("cid"), route("provider")],
        }
    )
    return payload


def honey_audit(*, probe_count: int = 4) -> dict:
    payload = base("sorafs.gateway_compliance.honey_audit_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "honey_probe_count": probe_count,
            "denied_response_verified": True,
            "cache_version_binding_verified": True,
            "proof_token_verified": True,
            "json_report_generated": True,
            "markdown_report_generated": True,
            "audit_digest_hex": DIGEST,
            "raw_probe_responses_included": False,
        }
    )
    return payload


def appeal_override() -> dict:
    payload = base("sorafs.gateway_compliance.appeal_override_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "appeal_outcome_consumed": True,
            "policy_override_signed": True,
            "cache_invalidation_verified": True,
            "override_expiry_enforced": True,
            "operator_audit_trail_persisted": True,
            "denylist_override_scoped": True,
            "override_digest_hex": DIGEST,
            "raw_appeal_payload_included": False,
        }
    )
    return payload


def transparency_publication() -> dict:
    payload = base("sorafs.gateway_compliance.transparency_publication_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "gar_receipts_published": True,
            "proof_token_index_published": True,
            "moderation_events_published": True,
            "legal_hold_redaction_summaries_published": True,
            "governance_dag_bound": True,
            "transparency_cycle_verified": True,
            "publication_digest_hex": DIGEST,
            "raw_receipts_included": False,
        }
    )
    return payload


def observability(*, critical: bool = False) -> dict:
    payload = base("sorafs.gateway_compliance.observability_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "critical_alerts_firing": critical,
            "metrics": list(MODULE.REQUIRED_METRICS),
            "response_bodies_included": False,
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.gateway_compliance.governance_approval.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "compliance_policy_bound": True,
            "denylist_feed_roster_bound": True,
            "transparency_policy_bound": True,
            "operator_roles_bound": True,
            "retention_policy_bound": True,
            "config_source": "iroha_config",
            "policy_digest_hex": DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "feed-promotion.json", feed_promotion())
    write_json(root / "gateway-reload.json", gateway_reload())
    write_json(root / "enforcement-probe.json", enforcement_probe())
    write_json(root / "honey-audit.json", honey_audit())
    write_json(root / "appeal-override.json", appeal_override())
    write_json(root / "transparency-publication.json", transparency_publication())
    write_json(root / "observability.json", observability())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_missing_argument_value_returns_argparse_error_code(capsys) -> None:
    assert MODULE.main(["--evidence-dir"]) == 2

    captured = capsys.readouterr()
    assert "expected one argument" in captured.err
    assert captured.out == ""


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.gateway_compliance.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["feed_promotion"]["valid"] is True
    assert payload["valid_bundle_digests"] == [DIGEST]


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "gateway.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_evidence_sources_fail_shared_preflight(capsys) -> None:
    assert MODULE.main(["--now-unix", str(NOW_UNIX)]) == 2

    captured = capsys.readouterr()
    assert "ERROR: provide --evidence-dir or --evidence" in captured.err


def test_missing_gateway_reload_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "gateway-reload.json").unlink()

    assert run_gate(tmp_path) == 1


def test_gateway_reload_requires_bundle_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = gateway_reload()
    payload.pop("bundle_digest_hex")
    write_json(tmp_path / "gateway-reload.json", payload)

    assert run_gate(tmp_path) == 1


def test_enforcement_bundle_binding_must_match_feed_promotion(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["bundle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["enforcement_probe"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "enforcement_probe bundle_digest_hex must match a valid feed_promotion bundle_digest_hex"
    ]


def test_invalid_feed_promotion_does_not_anchor_bundle_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "feed-promotion.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["gateway_reload"]
    artifact = required["artifacts"][0]
    assert payload["valid_bundle_digests"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "gateway_reload bundle_digest_hex requires a valid feed_promotion bundle_digest_hex"
    ]


def test_stale_feed_promotion_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "feed-promotion.json", payload)

    assert run_gate(tmp_path) == 1


def test_raw_feed_leakage_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["raw_feed"] = {"provider": "runtime-feed"}
    write_json(tmp_path / "feed-promotion.json", payload)

    assert run_gate(tmp_path) == 1


def test_enforcement_requires_denial_reason_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "enforcement-probe.json", enforcement_probe(reasons=["provider"]))

    assert run_gate(tmp_path) == 1


def test_honey_audit_requires_minimum_probe_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "honey-audit.json", honey_audit(probe_count=1))

    assert run_gate(tmp_path) == 1


def test_governance_must_use_iroha_config(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["config_source"] = "environment"
    write_json(tmp_path / "governance-approval.json", payload)

    assert run_gate(tmp_path) == 1


def test_unknown_directory_artifact_is_ignored_for_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "feed-promotion.json", feed_promotion())
    write_json(tmp_path / "unknown.json", {"schema": "sorafs.gateway_compliance.unknown.v1"})

    assert run_gate(tmp_path, "--require-kind", "feed_promotion") == 0


def test_unknown_required_kind_uses_shared_error_reporting(tmp_path: Path, capsys) -> None:
    assert run_gate(tmp_path, "--require-kind", "unknown") == 2

    captured = capsys.readouterr()
    assert "unknown required evidence kind `unknown`" in captured.err
    assert "usage:" not in captured.err
    assert captured.out == ""


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.gateway_compliance.unknown.v1"},
    )

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1
