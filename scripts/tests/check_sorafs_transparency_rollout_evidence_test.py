"""Tests for scripts/check_sorafs_transparency_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1]
    / "check_sorafs_transparency_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_transparency_rollout_evidence", MODULE_PATH
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


REQUIRED_SOURCE_KINDS = (
    "gar-enforcement-receipt",
    "moderation-ballot-governance-event",
    "appeal-finance-report",
    "appeal-finance-settlement-receipt",
    "legal-hold-notice",
    "redaction-notice",
    "evidence-access-summary",
)
DIGEST = "ab" * 32
DIGEST_2 = "cd" * 32


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def source_entry_evidence() -> dict:
    probes = [
        {
            "source_kind": source_kind,
            "response_success": True,
            "response_status": 202,
            "request_body_blake3": "a" * 64,
            "response_body_blake3": "b" * 64,
        }
        for source_kind in REQUIRED_SOURCE_KINDS
    ]
    return {
        "schema": "sorafs.transparency.source_entry.canary.v1",
        "status": "passed",
        "source_batch_digest_hex": DIGEST,
        "probe_count": len(probes),
        "passed_probe_count": len(probes),
        "source_entry_probe_count": len(probes),
        "payload_bytes_included": False,
        "private_payloads_included": False,
        "response_bodies_included": False,
        "probes": probes,
    }


def publication_evidence(*, publisher_identity: bool = True) -> dict:
    return {
        "schema": "sorafs.transparency.publication_canary.v1",
        "status": "passed" if publisher_identity else "failed",
        "source_batch_digest_hex": DIGEST,
        "cycle_digest_hex": DIGEST,
        "route_count": 2,
        "passed_route_count": 2 if publisher_identity else 1,
        "cycle_detail_probe_count": 1,
        "publisher_identity_required": True,
        "payload_bytes_included": False,
        "publication_bodies_included": False,
        "private_payloads_included": False,
        "routes": [
            {
                "name": "cycles_list",
                "passed": publisher_identity,
                "http_success": True,
                "status_code": 200,
                "anchor_metadata_present": True,
                "publisher_identity_present": publisher_identity,
                "verification_valid": True,
            },
            {
                "name": "cycle_publication",
                "passed": True,
                "http_success": True,
                "status_code": 200,
                "anchor_metadata_present": True,
                "publisher_identity_present": True,
                "verification_valid": True,
            },
        ],
    }


def privacy_aggregate_evidence(*, publish_due_count: int = 1) -> dict:
    return {
        "schema": "sorafs.transparency.privacy_aggregate.canary.v1",
        "status": "passed",
        "cycle_digest_hex": DIGEST,
        "probe_count": 2,
        "passed_probe_count": 2,
        "source_event_probe_count": 1,
        "publish_due_probe_count": publish_due_count,
        "payload_bytes_included": False,
        "raw_metric_values_included": False,
        "private_payloads_included": False,
        "probes": [
            {"response_success": True, "response_status": 202},
            {"response_success": True, "response_status": 200},
        ],
    }


def proof_token_issuance_evidence() -> dict:
    return {
        "schema": "sorafs.transparency.proof_token_issuance.canary.v1",
        "status": "passed",
        "cycle_digest_hex": DIGEST,
        "probe_count": 1,
        "passed_probe_count": 1,
        "issuance_probe_count": 1,
        "payload_bytes_included": False,
        "proof_token_frames_included": False,
        "private_digest_keys_included": False,
        "response_bodies_included": False,
        "probes": [{"response_success": True, "response_status": 202}],
    }


def explorer_evidence() -> dict:
    return {
        "schema": "sorafs.transparency.explorer_canary.v1",
        "status": "passed",
        "cycle_digest_hex": DIGEST,
        "route_count": 3,
        "payload_bytes_included": False,
        "private_digest_keys_included": False,
        "routes": [
            {"name": "explorer_snapshot", "status_code": 200},
            {"name": "browser_ui", "status_code": 200},
            {"name": "proof_token_issuance_index", "status_code": 200},
        ],
    }


def write_complete_evidence(root: Path) -> None:
    write_json(root / "source-entry.json", source_entry_evidence())
    write_json(root / "publication.json", publication_evidence())
    write_json(root / "privacy-aggregate.json", privacy_aggregate_evidence())
    write_json(root / "proof-token-issuance.json", proof_token_issuance_evidence())
    write_json(root / "explorer.json", explorer_evidence())


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert MODULE.main(["--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.transparency.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["recognized_artifact_count"] == 5
    assert payload["required"]["publication"]["valid"] is True
    assert payload["valid_source_batch_digests"] == [DIGEST]
    assert payload["valid_cycle_digests"] == [DIGEST]


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "transparency.args"
    args.write_text(f"--evidence-dir {tmp_path}\n", encoding="utf-8")

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_required_kind_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "explorer.json").unlink()

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_missing_evidence_directory_reports_directory_error(tmp_path: Path) -> None:
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(
            [
                "--evidence-dir",
                str(tmp_path / "missing"),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert "must exist" in "\n".join(payload["errors"])


def test_privacy_aggregate_requires_publish_due_probe(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "privacy-aggregate.json", privacy_aggregate_evidence(publish_due_count=0))

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_publication_requires_publisher_identity(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "publication.json", publication_evidence(publisher_identity=False))

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_publication_requires_publisher_identity_policy_flag(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = publication_evidence()
    payload["publisher_identity_required"] = False
    write_json(tmp_path / "publication.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert "publisher_identity_required must be true" in artifact["errors"]


def test_publication_requires_explicit_route_fields(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    del payload["routes"][0]["publisher_identity_present"]
    write_json(tmp_path / "publication.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_publication_requires_cycle_detail_probe(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probe_count"] = 0
    write_json(tmp_path / "publication.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_publication_requires_source_batch_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload.pop("source_batch_digest_hex")
    write_json(tmp_path / "publication.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_explorer_cycle_binding_must_match_publication(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = explorer_evidence()
    payload["cycle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "explorer.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_invalid_publication_does_not_anchor_cycle_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["source_batch_digest_hex"] = DIGEST_2
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        MODULE.main(["--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_source_batch_digests"] == [DIGEST]
    assert payload["valid_cycle_digests"] == []
    assert payload["required"]["publication"]["valid"] is False
    assert payload["required"]["explorer"]["valid"] is False
    errors = "\n".join(payload["errors"])
    assert (
        "publication source_batch_digest_hex must match a valid source_entry artifact"
        in errors
    )
    assert (
        "explorer cycle_digest_hex must match a valid source-bound publication artifact"
        in errors
    )


def test_cycle_bound_subset_requires_publication_anchor(tmp_path: Path) -> None:
    write_json(tmp_path / "explorer.json", explorer_evidence())

    assert MODULE.main(["--evidence-dir", str(tmp_path), "--require-kind", "explorer"]) == 1


def test_source_entry_requires_all_supported_source_kinds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    payload["probes"] = payload["probes"][:-1]
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_entry_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "source-entry.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_explorer_requires_named_route_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = explorer_evidence()
    payload["routes"][2]["name"] = "unexpected_route"
    write_json(tmp_path / "explorer.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_sensitive_response_body_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    payload["probes"][0]["response_body"] = {"secret": "leaked"}
    write_json(tmp_path / "source-entry.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_sensitive_authorization_token_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["authorization"] = "Bearer runtime-token"
    write_json(tmp_path / "publication.json", payload)

    assert MODULE.main(["--evidence-dir", str(tmp_path)]) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.unknown.v1"})

    assert MODULE.main(["--evidence", str(path)]) == 1
