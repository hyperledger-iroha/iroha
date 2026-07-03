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
POLICY_DIGEST = "ef" * 32


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
        "deployment_context_reviewed": True,
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
    gateways = [
        {"name": "gateway-a"},
        {"name": "gateway-b"},
        {"name": "gateway-c"},
    ]
    denylist_entries = [
        {"name": "ofac"},
        {"name": "eu-sanctions"},
        {"name": "malware"},
        {"name": "csam-hash"},
        {"name": "legal-hold"},
    ]
    if ack_count != len(gateways):
        gateways = [{"name": f"gateway-{index}"} for index in range(ack_count)]
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
            "gateways": gateways,
            "denylist_entry_count": len(denylist_entries),
            "denylist_entries": denylist_entries,
            "bundle_digest_hex": DIGEST,
            "policy_digest_hex": POLICY_DIGEST,
            "raw_feeds_included": False,
            "feed_payloads_included": False,
        }
    )
    return payload


def controller_runtime() -> dict:
    feeds = [{"name": name} for name in MODULE.REQUIRED_CONTROLLER_FEEDS]
    payload = base("sorafs.gateway_compliance.controller_runtime_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "controller_instance_id": "gateway-compliance-controller-a",
            "iroha_config_bound": True,
            "config_source": "iroha_config",
            "external_feed_count": len(feeds),
            "fetched_feed_count": len(feeds),
            "normalized_feed_count": len(feeds),
            "signed_feed_count": len(feeds),
            "feeds": feeds,
            "controller_service_enabled": True,
            "scheduler_config_bound": True,
            "external_feeds_fetched": True,
            "feed_signature_verified": True,
            "normalization_deterministic": True,
            "bundle_pack_verified": True,
            "update_history_persisted": True,
            "gateway_reload_requested": True,
            "failure_backoff_configured": True,
            "rollback_plan_verified": True,
            "raw_feeds_included": False,
            "feed_payloads_included": False,
            "response_bodies_included": False,
        }
    )
    return payload


def moderation_toggle() -> dict:
    toggles = [{"name": name} for name in MODULE.REQUIRED_MODERATION_TOGGLES]
    payload = base("sorafs.gateway_compliance.moderation_toggle_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "toggle_api_url": "https://gateway.example/v1/sorafs/gateway/moderation-toggles",
            "toggle_count": len(toggles),
            "approved_toggle_count": len(toggles),
            "toggles": toggles,
            "toggle_digest_hex": DIGEST,
            "iroha_config_bound": True,
            "config_source": "iroha_config",
            "operator_role_enforced": True,
            "approval_workflow_verified": True,
            "expiry_enforced": True,
            "cache_invalidation_verified": True,
            "operator_audit_trail_persisted": True,
            "rollback_verified": True,
            "raw_toggle_payloads_included": False,
            "response_bodies_included": False,
        }
    )
    return payload


def gateway_reload(*, reload_latency_ms: int = 1_000) -> dict:
    gateways = [
        {"name": "gateway-a"},
        {"name": "gateway-b"},
        {"name": "gateway-c"},
    ]
    payload = base("sorafs.gateway_compliance.gateway_reload_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "reload_ack_count": len(gateways),
            "gateways": gateways,
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
    denial_reasons = list(MODULE.REQUIRED_DENIAL_REASONS) if reasons is None else reasons
    payload = base("sorafs.gateway_compliance.enforcement_probe_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "denial_reasons_observed": denial_reasons,
            "denial_reason_count": len(denial_reasons),
            "structured_error_labels_verified": True,
            "telemetry_labels_stable": True,
            "fail_closed_missing_envelope": True,
            "fail_closed_unadmitted_provider": True,
            "rate_limit_verified": True,
            "geofence_verified": True,
            "proof_token_required": True,
            "response_bodies_included": False,
            "route_count": 3,
            "passed_route_count": 3,
            "routes": [route("manifest"), route("cid"), route("provider")],
        }
    )
    return payload


def honey_audit(*, probe_count: int = 4) -> dict:
    probes = [{"name": f"honey-probe-{index}"} for index in range(probe_count)]
    payload = base("sorafs.gateway_compliance.honey_audit_canary.v1")
    payload.update(
        {
            "bundle_digest_hex": DIGEST,
            "honey_probe_count": probe_count,
            "probes": probes,
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
            "metric_count": len(MODULE.REQUIRED_METRICS),
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
            "policy_digest_hex": POLICY_DIGEST,
        }
    )
    return payload


def write_complete_evidence(root: Path) -> None:
    write_json(root / "feed-promotion.json", feed_promotion())
    write_json(root / "controller-runtime.json", controller_runtime())
    write_json(root / "moderation-toggle.json", moderation_toggle())
    write_json(root / "gateway-reload.json", gateway_reload())
    write_json(root / "enforcement-probe.json", enforcement_probe())
    write_json(root / "honey-audit.json", honey_audit())
    write_json(root / "appeal-override.json", appeal_override())
    write_json(root / "transparency-publication.json", transparency_publication())
    write_json(root / "observability.json", observability())
    write_json(root / "governance-approval.json", governance_approval())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


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


def test_observability_metrics_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = observability()
    payload["metrics"].append("sorafs_gateway_unknown_metric_total")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "observability.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1
    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["observability"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "metrics must not include unknown values" in artifact["errors"]


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
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    observability_artifact = payload["required"]["observability"]["artifacts"][0]
    assert observability_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert observability_artifact["fingerprint"]["metrics"] == list(
        MODULE.REQUIRED_METRICS
    )


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


def test_controller_runtime_requires_feed_count_equality(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["signed_feed_count"] = 1
    write_json(tmp_path / "controller-runtime.json", payload)

    assert run_gate(tmp_path) == 1


def test_controller_runtime_instance_id_must_be_canonical(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["controller_instance_id"] = "gateway_compliance_controller_prod_a"
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["controller_runtime"]["artifacts"][0]
    assert MODULE.CONTROLLER_INSTANCE_ID_ERROR in artifact["errors"]


def test_controller_runtime_instance_id_rejects_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["controller_instance_id"] = "compliance-controller-prod-placeholder"
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["controller_runtime"]["artifacts"][0]
    assert (
        "controller_instance_id must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


def test_controller_runtime_instance_id_accepts_gateway_prefixed_label(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["controller_instance_id"] = "gateway-compliance-controller-prod-b-2"
    write_json(tmp_path / "controller-runtime.json", payload)

    assert run_gate(tmp_path) == 0


def test_controller_runtime_feed_count_must_match_unique_feeds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["external_feed_count"] += 1
    payload["fetched_feed_count"] = payload["external_feed_count"]
    payload["normalized_feed_count"] = payload["external_feed_count"]
    payload["signed_feed_count"] = payload["external_feed_count"]
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["controller_runtime"]["artifacts"][0]
    assert "external_feed_count must match unique feeds count" in artifact["errors"]


def test_controller_runtime_feeds_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["feeds"].append({"name": "ofac"})
    payload["external_feed_count"] = len(payload["feeds"])
    payload["fetched_feed_count"] = len(payload["feeds"])
    payload["normalized_feed_count"] = len(payload["feeds"])
    payload["signed_feed_count"] = len(payload["feeds"])
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["controller_runtime"]["artifacts"][0]
    assert "feeds must not contain duplicate values" in artifact["errors"]
    assert "external_feed_count must match unique feeds count" in artifact["errors"]


def test_controller_runtime_must_cover_required_feeds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["feeds"] = [
        feed for feed in payload["feeds"] if feed["name"] != "appeal-overrides"
    ]
    payload["external_feed_count"] = len(payload["feeds"])
    payload["fetched_feed_count"] = len(payload["feeds"])
    payload["normalized_feed_count"] = len(payload["feeds"])
    payload["signed_feed_count"] = len(payload["feeds"])
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["controller_runtime"]["artifacts"][0]
    assert "external_feed_count must be at least 7" in artifact["errors"]
    assert "feeds must include name `appeal-overrides`" in artifact["errors"]


def test_controller_runtime_feeds_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["feeds"].append({"name": "unknown-feed"})
    payload["external_feed_count"] = len(payload["feeds"])
    payload["fetched_feed_count"] = len(payload["feeds"])
    payload["normalized_feed_count"] = len(payload["feeds"])
    payload["signed_feed_count"] = len(payload["feeds"])
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["controller_runtime"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "feeds must not include unknown values" in artifact["errors"]


def test_controller_runtime_bundle_binding_must_match_feed_promotion(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = controller_runtime()
    payload["bundle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "controller-runtime.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["controller_runtime"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "controller_runtime bundle_digest_hex must match a valid feed_promotion bundle_digest_hex"
    ]


def test_feed_promotion_policy_digest_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload.pop("policy_digest_hex")
    write_json(tmp_path / "feed-promotion.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["feed_promotion"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "policy_digest_hex must be a non-empty string" in artifact["errors"]
    assert payload["valid_policy_digests"] == []


def test_governance_approval_policy_digest_must_match_feed_promotion(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = governance_approval()
    payload["policy_digest_hex"] = DIGEST_2
    write_json(tmp_path / "governance-approval.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["governance_approval"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    assert (
        "governance_approval policy_digest_hex must match a valid "
        "feed_promotion policy_digest_hex"
    ) in artifact["errors"]


def test_moderation_toggle_requires_approved_toggle_count_equality(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = moderation_toggle()
    payload["approved_toggle_count"] = 1
    write_json(tmp_path / "moderation-toggle.json", payload)

    assert run_gate(tmp_path) == 1


def test_moderation_toggle_count_must_match_unique_toggles(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = moderation_toggle()
    payload["toggle_count"] += 1
    payload["approved_toggle_count"] = payload["toggle_count"]
    write_json(tmp_path / "moderation-toggle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["moderation_toggle"]["artifacts"][0]
    assert "toggle_count must match unique toggles count" in artifact["errors"]


def test_moderation_toggle_toggles_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = moderation_toggle()
    payload["toggles"].append({"name": "provider-deny"})
    payload["toggle_count"] = len(payload["toggles"])
    payload["approved_toggle_count"] = len(payload["toggles"])
    write_json(tmp_path / "moderation-toggle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["moderation_toggle"]["artifacts"][0]
    assert "toggles must not contain duplicate values" in artifact["errors"]
    assert "toggle_count must match unique toggles count" in artifact["errors"]


def test_moderation_toggle_must_cover_required_toggles(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = moderation_toggle()
    payload["toggles"] = [
        toggle for toggle in payload["toggles"] if toggle["name"] != "regional-emergency"
    ]
    payload["toggle_count"] = len(payload["toggles"])
    payload["approved_toggle_count"] = len(payload["toggles"])
    write_json(tmp_path / "moderation-toggle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["moderation_toggle"]["artifacts"][0]
    assert "toggle_count must be at least 4" in artifact["errors"]
    assert "toggles must include name `regional-emergency`" in artifact["errors"]


def test_moderation_toggle_toggles_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = moderation_toggle()
    payload["toggles"].append({"name": "unknown-toggle"})
    payload["toggle_count"] = len(payload["toggles"])
    payload["approved_toggle_count"] = len(payload["toggles"])
    write_json(tmp_path / "moderation-toggle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["moderation_toggle"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "toggles must not include unknown values" in artifact["errors"]


def test_moderation_toggle_url_must_be_safe_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    unsafe_urls = (
        "https://user:private_key@gateway.example/v1/toggles",
        "https://gateway.example/%2e%2e/toggles",
        "https://gateway.example/bad%2Ftoggle",
        "https://gateway.example/C%3A/toggles",
        "https://gateway.example/v1/toggles?token=secret",
    )

    for index, unsafe_url in enumerate(unsafe_urls):
        case_dir = tmp_path / f"case-{index}"
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = moderation_toggle()
        payload["toggle_api_url"] = unsafe_url
        write_json(case_dir / "moderation-toggle.json", payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        captured = capsys.readouterr()
        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"]["moderation_toggle"]["artifacts"][0]
        result_text = json.dumps(result, sort_keys=True)
        assert MODULE.EVIDENCE_URL_FIELD_ERROR in artifact["errors"]
        assert unsafe_url not in captured.err
        assert unsafe_url not in result_text


def test_moderation_toggle_bundle_binding_must_match_feed_promotion(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = moderation_toggle()
    payload["bundle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "moderation-toggle.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["moderation_toggle"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "moderation_toggle bundle_digest_hex must match a valid feed_promotion bundle_digest_hex"
    ]


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


def test_gateway_reload_ack_count_must_match_unique_gateways(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = gateway_reload()
    payload["reload_ack_count"] += 1
    write_json(tmp_path / "gateway-reload.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["gateway_reload"]["artifacts"][0]
    assert "reload_ack_count must match unique gateways count" in artifact["errors"]


def test_gateway_reload_gateways_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = gateway_reload()
    payload["gateways"].append({"name": "gateway-a"})
    payload["reload_ack_count"] = len(payload["gateways"])
    write_json(tmp_path / "gateway-reload.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["gateway_reload"]["artifacts"][0]
    assert "gateways must not contain duplicate values" in artifact["errors"]
    assert "reload_ack_count must match unique gateways count" in artifact["errors"]


def test_stale_feed_promotion_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["generated_at_unix"] = NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1
    write_json(tmp_path / "feed-promotion.json", payload)

    assert run_gate(tmp_path) == 1


def test_feed_promotion_ack_count_must_match_unique_gateways(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["gateway_ack_count"] += 1
    write_json(tmp_path / "feed-promotion.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["feed_promotion"]["artifacts"][0]
    assert "gateway_ack_count must match unique gateways count" in artifact["errors"]


def test_feed_promotion_gateways_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["gateways"].append({"name": "gateway-a"})
    payload["gateway_ack_count"] = len(payload["gateways"])
    write_json(tmp_path / "feed-promotion.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["feed_promotion"]["artifacts"][0]
    assert "gateways must not contain duplicate values" in artifact["errors"]
    assert "gateway_ack_count must match unique gateways count" in artifact["errors"]


def test_feed_promotion_denylist_entry_count_must_match_unique_entries(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["denylist_entry_count"] += 1
    write_json(tmp_path / "feed-promotion.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["feed_promotion"]["artifacts"][0]
    assert (
        "denylist_entry_count must match unique denylist_entries count"
        in artifact["errors"]
    )


def test_feed_promotion_denylist_entries_must_not_duplicate(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = feed_promotion()
    payload["denylist_entries"].append(dict(payload["denylist_entries"][0]))
    payload["denylist_entry_count"] = len(payload["denylist_entries"])
    write_json(tmp_path / "feed-promotion.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["feed_promotion"]["artifacts"][0]
    assert "denylist_entries must not contain duplicate values" in artifact["errors"]
    assert (
        "denylist_entry_count must match unique denylist_entries count"
        in artifact["errors"]
    )


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


def test_enforcement_requires_denial_reason_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    del payload["denial_reason_count"]
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["enforcement_probe"]["artifacts"][0]
    assert "denial_reason_count must be a positive integer" in artifact["errors"]


def test_enforcement_denial_reason_count_must_match_unique_reasons(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["denial_reason_count"] += 1
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["enforcement_probe"]["artifacts"][0]
    assert (
        "denial_reason_count must match unique denial_reasons_observed count"
        in artifact["errors"]
    )


def test_enforcement_route_count_must_match_unique_routes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["route_count"] += 1
    payload["passed_route_count"] = payload["route_count"]
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["enforcement_probe"]["artifacts"][0]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_enforcement_routes_must_cover_required_route_inventory(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["routes"] = [route("manifest"), route("cid")]
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["enforcement_probe"]["artifacts"][0]
    assert "route_count must be at least 3" in artifact["errors"]
    assert "routes must include name `provider`" in artifact["errors"]


def test_enforcement_routes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["routes"].append(route("manifest"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["enforcement_probe"]["artifacts"][0]
    assert "routes must not contain duplicate values" in artifact["errors"]
    assert "route_count must match unique routes count" in artifact["errors"]


def test_enforcement_routes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["routes"].append(route("shadow-route"))
    payload["route_count"] = len(payload["routes"])
    payload["passed_route_count"] = len(payload["routes"])
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["enforcement_probe"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "routes must not include unknown values" in artifact["errors"]


def test_enforcement_denial_reasons_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["denial_reasons_observed"].append(payload["denial_reasons_observed"][0])
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["enforcement_probe"]["artifacts"][0]
    assert "denial_reasons_observed must not contain duplicate values" in artifact["errors"]


def test_enforcement_denial_reasons_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = enforcement_probe()
    payload["denial_reasons_observed"].append("unknown-denial-reason")
    payload["denial_reason_count"] = len(payload["denial_reasons_observed"])
    write_json(tmp_path / "enforcement-probe.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["enforcement_probe"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "denial_reasons_observed must not include unknown values"
        in artifact["errors"]
    )


def test_honey_audit_requires_minimum_probe_count(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "honey-audit.json", honey_audit(probe_count=1))

    assert run_gate(tmp_path) == 1


def test_honey_audit_probe_count_must_match_unique_probes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = honey_audit()
    payload["honey_probe_count"] += 1
    write_json(tmp_path / "honey-audit.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["honey_audit"]["artifacts"][0]
    assert "honey_probe_count must match unique probes count" in artifact["errors"]


def test_honey_audit_probes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = honey_audit()
    payload["probes"].append({"name": "honey-probe-0"})
    payload["honey_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "honey-audit.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["honey_audit"]["artifacts"][0]
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert "honey_probe_count must match unique probes count" in artifact["errors"]


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
    assert "unknown required evidence kind" in captured.err
    assert "usage:" not in captured.err
    assert captured.out == ""


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(
        tmp_path / "unknown.json",
        {"schema": "sorafs.gateway_compliance.unknown.v1"},
    )

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1
