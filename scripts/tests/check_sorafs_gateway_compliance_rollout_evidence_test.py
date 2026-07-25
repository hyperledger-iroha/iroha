from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT = (
    Path(__file__).resolve().parents[1]
    / "check_sorafs_gateway_compliance_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location("gateway_compliance_gate", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

GENERATED_AT = 1_800_699_940
NOW = 1_800_700_000
CATALOG_DIGEST = "ab" * 32
PREDECESSOR_DIGEST = "cd" * 32
POLICY_DIGEST = "ef" * 32
PUBLICATION_DIGEST = "12" * 32
CONTROL_DIGEST = "34" * 32


def base(schema: str) -> dict:
    return {
        "schema": schema,
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": "sorafs-gateway-release-42",
        "environment": "production",
        "deployment_context_reviewed": True,
        "evidence_scope": "production",
        "catalog_digest_hex": CATALOG_DIGEST,
        "catalog_sequence": 8,
    }


def acknowledgements() -> list[dict]:
    return [
        {
            "gateway_id": "gateway-compliance-gateway-eu",
            "administration_id": "gateway-administration-eu",
            "region_id": "gateway-region-eu",
            "catalog_digest_hex": CATALOG_DIGEST,
            "acknowledged": True,
            "signature_verified": True,
            "acknowledged_at_unix": GENERATED_AT,
        },
        {
            "gateway_id": "gateway-compliance-gateway-apac",
            "administration_id": "gateway-administration-apac",
            "region_id": "gateway-region-apac",
            "catalog_digest_hex": CATALOG_DIGEST,
            "acknowledged": True,
            "signature_verified": True,
            "acknowledged_at_unix": GENERATED_AT,
        },
    ]


def catalog_promotion() -> dict:
    payload = base("sorafs.gateway_compliance.catalog_promotion_canary.v1")
    payload.update(
        {
            "predecessor_catalog_digest_hex": PREDECESSOR_DIGEST,
            "predecessor_catalog_sequence": 7,
            "promoted_catalog_digest_hex": CATALOG_DIGEST,
            "catalog_entry_count": 4,
            "catalog_entries": [
                {
                    "entry_id": "gateway-compliance-entry-baseline",
                    "entry_kind": "baseline_rule",
                    "source_id": "gateway-compliance-source-sanctions",
                },
                {
                    "entry_id": "gateway-compliance-entry-appeal",
                    "entry_kind": "accepted_appeal",
                    "source_id": "gateway-compliance-source-legal",
                },
                {
                    "entry_id": "gateway-compliance-entry-hold",
                    "entry_kind": "legal_safety_hold",
                    "source_id": "gateway-compliance-source-safety",
                },
                {
                    "entry_id": "gateway-compliance-entry-control",
                    "entry_kind": "scoped_toggle",
                    "source_id": "gateway-compliance-source-malware",
                },
            ],
            "catalog_change_count": 1,
            "catalog_changes": [
                {
                    "change_id": "gateway-compliance-entry-change-008",
                    "entry_id": "gateway-compliance-entry-baseline",
                    "operation": "replace",
                }
            ],
            "approval_threshold": 2,
            "approval_signer_count": 2,
            "approval_signer_ids": [
                "gateway-compliance-signer-alpha",
                "gateway-compliance-signer-beta",
            ],
            "catalog_signatures_verified": True,
            "gateway_ack_count": 2,
            "gateway_acknowledgements": acknowledgements(),
            "policy_digest_hex": POLICY_DIGEST,
        }
    )
    return payload


def controller_runtime() -> dict:
    payload = base("sorafs.gateway_compliance.controller_runtime_canary.v1")
    payload.update(
        {
            "predecessor_catalog_digest_hex": PREDECESSOR_DIGEST,
            "controller_instance_id": "gateway-compliance-controller-primary",
            "iroha_config_bound": True,
            "config_source": "iroha_config",
            "source_anchor_count": 4,
            "source_anchors": [
                {
                    "source_id": source,
                    "source_digest_hex": f"{index + 1:02x}" * 32,
                    "generated_at_unix": GENERATED_AT,
                    "signature_verified": True,
                }
                for index, source in enumerate(MODULE.REQUIRED_CONTROLLER_SOURCES)
            ],
            "controller_service_enabled": True,
            "catalog_signatures_verified": True,
            "predecessor_link_verified": True,
            "durable_history_reconciled": True,
            "last_known_good_available": True,
            "atomic_catalog_replacement": True,
        }
    )
    return payload


def moderation_toggle() -> dict:
    payload = base("sorafs.gateway_compliance.moderation_toggle_canary.v1")
    controls = [{"name": name} for name in MODULE.REQUIRED_MODERATION_CONTROLS]
    payload.update(
        {
            "control_api_url": "https://gateway-control.invalid/v1/sorafs/gateway/compliance",
            "control_count": len(controls),
            "approved_control_count": len(controls),
            "controls": controls,
            "control_digest_hex": CONTROL_DIGEST,
            "iroha_config_bound": True,
            "config_source": "iroha_config",
            "operator_role_enforced": True,
            "approval_workflow_enforced": True,
            "expiry_enforced": True,
            "catalog_reconciliation_observed": True,
            "operator_audit_trail_persisted": True,
        }
    )
    return payload


def gateway_reload() -> dict:
    payload = base("sorafs.gateway_compliance.gateway_reload_canary.v1")
    payload.update(
        {
            "predecessor_catalog_digest_hex": PREDECESSOR_DIGEST,
            "reload_ack_count": 2,
            "gateway_acknowledgements": acknowledgements(),
            "max_reload_latency_ms": 1_000,
            "atomic_catalog_replacement": True,
            "persisted_catalog_readback": True,
            "stale_catalog_rejected": True,
            "predecessor_mismatch_rejected": True,
            "rollback_catalog_digest_hex": PREDECESSOR_DIGEST,
            "rollback_available": True,
        }
    )
    return payload


def denial_record(name: str, source: str) -> dict:
    return {
        "name": name,
        "status_code": 451,
        "error": "gateway_compliance_denied",
        "source": source,
        "catalog_digest_hex": CATALOG_DIGEST,
        "cache_control": "private, no-store, max-age=0",
        "latency_ms": 42,
    }


def enforcement_probe() -> dict:
    payload = base("sorafs.gateway_compliance.enforcement_probe_canary.v1")
    routes = [
        denial_record("manifest", "baseline"),
        denial_record("cid", "legal_safety_hold"),
        denial_record("provider", "baseline"),
    ]
    payload.update(
        {
            "denial_sources_observed": ["baseline", "legal_safety_hold"],
            "denial_source_count": 2,
            "fail_closed_missing_catalog": True,
            "fail_closed_expired_catalog": True,
            "rate_limit_enforced": True,
            "route_count": len(routes),
            "routes": routes,
        }
    )
    return payload


def honey_audit() -> dict:
    payload = base("sorafs.gateway_compliance.honey_audit_canary.v1")
    probes = []
    for index, attack in enumerate(MODULE.REQUIRED_HONEY_ATTACKS):
        probe = denial_record(
            f"gateway-compliance-probe-{index + 1:02d}",
            "baseline" if index % 2 == 0 else "legal_safety_hold",
        )
        probe["probe_id"] = probe.pop("name")
        probe["attack"] = attack
        probes.append(probe)
    payload.update(
        {
            "probe_count": len(probes),
            "probes": probes,
            "attack_count": len(MODULE.REQUIRED_HONEY_ATTACKS),
            "attacks_observed": list(MODULE.REQUIRED_HONEY_ATTACKS),
        }
    )
    return payload


def precedence() -> dict:
    payload = base("sorafs.gateway_compliance.precedence_canary.v1")
    payload.update(
        {
            "case_count": 3,
            "cases": [
                {
                    "case_id": case_id,
                    "source": source,
                    "denied": denied,
                }
                for case_id, (source, denied) in MODULE.REQUIRED_PRECEDENCE_CASES.items()
            ],
            "finalized_chain_projection": True,
        }
    )
    return payload


def transparency_publication() -> dict:
    payload = base(
        "sorafs.gateway_compliance.transparency_publication_canary.v1"
    )
    payload.update(
        {
            "catalog_history_published": True,
            "catalog_acknowledgements_published": True,
            "moderation_events_published": True,
            "legal_hold_redaction_summaries_published": True,
            "governance_dag_bound": True,
            "publication_digest_hex": PUBLICATION_DIGEST,
        }
    )
    return payload


def observability() -> dict:
    payload = base("sorafs.gateway_compliance.observability_canary.v1")
    payload.update(
        {
            "metrics_scrape_success": True,
            "dashboard_provisioned": True,
            "alert_rules_installed": True,
            "critical_alerts_firing": False,
            "metrics": list(MODULE.REQUIRED_METRICS),
            "metric_count": len(MODULE.REQUIRED_METRICS),
        }
    )
    return payload


def governance_approval() -> dict:
    payload = base("sorafs.gateway_compliance.governance_approval.v1")
    payload.update(
        {
            "approved": True,
            "governance_vote_recorded": True,
            "iroha_config_bound": True,
            "config_source": "iroha_config",
            "catalog_policy_bound": True,
            "catalog_source_roster_bound": True,
            "transparency_policy_bound": True,
            "operator_roles_bound": True,
            "retention_policy_bound": True,
            "policy_digest_hex": POLICY_DIGEST,
        }
    )
    return payload


BUILDERS = {
    "catalog_promotion": catalog_promotion,
    "controller_runtime": controller_runtime,
    "moderation_toggle": moderation_toggle,
    "gateway_reload": gateway_reload,
    "enforcement_probe": enforcement_probe,
    "honey_audit": honey_audit,
    "precedence": precedence,
    "transparency_publication": transparency_publication,
    "observability": observability,
    "governance_approval": governance_approval,
}


def write_json(path: Path, payload: dict) -> None:
    path.write_text(json.dumps(payload, sort_keys=True), encoding="utf-8")


def write_all(root: Path) -> None:
    for kind, builder in BUILDERS.items():
        write_json(root / f"{kind}.json", builder())


def write_complete_evidence(root: Path) -> None:
    """Write the complete canonical lane fixture used by aggregate tests."""

    write_all(root)


def run_gate(root: Path, *extra: str) -> tuple[int, dict]:
    summary = root / "summary.json"
    summary.unlink(missing_ok=True)
    code = MODULE.main(
        [
            "--evidence-dir",
            str(root),
            "--summary-out",
            str(summary),
            "--now-unix",
            str(NOW),
            *extra,
        ]
    )
    rendered = (
        json.loads(summary.read_text(encoding="utf-8")) if summary.exists() else {}
    )
    return code, rendered


def run_one(
    root: Path, kind: str, payload: dict, *, include_anchor: bool = True
) -> tuple[int, dict]:
    if include_anchor and kind != "catalog_promotion":
        write_json(root / "catalog_promotion.json", catalog_promotion())
    write_json(root / f"{kind}.json", payload)
    return run_gate(root, "--require-kind", kind)


def artifact_errors(summary: dict, kind: str) -> list[str]:
    return summary["required"][kind]["artifacts"][0]["errors"]


def test_complete_canonical_evidence_is_ready(tmp_path: Path) -> None:
    write_all(tmp_path)
    code, summary = run_gate(tmp_path)
    assert code == 0
    assert summary["status"] == "ready"
    assert summary["valid_catalog_digests"] == [CATALOG_DIGEST]
    assert "valid_bundle_digests" not in summary
    assert summary["recognized_artifact_count"] == len(BUILDERS)


@pytest.mark.parametrize("kind,builder", BUILDERS.items())
def test_each_canonical_kind_is_individually_valid(
    tmp_path: Path, kind: str, builder
) -> None:
    payload = builder()
    code, summary = run_one(tmp_path, kind, payload)
    assert code == 0, summary["errors"]


@pytest.mark.parametrize(
    "legacy_field,value",
    [
        ("denylist_entries", []),
        ("bundle_digest_hex", CATALOG_DIGEST),
        ("proof_token_required", True),
        ("proof_token_verified", True),
        ("cache_version_binding_verified", True),
        ("x_sora_denylist_version", "8"),
        ("feed_signature_verified", True),
        ("denylist_override_scoped", True),
        ("json_report_generated", True),
    ],
)
def test_removed_fields_fail_closed(
    tmp_path: Path, legacy_field: str, value: object
) -> None:
    payload = enforcement_probe()
    payload[legacy_field] = value
    code, summary = run_one(tmp_path, "enforcement_probe", payload)
    assert code == 1
    assert any(
        "removed gateway-compliance V1 field" in error
        or "contains unknown fields" in error
        for error in artifact_errors(summary, "enforcement_probe")
    )


def test_removed_denial_code_fails_closed(tmp_path: Path) -> None:
    payload = enforcement_probe()
    payload["routes"][0]["error"] = "denylisted"
    code, summary = run_one(tmp_path, "enforcement_probe", payload)
    assert code == 1
    errors = artifact_errors(summary, "enforcement_probe")
    assert any("removed gateway-compliance denial code" in error for error in errors)


def test_denial_status_must_be_exact_http_451(tmp_path: Path) -> None:
    payload = enforcement_probe()
    payload["routes"][0]["status_code"] = 403
    code, summary = run_one(tmp_path, "enforcement_probe", payload)
    assert code == 1
    assert "routes[0].status_code must be exactly 451" in artifact_errors(
        summary, "enforcement_probe"
    )


def test_denial_digest_is_required_and_lowercase(tmp_path: Path) -> None:
    payload = enforcement_probe()
    del payload["routes"][0]["catalog_digest_hex"]
    code, summary = run_one(tmp_path, "enforcement_probe", payload)
    assert code == 1
    assert any(
        "catalog_digest_hex" in error
        for error in artifact_errors(summary, "enforcement_probe")
    )

    upper = enforcement_probe()
    upper["routes"][0]["catalog_digest_hex"] = CATALOG_DIGEST.upper()
    write_json(tmp_path / "enforcement_probe.json", upper)
    code, summary = run_gate(tmp_path, "--require-kind", "enforcement_probe")
    assert code == 1
    assert any(
        "catalog_digest_hex" in error
        for error in artifact_errors(summary, "enforcement_probe")
    )


def test_unknown_denial_source_fails_closed(tmp_path: Path) -> None:
    payload = enforcement_probe()
    payload["routes"][0]["source"] = "local_override"
    code, summary = run_one(tmp_path, "enforcement_probe", payload)
    assert code == 1
    assert "routes[0].source is not recognized" in artifact_errors(
        summary, "enforcement_probe"
    )


def test_split_gateway_catalog_fails_closed(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["gateway_acknowledgements"][1]["catalog_digest_hex"] = "11" * 32
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert any(
        "must match promoted catalog_digest_hex" in error
        for error in artifact_errors(summary, "catalog_promotion")
    )


def test_gateway_acknowledgements_require_independent_administrations(
    tmp_path: Path,
) -> None:
    payload = catalog_promotion()
    payload["gateway_acknowledgements"][1]["administration_id"] = (
        "gateway-administration-eu"
    )
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert (
        "gateway_acknowledgements must cover independently administered gateways"
        in artifact_errors(summary, "catalog_promotion")
    )


def test_gateway_acknowledgements_require_distinct_regions(
    tmp_path: Path,
) -> None:
    payload = catalog_promotion()
    payload["gateway_acknowledgements"][1]["region_id"] = "gateway-region-eu"
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert (
        "gateway_acknowledgements must cover distinct deployment regions"
        in artifact_errors(summary, "catalog_promotion")
    )


@pytest.mark.parametrize(
    "field,value,error_fragment",
    [
        ("acknowledged", False, "acknowledged"),
        ("signature_verified", False, "signature_verified"),
        ("acknowledged_at_unix", GENERATED_AT - 90_000, "older than"),
    ],
)
def test_gateway_acknowledgement_failures_are_rejected(
    tmp_path: Path, field: str, value: object, error_fragment: str
) -> None:
    payload = catalog_promotion()
    payload["gateway_acknowledgements"][0][field] = value
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert any(
        error_fragment in error
        for error in artifact_errors(summary, "catalog_promotion")
    )


def test_stale_artifact_fails_closed(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["generated_at_unix"] = GENERATED_AT - 90_000
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert any(
        "older than" in error for error in artifact_errors(summary, "catalog_promotion")
    )


@pytest.mark.parametrize(
    "mutation,error_fragment",
    [
        (
            lambda payload: payload.__setitem__(
                "predecessor_catalog_digest_hex", "0" * 64
            ),
            "must be non-zero",
        ),
        (
            lambda payload: payload.__setitem__(
                "predecessor_catalog_digest_hex", CATALOG_DIGEST
            ),
            "must differ",
        ),
        (
            lambda payload: payload.__setitem__("predecessor_catalog_sequence", 6),
            "immediately follow",
        ),
        (
            lambda payload: payload.__setitem__(
                "promoted_catalog_digest_hex", "44" * 32
            ),
            "must match catalog_digest_hex",
        ),
    ],
)
def test_predecessor_and_promotion_binding_fail_closed(
    tmp_path: Path, mutation, error_fragment: str
) -> None:
    payload = catalog_promotion()
    mutation(payload)
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert any(
        error_fragment in error
        for error in artifact_errors(summary, "catalog_promotion")
    )


def test_signature_quorum_and_uniqueness_fail_closed(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["approval_threshold"] = 3
    payload["approval_signer_ids"][1] = payload["approval_signer_ids"][0]
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    errors = artifact_errors(summary, "catalog_promotion")
    assert any("approval_threshold" in error for error in errors)
    assert any("duplicate" in error for error in errors)


def test_unverified_catalog_signature_fails_closed(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["catalog_signatures_verified"] = False
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert any(
        "catalog_signatures_verified" in error
        for error in artifact_errors(summary, "catalog_promotion")
    )


def test_catalog_entry_and_change_inventory_are_strict(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["catalog_entries"][0]["entry_id"] = "gateway-denylist-entry-old"
    payload["catalog_changes"][0]["operation"] = "override"
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    errors = artifact_errors(summary, "catalog_promotion")
    assert any("entry_id is not a canonical" in error for error in errors)
    assert any("operation is not recognized" in error for error in errors)


def test_catalog_anchor_mismatch_fails_closed(tmp_path: Path) -> None:
    write_json(tmp_path / "catalog_promotion.json", catalog_promotion())
    payload = controller_runtime()
    payload["catalog_digest_hex"] = "55" * 32
    write_json(tmp_path / "controller_runtime.json", payload)
    code, summary = run_gate(tmp_path, "--require-kind", "controller_runtime")
    assert code == 1
    assert any(
        "controller_runtime catalog_digest_hex must match a valid "
        "catalog_promotion catalog_digest_hex"
        in error
        for error in summary["errors"]
    )


def test_legacy_schema_is_not_recognized(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["schema"] = "sorafs.gateway_compliance.feed_promotion_canary.v1"
    legacy = tmp_path / "legacy.json"
    summary_path = tmp_path / "summary.json"
    write_json(legacy, payload)
    code = MODULE.main(
        [
            "--evidence",
            str(legacy),
            "--summary-out",
            str(summary_path),
            "--now-unix",
            str(NOW),
            "--require-kind",
            "catalog_promotion",
        ]
    )
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    assert code == 1
    assert any("schema is not a recognized" in error for error in summary["errors"])


def test_precedence_is_exact(tmp_path: Path) -> None:
    payload = precedence()
    payload["cases"][0]["source"] = "accepted_appeal"
    code, summary = run_one(tmp_path, "precedence", payload)
    assert code == 1
    assert any(
        "violates canonical precedence" in error
        for error in artifact_errors(summary, "precedence")
    )


def test_non_production_fixture_is_not_release_evidence(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["status"] = "non_production"
    payload["evidence_scope"] = "non_production_fixture"
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    errors = artifact_errors(summary, "catalog_promotion")
    assert any("status" in error or "evidence_scope" in error for error in errors)


def test_payload_bodies_are_rejected_without_leaking(tmp_path: Path, capsys) -> None:
    secret = "runtime-secret-material"
    payload = enforcement_probe()
    payload["response_body"] = secret
    code, summary = run_one(tmp_path, "enforcement_probe", payload)
    captured = capsys.readouterr()
    assert code == 1
    assert secret not in captured.err
    assert secret not in json.dumps(summary)


def test_unknown_top_level_field_fails_closed(tmp_path: Path) -> None:
    payload = catalog_promotion()
    payload["locally_verified"] = True
    code, summary = run_one(
        tmp_path, "catalog_promotion", payload, include_anchor=False
    )
    assert code == 1
    assert any(
        "contains unknown fields" in error
        for error in artifact_errors(summary, "catalog_promotion")
    )


def test_unknown_required_kind_is_usage_error(tmp_path: Path) -> None:
    write_all(tmp_path)
    code, _summary = run_gate(tmp_path, "--require-kind", "feed_promotion")
    assert code == 2
