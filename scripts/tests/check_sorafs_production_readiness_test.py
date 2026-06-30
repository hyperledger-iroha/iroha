"""Tests for scripts/check_sorafs_production_readiness.py."""

from __future__ import annotations

import copy
import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("check_sorafs_production_readiness", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


NOW_UNIX = 1_800_800_000
GENERATED_AT = NOW_UNIX - 120
SHA256 = "ab" * 32
SNAPSHOT_ID = "cd" * 16
DEPLOYMENT_ID = "sorafs-mainnet-2026-06"
ENVIRONMENT = "production"


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def gate_summary(
    gate_name: str,
    *,
    status: str = "ready",
    errors: list[str] | None = None,
    generated_at_unix: int = GENERATED_AT,
    deployment_id: str = DEPLOYMENT_ID,
    environment: str = ENVIRONMENT,
    raw_response: bool = False,
    required_kinds: list[str] | None = None,
) -> dict:
    gate = MODULE.GATE_BY_NAME[gate_name]
    gate_required_kinds = (
        list(gate.required_kinds) if required_kinds is None else required_kinds
    )
    required_rows = {}
    for kind_name in gate_required_kinds:
        required_rows[kind_name] = {
            "schema": f"{gate.schema}.{kind_name}",
            "present": True,
            "valid": True,
            "artifact_count": 1,
            "artifacts": [
                {
                    "path": f"artifacts/{gate_name}/{kind_name}.json",
                    "sha256": SHA256,
                    "schema": f"{gate.schema}.{kind_name}",
                    "status": "passed",
                    "fingerprint": {
                        "generated_at_unix": generated_at_unix,
                        "deployment_id": deployment_id,
                        "environment": environment,
                        "deployment_context_reviewed": True,
                    },
                    "valid": True,
                    "errors": [],
                }
            ],
            "errors": [],
        }
    payload = {
        "schema": gate.schema,
        "status": status,
        "required_kinds": gate_required_kinds,
        "thresholds": {"max_evidence_bytes": 2_097_152},
        "evidence_file_count": len(gate_required_kinds),
        "recognized_artifact_count": len(gate_required_kinds),
        "recognized_artifacts": recognized_artifacts_from_required(
            {"required": required_rows}
        ),
        "required": required_rows,
        "errors": [] if errors is None else errors,
    }
    if raw_response:
        payload["response_body"] = "leaked"
    return payload


def write_gate(root: Path, gate_name: str, **kwargs: object) -> Path:
    return write_json(root / f"{gate_name}.json", gate_summary(gate_name, **kwargs))


def write_all_gates(root: Path) -> None:
    for gate_name in MODULE.DEFAULT_REQUIRED_GATES:
        write_gate(root, gate_name)


def recognized_artifacts_from_required(payload: dict) -> list[dict]:
    artifacts = []
    for kind_name, row in payload["required"].items():
        for required_artifact in row["artifacts"]:
            artifact = dict(required_artifact)
            artifact["kind"] = kind_name
            artifacts.append(artifact)
    return artifacts


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(
        [
            "--evidence-dir",
            str(root),
            "--now-unix",
            str(NOW_UNIX),
            "--deployment-id",
            DEPLOYMENT_ID,
            "--environment",
            ENVIRONMENT,
            *extra,
        ]
    )


def test_payload_free_summary_metadata_fields_are_derived_from_gate_contracts() -> None:
    expected = frozenset().union(*MODULE.GATE_METADATA_FIELDS.values())

    assert MODULE.PAYLOAD_FREE_SUMMARY_METADATA_FIELDS == expected
    assert MODULE.PAYLOAD_FREE_SUMMARY_FIELDS == (
        MODULE.PAYLOAD_FREE_SUMMARY_CORE_FIELDS | expected
    )
    assert set(MODULE.GATE_METADATA_FIELDS) == set(MODULE.GATE_BY_NAME)


def test_complete_aggregate_readiness_passes(tmp_path: Path) -> None:
    write_all_gates(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == MODULE.SUMMARY_SCHEMA
    assert payload["status"] == "ready"
    assert payload["recognized_summary_count"] == len(MODULE.DEFAULT_REQUIRED_GATES)
    assert payload["deployment"] == {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    assert payload["required"]["gateway_load"]["valid"] is True
    assert payload["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    args = tmp_path / "aggregate.args"
    args.write_text(
        "\n".join(
            [
                f"--evidence-dir {tmp_path}",
                "--require-gate gateway_load",
                f"--now-unix {NOW_UNIX}",
                f"--deployment-id {DEPLOYMENT_ID}",
                f"--environment {ENVIRONMENT}",
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_required_gate_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")

    assert run_gate(tmp_path) == 1


def test_explicit_unrequired_gate_summary_fails(tmp_path: Path) -> None:
    gateway_load = write_gate(tmp_path, "gateway_load")
    reputation = write_gate(tmp_path, "reputation")
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence",
                str(gateway_load),
                "--evidence",
                str(reputation),
                "--require-gate",
                "gateway_load",
                "--now-unix",
                str(NOW_UNIX),
                "--deployment-id",
                DEPLOYMENT_ID,
                "--environment",
                ENVIRONMENT,
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "explicit production readiness summary belongs to unrequired gate"
        in errors
    )
    assert (
        result["errors"].count(
            "explicit production readiness summary belongs to unrequired gate"
        )
        == 1
    )
    assert MODULE.GATE_BY_NAME["reputation"].schema not in errors
    assert "reputation` gate" not in errors

    drift_errors: list[str] = []
    MODULE.validate_disallowed_summary_diagnostics(
        drift_errors,
        unknown_schema_count=0,
        explicit_unrequired_count=1,
    )
    assert (
        "aggregate summary unrequired-gate diagnostics must match explicit unrequired summaries"
        in "\n".join(drift_errors)
    )


def test_unknown_sorafs_schema_in_summary_dir_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    unknown_schema = "sorafs.unknown.private-key-placeholder.v1"
    unknown_path = tmp_path / "unknown.json"
    write_json(
        unknown_path,
        {
            "schema": unknown_schema,
            "status": "ready",
            "errors": [],
        },
    )
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(payload["errors"])
    assert "unknown SoraFS readiness summary schema" in errors
    assert payload["errors"].count("unknown SoraFS readiness summary schema") == 1
    assert unknown_schema not in errors
    assert str(unknown_path) not in errors

    drift_errors: list[str] = []
    MODULE.validate_disallowed_summary_diagnostics(
        drift_errors,
        unknown_schema_count=1,
        explicit_unrequired_count=0,
    )
    assert (
        "aggregate summary unknown-schema diagnostics must match discovered unknown summaries"
        in "\n".join(drift_errors)
    )


def test_unknown_non_sorafs_schema_in_summary_dir_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_json(
        tmp_path / "unrelated.json",
        {
            "schema": "unrelated.summary.v1",
            "status": "ready",
            "errors": [],
        },
    )

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_blocked_lane_summary_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load", status="blocked", errors=["lane failed"])

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_with_load_errors_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["load_errors"] = ["failed to parse skipped-evidence.json"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_lane_summary_load_errors_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["load_errors"] = "failed to parse skipped-evidence.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_lane_summary_thresholds_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = ["max_artifact_age_secs=86400"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_missing_lane_summary_thresholds_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    del payload["thresholds"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_empty_lane_summary_thresholds_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_threshold_keys_must_be_canonical(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {" max_evidence_age_secs": 86_400}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_lane_summary_threshold_values_must_be_non_negative_int(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {"max_evidence_age_secs": {"seconds": 86_400}}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_threshold_key_value_diagnostic_is_sanitized(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {"bad\nkey": False}
    write_json(tmp_path / "gateway_load.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "thresholds.<invalid> must be a non-negative integer" in errors
    assert "bad\nkey" not in errors


def test_malformed_threshold_entries_are_not_carried_into_summary(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"] = {
        "max_evidence_bytes": 2_097_152,
        " bad_key": 5,
        "nested": {"value": 1},
    }
    write_json(tmp_path / "gateway_load.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }
    errors = "\n".join(result["errors"])
    assert "thresholds keys must be canonical strings" in errors
    assert "thresholds.nested must be a non-negative integer" in errors


def test_stale_artifact_timestamp_fails(tmp_path: Path) -> None:
    write_gate(
        tmp_path,
        "gateway_load",
        generated_at_unix=NOW_UNIX - MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS - 1,
    )

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_required_artifact_digest_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["artifacts"][0]["sha256"] = "AB" * 32
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_required_artifact_metadata_label_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["artifacts"][0]["schema"] = "\ninvalid"
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_required_row_schema_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["schema"] = " padded-schema "
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_duplicate_required_artifact_identities_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    duplicate = dict(payload["required"][first_required]["artifacts"][0])
    payload["required"][first_required]["artifacts"].append(duplicate)
    payload["required"][first_required]["artifact_count"] = 2
    payload["recognized_artifact_count"] += 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_deployment_mismatch_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    write_gate(tmp_path, "reputation", deployment_id="different-deployment")

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--require-gate",
            "reputation",
        )
        == 1
    )


def test_sensitive_summary_payload_fails(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load", raw_response=True)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_sensitive_summary_key_diagnostic_is_sanitized(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["private\nkey"] = "runtime-only-private-key"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "<sensitive-key> must not be present" in errors
    assert "private\nkey" not in errors


def test_sensitive_summary_key_diagnostic_sanitizes_canonical_key(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["private_key"] = "runtime-only-key-material"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "<sensitive-key> must not be present" in errors
    assert "<sensitive-key> is not allowed in payload-free lane summary" in errors
    assert "private_key" not in errors
    assert "runtime-only-key-material" not in errors


def test_sensitive_threshold_key_is_not_carried_into_summary(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["thresholds"]["private_key"] = 1
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "thresholds.<sensitive-key> must not be present" in errors
    assert result["required"]["gateway_load"]["thresholds"] == {
        "max_evidence_bytes": 2_097_152,
    }
    assert "private_key" not in errors


def test_sensitive_required_and_artifact_field_diagnostics_are_sanitized(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["private_key"] = "row-key-material"
    payload["required"][first_kind]["artifacts"][0][
        "private_key"
    ] = "artifact-key-material"
    payload["recognized_artifacts"][0]["private_key"] = "recognized-key-material"
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        ".<sensitive-key> is not allowed in payload-free required row"
        in errors
    )
    assert (
        ".<sensitive-key> is not allowed in payload-free artifact summary"
        in errors
    )
    assert "private_key" not in errors
    assert "row-key-material" not in errors
    assert "artifact-key-material" not in errors
    assert "recognized-key-material" not in errors


def test_extra_top_level_lane_summary_field_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["debug_report"] = {"note": "not part of the payload-free contract"}
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "debug_report is not allowed in payload-free lane summary"
        in "\n".join(result["errors"])
    )


def test_allowed_top_level_lane_metadata_shape_is_validated(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = {"digest": SHA256}
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "valid_suite_report_digests must be a payload-free metadata list"
        in "\n".join(result["errors"])
    )


def test_allowed_top_level_lane_metadata_rejects_nested_raw_payload(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = [{"raw": "leak"}]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_suite_report_digests[0].<sensitive-key> must not be present"
        in errors
    )
    assert "valid_suite_report_digests[0].raw must not be present" not in errors


def test_allowed_top_level_lane_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_suite_report_digests"] = [SHA256]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 0


def test_digest_list_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    bad_digest = "not-a-digest"
    payload["valid_suite_report_digests"] = [bad_digest]
    payload["valid_staging_report_digests"] = ["AB" * 32, {"digest": SHA256}]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_suite_report_digests[0] must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_staging_report_digests[0] must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_staging_report_digests[1] must be 64 lowercase hex characters"
        in errors
    )
    assert bad_digest not in errors
    assert "AB" * 32 not in errors


def test_digest_list_metadata_entries_must_be_unique_and_sorted(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    lower_digest = "00" * 32
    payload["valid_suite_report_digests"] = [SHA256, lower_digest]
    payload["valid_staging_report_digests"] = [SHA256, SHA256]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_suite_report_digests must be sorted in canonical order" in errors
    assert (
        "valid_staging_report_digests must not contain duplicate metadata entries"
        in errors
    )
    assert lower_digest not in errors
    assert SHA256 not in errors


def test_hex_binding_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("ai_prescreen")
    payload["valid_runner_bindings"] = [
        {
            "manifest_id_hex": "12" * 16,
            "runner_hash_hex": SHA256,
            "subject_digest_hex": SHA256,
        }
    ]
    payload["valid_workflow_digests"] = [SHA256]
    write_json(tmp_path / "ai_prescreen.json", payload)

    assert run_gate(tmp_path, "--require-gate", "ai_prescreen") == 0


def test_hex_binding_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    bad_snapshot_id = "AB" * 16
    payload["valid_snapshot_bindings"] = [
        {
            "snapshot_id_hex": bad_snapshot_id,
            "merkle_root_hex": SHA256,
            "raw_response": "runtime-only-body",
        },
        {"snapshot_id_hex": SNAPSHOT_ID},
        "not-a-binding",
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_snapshot_bindings[0].snapshot_id_hex must be 32 lowercase hex characters"
        in errors
    )
    assert (
        "valid_snapshot_bindings[0].<sensitive-key> is not allowed in payload-free binding metadata"
        in errors
    )
    assert (
        "valid_snapshot_bindings[1].merkle_root_hex must be 64 lowercase hex characters"
        in errors
    )
    assert "valid_snapshot_bindings[2] must be a payload-free binding object" in errors
    assert bad_snapshot_id not in errors
    assert "raw_response" not in errors
    assert "runtime-only-body" not in errors
    assert "not-a-binding" not in errors


def test_binding_metadata_entries_must_be_unique_and_sorted(
    tmp_path: Path,
) -> None:
    high_binding = {
        "snapshot_id_hex": "ff" * 16,
        "merkle_root_hex": SHA256,
    }
    low_binding = {
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": SHA256,
    }
    payload = gate_summary("reputation")
    payload["valid_snapshot_bindings"] = [high_binding, low_binding, low_binding]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_snapshot_bindings must be sorted in canonical order" in errors
    assert (
        "valid_snapshot_bindings must not contain duplicate metadata entries"
        in errors
    )
    assert "ff" * 16 not in errors
    assert SNAPSHOT_ID not in errors


def test_reference_decision_id_metadata_entries_are_validated(
    tmp_path: Path,
) -> None:
    payload = gate_summary("hedging_billing")
    bad_decision_id = "decision-private-key-placeholder"
    payload["valid_reference_decision_ids"] = [SHA256, bad_decision_id, "AB" * 32]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "hedging_billing.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "hedging_billing",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_reference_decision_ids[1] must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_reference_decision_ids[2] must be 64 lowercase hex characters"
        in errors
    )
    assert bad_decision_id not in errors
    assert "AB" * 32 not in errors


def test_public_head_cid_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("governance_dag")
    bad_public_head = "cid-private-key-placeholder"
    payload["valid_public_head_cids"] = [SHA256, bad_public_head, "AB" * 32]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "governance_dag.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "governance_dag",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_public_head_cids[1] must be 64 lowercase hex characters" in errors
    assert "valid_public_head_cids[2] must be 64 lowercase hex characters" in errors
    assert bad_public_head not in errors
    assert "AB" * 32 not in errors


def test_provider_count_values_metadata_entries_are_positive_ints(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["provider_count_values"] = [2, 0, -1, True, "3"]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "provider_count_values[1] must be a positive integer" in errors
    assert "provider_count_values[2] must be a positive integer" in errors
    assert "provider_count_values[3] must be a positive integer" in errors
    assert "provider_count_values[4] must be a positive integer" in errors
    assert '"3"' not in errors


def test_provider_count_values_metadata_must_be_unique_and_sorted(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reputation")
    payload["provider_count_values"] = [3, 2, 2]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "provider_count_values must be sorted in canonical order" in errors
    assert "provider_count_values must not contain duplicate metadata entries" in errors


def test_object_list_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    payload["valid_multi_peer_runs"] = [
        {
            "generated_at_unix": GENERATED_AT,
            "peer_count": 4,
            "validator_count": 4,
            "case_count": 2,
            "config_digest_hex": SHA256,
        }
    ]
    payload["valid_config_digests"] = [SHA256]
    write_json(tmp_path / "appeal_finance.json", payload)

    assert run_gate(tmp_path, "--require-gate", "appeal_finance") == 0


def test_object_list_metadata_entries_are_validated(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    bad_digest = "AB" * 32
    payload["valid_multi_peer_runs"] = [
        {
            "generated_at_unix": GENERATED_AT,
            "peer_count": 0,
            "validator_count": True,
            "case_count": "2",
            "config_digest_hex": bad_digest,
            "private_key": "runtime-only-key-material",
        },
        "not-a-run",
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "valid_multi_peer_runs[0].peer_count must be a positive integer" in errors
    assert (
        "valid_multi_peer_runs[0].validator_count must be a positive integer"
        in errors
    )
    assert "valid_multi_peer_runs[0].case_count must be a positive integer" in errors
    assert (
        "valid_multi_peer_runs[0].config_digest_hex must be 64 lowercase hex characters"
        in errors
    )
    assert (
        "valid_multi_peer_runs[0].<sensitive-key> is not allowed in payload-free object metadata"
        in errors
    )
    assert "valid_multi_peer_runs[1] must be a payload-free metadata object" in errors
    assert bad_digest not in errors
    assert "private_key" not in errors
    assert "runtime-only-key-material" not in errors
    assert "not-a-run" not in errors


def test_object_list_metadata_entries_must_not_duplicate(tmp_path: Path) -> None:
    payload = gate_summary("appeal_finance")
    run = {
        "generated_at_unix": GENERATED_AT,
        "peer_count": 4,
        "validator_count": 4,
        "case_count": 2,
        "config_digest_hex": SHA256,
    }
    payload["valid_multi_peer_runs"] = [dict(run), dict(run)]
    payload["valid_config_digests"] = [SHA256]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "appeal_finance.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "appeal_finance",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "valid_multi_peer_runs must not contain duplicate metadata entries"
        in errors
    )
    assert SHA256 not in errors


def test_provider_bake_metadata_completed_at_must_not_precede_start(
    tmp_path: Path,
) -> None:
    payload = gate_summary("reserve_rent")
    payload["valid_policy_digests"] = [SHA256]
    payload["valid_provider_bakes"] = [
        {
            "bake_id": "reserve-bake-001",
            "started_at_unix": GENERATED_AT,
            "completed_at_unix": GENERATED_AT - 1,
            "provider_count": 3,
        }
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reserve_rent.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reserve_rent",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "valid_provider_bakes[0].completed_at_unix must be >= started_at_unix"
        in "\n".join(result["errors"])
    )


def test_deployment_context_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("moderation_panel")
    payload["deployment_context"] = {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
    }
    payload["valid_case_digests"] = [SHA256]
    write_json(tmp_path / "moderation_panel.json", payload)

    assert run_gate(tmp_path, "--require-gate", "moderation_panel") == 0


def test_deployment_context_metadata_entries_are_validated(
    tmp_path: Path,
) -> None:
    payload = gate_summary("moderation_panel")
    payload["deployment_context"] = {
        "deployment_id": " runtime-only-deployment",
        "private_key": "runtime-only-key-material",
    }
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "moderation_panel.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "moderation_panel",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "deployment_context.deployment_id must be a canonical string" in errors
    assert "deployment_context.environment must be a canonical string" in errors
    assert (
        "deployment_context.<sensitive-key> is not allowed in payload-free object metadata"
        in errors
    )
    assert "runtime-only-deployment" not in errors
    assert "private_key" not in errors
    assert "runtime-only-key-material" not in errors


def test_reputation_top_level_hex_metadata_for_gate_passes(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    payload["snapshot_id_hex"] = SNAPSHOT_ID
    payload["merkle_root_hex"] = SHA256
    write_json(tmp_path / "reputation.json", payload)

    assert run_gate(tmp_path, "--require-gate", "reputation") == 0


def test_reputation_top_level_hex_metadata_is_validated(tmp_path: Path) -> None:
    payload = gate_summary("reputation")
    bad_snapshot_id = "not-a-snapshot-id"
    bad_merkle_root = "AB" * 32
    payload["snapshot_id_hex"] = bad_snapshot_id
    payload["merkle_root_hex"] = bad_merkle_root
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "reputation.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "reputation",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "snapshot_id_hex must be 32 lowercase hex characters" in errors
    assert "merkle_root_hex must be 64 lowercase hex characters" in errors
    assert bad_snapshot_id not in errors
    assert bad_merkle_root not in errors


def test_cross_lane_top_level_lane_metadata_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["valid_snapshot_bindings"] = [
        {
            "snapshot_id_hex": SHA256,
            "merkle_root_hex": SHA256,
        }
    ]
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        "valid_snapshot_bindings is not allowed for `gateway_load` lane metadata"
        in "\n".join(result["errors"])
    )


def test_narrowed_lane_summary_fails(tmp_path: Path) -> None:
    gate = MODULE.GATE_BY_NAME["gateway_load"]
    write_gate(tmp_path, "gateway_load", required_kinds=["local_conformance"])
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "required_kinds missing full-contract kinds" in errors
    for missing_kind in set(gate.required_kinds) - {"local_conformance"}:
        assert missing_kind not in errors


def test_extra_required_kind_labels_are_payload_free(tmp_path: Path) -> None:
    gate = MODULE.GATE_BY_NAME["gateway_load"]
    hidden_kind = "shadow_optional_row"
    required_kinds = list(gate.required_kinds) + [hidden_kind, hidden_kind]
    payload = gate_summary("gateway_load", required_kinds=required_kinds)
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "required_kinds contains duplicate kind" in errors
    assert "required_kinds contains unknown full-contract kinds" in errors
    assert hidden_kind not in errors


def test_extra_required_row_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    extra_row = dict(payload["required"][first_required])
    extra_row["schema"] = f"{payload['schema']}.hidden_optional"
    payload["required"]["hidden_optional"] = extra_row
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_extra_required_row_label_is_payload_free(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    hidden_row = "shadow_optional_row"
    extra_row = dict(payload["required"][first_required])
    extra_row["schema"] = f"{payload['schema']}.hidden_optional"
    payload["required"][hidden_row] = extra_row
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert (
        "required contains rows outside the full `gateway_load` gate contract"
        in errors
    )
    assert hidden_row not in errors


def test_malformed_extra_required_row_label_is_sanitized(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    extra_row = dict(payload["required"][first_required])
    extra_row["schema"] = f"{payload['schema']}.hidden_optional"
    payload["required"]["hidden\noptional"] = extra_row
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "required row labels must be canonical strings" in errors
    assert (
        "required contains rows outside the full `gateway_load` gate contract"
        in errors
    )
    assert "hidden\noptional" not in errors


def test_extra_required_row_field_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_required = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    payload["required"][first_required]["payload"] = {"raw": "leak"}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifact_count_mismatch_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifact_count"] += 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_evidence_file_count_exceeds_artifacts_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["evidence_file_count"] = payload["recognized_artifact_count"] + 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_evidence_file_count_must_match_recognized_artifact_paths(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    payload["evidence_file_count"] = 1
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_top_level_counts_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["evidence_file_count"] = 0
    payload["recognized_artifact_count"] = True
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_invalid_top_level_recognized_artifacts_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifacts"][0]["valid"] = False
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_artifact_extra_fields_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["payload"] = {"raw": "leak"}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_artifact_kind_mismatch_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["kind"] = "wrong_kind"
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_required_artifact_duplicate_paths_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    first_artifact = payload["required"][first_kind]["artifacts"][0]
    duplicate_path_artifact = dict(first_artifact)
    duplicate_path_artifact["sha256"] = "cd" * 32
    duplicate_path_artifact["fingerprint"] = dict(first_artifact["fingerprint"])
    duplicate_path_artifact["fingerprint"]["generated_at_unix"] = GENERATED_AT - 1
    payload["required"][first_kind]["artifacts"].append(duplicate_path_artifact)
    payload["required"][first_kind]["artifact_count"] = 2
    payload["recognized_artifact_count"] += 1
    payload["recognized_artifacts"] = recognized_artifacts_from_required(payload)
    payload["evidence_file_count"] = len(
        {artifact["path"] for artifact in payload["recognized_artifacts"]}
    )
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert ".artifacts must not duplicate artifact paths" in errors
    assert "recognized_artifacts must not duplicate artifact paths" in errors


def test_recognized_artifact_extra_fields_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifacts"][0]["payload"] = {"raw": "leak"}
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_missing_top_level_recognized_artifacts_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    del payload["recognized_artifacts"]
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_malformed_recognized_artifact_metadata_label_fails(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    payload["recognized_artifacts"][0]["status"] = " padded-status "
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifacts_must_match_required_kind_counts(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["recognized_artifacts"][0]["kind"]
    replaced_kind = payload["recognized_artifacts"][-1]["kind"]
    artifacts = payload["recognized_artifacts"]
    artifacts[-1] = dict(artifacts[0])
    payload["recognized_artifacts"] = artifacts
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    errors = "\n".join(result["errors"])
    assert "recognized_artifacts must match required artifact counts" in errors
    assert "{'required':" not in errors
    assert first_kind not in errors
    assert replaced_kind not in errors


def test_recognized_artifacts_must_match_required_artifact_identities(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    artifacts = payload["recognized_artifacts"]
    artifacts[0] = dict(artifacts[0])
    artifacts[0]["sha256"] = "cd" * 32
    payload["recognized_artifacts"] = artifacts
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifacts_must_match_required_artifact_metadata(
    tmp_path: Path,
) -> None:
    payload = gate_summary("gateway_load")
    artifacts = payload["recognized_artifacts"]
    artifacts[0] = dict(artifacts[0])
    artifacts[0]["fingerprint"] = dict(artifacts[0]["fingerprint"])
    artifacts[0]["fingerprint"]["generated_at_unix"] = GENERATED_AT + 1
    payload["recognized_artifacts"] = artifacts
    write_json(tmp_path / "gateway_load.json", payload)

    assert run_gate(tmp_path, "--require-gate", "gateway_load") == 1


def test_recognized_artifacts_duplicate_paths_fail(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    artifacts = payload["recognized_artifacts"]
    artifacts[0] = dict(artifacts[0])
    artifacts[0]["path"] = artifacts[1]["path"]
    artifacts[0]["sha256"] = artifacts[1]["sha256"]
    artifacts[0]["fingerprint"] = dict(artifacts[1]["fingerprint"])
    payload["recognized_artifacts"] = artifacts
    payload["evidence_file_count"] = len({artifact["path"] for artifact in artifacts})
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert "recognized_artifacts must not duplicate artifact paths" in "\n".join(
        result["errors"]
    )


def test_artifact_fingerprint_metadata_must_be_payload_free(tmp_path: Path) -> None:
    payload = gate_summary("gateway_load")
    first_kind = payload["required_kinds"][0]
    payload["required"][first_kind]["artifacts"][0]["fingerprint"]["optional"] = None
    payload["recognized_artifacts"][0]["fingerprint"]["optional"] = None
    summary = tmp_path / "summary.json"
    write_json(tmp_path / "gateway_load.json", payload)

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert (
        ".fingerprint.optional must contain only payload-free canonical metadata"
        in "\n".join(result["errors"])
    )


def test_aggregate_gate_row_output_shape_is_validated() -> None:
    gate = MODULE.GATE_BY_NAME["gateway_load"]
    payload = gate_summary("gateway_load")
    row, validation_errors = MODULE.validate_gate_summary(
        gate,
        payload,
        MODULE.ValidationOptions(
            now_unix=NOW_UNIX,
            max_summary_artifact_age_secs=MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
        ),
    )
    assert validation_errors == []
    row["path"] = "gateway_load.json"
    row["sha256"] = SHA256

    errors: list[str] = []
    MODULE.validate_aggregate_gate_row_output(gate, row, errors)
    assert errors == []

    row["private_key"] = "runtime-only-key-material"
    row["sha256"] = "AB" * 32
    row["expected_required_kinds"] = list(reversed(row["expected_required_kinds"]))
    row["newest_generated_at_unix"] = row["oldest_generated_at_unix"] - 1
    row["errors"] = ["row drifted"]
    MODULE.validate_aggregate_gate_row_output(gate, row, errors)
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate row fields must match the schema-closed output contract"
        in diagnostics
    )
    assert "gateway_load aggregate row <sensitive-key> is not allowed" in diagnostics
    assert (
        "gateway_load aggregate row sha256 must be canonical lowercase SHA-256"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row expected_required_kinds must match gate contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate row newest_generated_at_unix must be >= oldest_generated_at_unix"
        in diagnostics
    )
    assert "gateway_load aggregate row errors must be empty" in diagnostics
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "AB" * 32 not in diagnostics


def test_aggregate_summary_output_shape_is_validated(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    options = MODULE.ValidationOptions(
        now_unix=NOW_UNIX,
        max_summary_artifact_age_secs=MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
        deployment_id=DEPLOYMENT_ID,
        environment=ENVIRONMENT,
    )
    summary, build_errors = MODULE.build_summary(
        [tmp_path],
        [],
        ("gateway_load",),
        options,
        None,
    )
    assert build_errors == []
    assert summary["status"] == "ready"

    errors: list[str] = []
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    assert errors == []

    summary["status"] = "blocked"
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary status must match aggregate diagnostics" in diagnostics

    errors = []
    summary["status"] = "ready"
    summary["errors"] = ["drifted aggregate diagnostic"]
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary status must match aggregate diagnostics" in diagnostics

    errors = []
    summary["status"] = "ready"
    summary["errors"] = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["deployment"] = {}
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary ready deployment must include deployment_id and environment"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["recognized_summary_count"] = 0
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary ready recognized_summary_count must match required gate count"
        in diagnostics
    )

    errors = []
    ready_summary = copy.deepcopy(summary)
    ready_summary["required"]["gateway_load"]["valid"] = False
    ready_summary["required"]["gateway_load"]["errors"] = ["invalid required row"]
    MODULE.validate_aggregate_summary_output(ready_summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert "aggregate summary ready rows must all be present and valid" in diagnostics

    errors = []
    summary["private_key"] = "runtime-only-key-material"
    summary["status"] = "done"
    summary["required_gates"] = ["gateway_load", "shadow_gate"]
    summary["recognized_summary_count"] = 0
    summary["deployment"] = {
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "private_key": "runtime-only-key-material",
    }
    summary["errors"] = ["bad\nerror"]
    MODULE.validate_aggregate_summary_output(summary, ("gateway_load",), errors)
    diagnostics = "\n".join(errors)
    assert (
        "aggregate summary fields must match the schema-closed output contract"
        in diagnostics
    )
    assert "aggregate summary <sensitive-key> is not allowed" in diagnostics
    assert "aggregate summary status must be ready, failed, or blocked" in diagnostics
    assert "aggregate summary required_gates must match requested gates" in diagnostics
    assert (
        "aggregate summary recognized_summary_count must match present required rows"
        in diagnostics
    )
    assert (
        "aggregate summary deployment fields must be deployment_id and environment"
        in diagnostics
    )
    assert "aggregate summary deployment <sensitive-key> is not allowed" in diagnostics
    assert "aggregate summary errors must contain canonical strings" in diagnostics
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "bad\nerror" not in diagnostics


def test_aggregate_required_row_output_shape_is_validated(tmp_path: Path) -> None:
    write_gate(tmp_path, "gateway_load")
    options = MODULE.ValidationOptions(
        now_unix=NOW_UNIX,
        max_summary_artifact_age_secs=MODULE.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS,
        deployment_id=DEPLOYMENT_ID,
        environment=ENVIRONMENT,
    )
    summary, build_errors = MODULE.build_summary(
        [tmp_path],
        [],
        ("gateway_load", "reputation"),
        options,
        None,
    )
    assert summary["status"] == "blocked"
    assert build_errors == [
        "missing required reputation production readiness summary",
    ]

    errors: list[str] = []
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        summary["required"]["gateway_load"],
        errors,
    )
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["reputation"],
        summary["required"]["reputation"],
        errors,
    )
    assert errors == []

    missing_row = dict(summary["required"]["reputation"])
    missing_row["errors"] = [
        "missing required reputation production readiness summary",
        "private-key-placeholder drift",
    ]
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["reputation"],
        missing_row,
        errors,
    )

    summary["required"]["gateway_load"]["private_key"] = "runtime-only-key-material"
    summary["required"]["gateway_load"]["valid"] = False
    summary["required"]["gateway_load"]["errors"] = []
    summary["required"]["gateway_load"]["sha256"] = "AB" * 32
    summary["required"]["gateway_load"]["thresholds"] = {"bad\nkey": False}
    summary["required"]["gateway_load"]["oldest_generated_at_unix"] = GENERATED_AT + 1
    summary["required"]["gateway_load"]["newest_generated_at_unix"] = GENERATED_AT
    summary["required"]["gateway_load"]["deployment_id"] = " runtime-only-deployment"
    summary["required"]["gateway_load"]["environment"] = "prod\nsecret"
    summary["required"]["reputation"]["present"] = True
    summary["required"]["reputation"]["private_key"] = "runtime-only-key-material"
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["gateway_load"],
        summary["required"]["gateway_load"],
        errors,
    )
    MODULE.validate_aggregate_required_row_output(
        MODULE.GATE_BY_NAME["reputation"],
        summary["required"]["reputation"],
        errors,
    )
    diagnostics = "\n".join(errors)
    assert (
        "gateway_load aggregate required row fields must match the schema-closed output contract"
        in diagnostics
    )
    assert (
        "gateway_load aggregate required row <sensitive-key> is not allowed"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row sha256 must be canonical lowercase SHA-256"
        in diagnostics
    )
    assert "gateway_load aggregate invalid row errors must not be empty" in diagnostics
    assert (
        "gateway_load aggregate invalid row newest_generated_at_unix must be >= oldest_generated_at_unix"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row thresholds.<invalid> must be a non-negative integer"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row deployment_id must be canonical when present"
        in diagnostics
    )
    assert (
        "gateway_load aggregate invalid row environment must be canonical when present"
        in diagnostics
    )
    assert (
        "reputation aggregate required row fields must match the schema-closed output contract"
        in diagnostics
    )
    assert (
        "reputation aggregate required row <sensitive-key> is not allowed"
        in diagnostics
    )
    assert (
        "reputation aggregate missing row errors must match the deterministic missing summary diagnostic"
        in diagnostics
    )
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private-key-placeholder drift" not in diagnostics
    assert "AB" * 32 not in diagnostics
    assert "runtime-only-deployment" not in diagnostics
    assert "prod\nsecret" not in diagnostics


def test_duplicate_gate_summary_fails(tmp_path: Path) -> None:
    first = write_gate(tmp_path, "gateway_load")
    second = tmp_path / "gateway_load_duplicate.json"
    second.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
    third = tmp_path / "gateway_load_duplicate_2.json"
    third.write_text(first.read_text(encoding="utf-8"), encoding="utf-8")
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-gate",
            "gateway_load",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    result = json.loads(summary.read_text(encoding="utf-8"))
    row_errors = result["required"]["gateway_load"]["errors"]
    assert row_errors.count("duplicate gateway_load production readiness summary") == 1
    assert (
        result["errors"].count("duplicate gateway_load production readiness summary")
        == 2
    )

    errors: list[str] = []
    result["required"]["gateway_load"]["errors"] = [
        "duplicate gateway_load production readiness summary",
        "duplicate gateway_load production readiness summary",
    ]
    MODULE.validate_duplicate_summary_diagnostics(
        result["required"],
        {"gateway_load"},
        2,
        errors,
    )
    assert (
        "gateway_load duplicate summary row errors must contain the deterministic duplicate summary diagnostic exactly once"
        in "\n".join(errors)
    )
    errors = []
    result["required"]["gateway_load"]["errors"] = [
        "duplicate gateway_load production readiness summary"
    ]
    MODULE.validate_duplicate_summary_diagnostics(
        result["required"],
        {"gateway_load"},
        3,
        errors,
    )
    assert (
        "aggregate summary duplicate-summary diagnostics must match duplicate summary inputs"
        in "\n".join(errors)
    )
