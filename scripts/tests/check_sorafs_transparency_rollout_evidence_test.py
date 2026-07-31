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

TEST_DIR = Path(__file__).resolve().parent
if str(TEST_DIR) not in sys.path:
    sys.path.insert(0, str(TEST_DIR))
from sorafs_rollout_runner_test_support import TopologyBoundChecker  # noqa: E402


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
DEPLOYMENT_ID = "transparency-production-a"
ENVIRONMENT = "production"
GENERATED_AT = 1_800_000_120
NOW_UNIX = GENERATED_AT
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="transparency-checker",
)


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
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
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
    cycle_detail_probes = [
        {
            "name": MODULE.REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES[0],
            "status_code": 200,
            "body_blake3_hex": "3" * 64,
            "anchor_metadata_present": True,
            "publisher_identity_present": True,
            "verification_valid": True,
        }
    ]
    return {
        "schema": "sorafs.transparency.publication_canary.v1",
        "status": "passed" if publisher_identity else "failed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
        "source_batch_digest_hex": DIGEST,
        "cycle_digest_hex": DIGEST,
        "route_count": 2,
        "passed_route_count": 2 if publisher_identity else 1,
        "cycle_detail_probe_count": len(cycle_detail_probes),
        "cycle_detail_probes": cycle_detail_probes,
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
                "body_blake3_hex": "1" * 64,
                "anchor_metadata_present": True,
                "publisher_identity_present": publisher_identity,
                "verification_valid": True,
            },
            {
                "name": "cycle_publication",
                "passed": True,
                "http_success": True,
                "status_code": 200,
                "body_blake3_hex": "2" * 64,
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
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
        "cycle_digest_hex": DIGEST,
        "probe_count": 2,
        "passed_probe_count": 2,
        "source_event_probe_count": 1,
        "publish_due_probe_count": publish_due_count,
        "payload_bytes_included": False,
        "raw_metric_values_included": False,
        "private_payloads_included": False,
        "probes": [
            {
                "action": "source_event",
                "response_success": True,
                "response_status": 202,
                "request_body_blake3": "a" * 64,
                "response_body_blake3": "b" * 64,
            },
            {
                "action": "publish_due",
                "response_success": True,
                "response_status": 200,
                "request_body_blake3": "c" * 64,
                "response_body_blake3": "d" * 64,
            },
        ],
    }


def proof_token_issuance_evidence() -> dict:
    return {
        "schema": "sorafs.transparency.proof_token_issuance.canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
        "cycle_digest_hex": DIGEST,
        "probe_count": 1,
        "passed_probe_count": 1,
        "issuance_probe_count": 1,
        "payload_bytes_included": False,
        "proof_token_frames_included": False,
        "private_digest_keys_included": False,
        "response_bodies_included": False,
        "probes": [
            {
                "action": "proof_token_issuance",
                "response_success": True,
                "response_status": 202,
                "request_body_blake3": "e" * 64,
                "response_body_blake3": "f" * 64,
            }
        ],
    }


def explorer_evidence() -> dict:
    return {
        "schema": "sorafs.transparency.explorer_canary.v1",
        "status": "passed",
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
        "cycle_digest_hex": DIGEST,
        "route_count": 3,
        "payload_bytes_included": False,
        "private_digest_keys_included": False,
        "routes": [
            {
                "name": "explorer_snapshot",
                "status_code": 200,
                "body_blake3_hex": "1" * 64,
            },
            {
                "name": "browser_ui",
                "status_code": 200,
                "body_blake3_hex": "2" * 64,
            },
            {
                "name": "proof_token_issuance_index",
                "status_code": 200,
                "body_blake3_hex": "3" * 64,
            },
        ],
    }


def write_complete_evidence(root: Path) -> None:
    write_json(root / "source-entry.json", source_entry_evidence())
    write_json(root / "publication.json", publication_evidence())
    write_json(root / "privacy-aggregate.json", privacy_aggregate_evidence())
    write_json(root / "proof-token-issuance.json", proof_token_issuance_evidence())
    write_json(root / "explorer.json", explorer_evidence())


SOURCE_BOUND_FIXTURES = (
    ("publication", "publication.json", publication_evidence),
)

CYCLE_BOUND_FIXTURES = (
    ("privacy_aggregate", "privacy-aggregate.json", privacy_aggregate_evidence),
    ("proof_token_issuance", "proof-token-issuance.json", proof_token_issuance_evidence),
    ("explorer", "explorer.json", explorer_evidence),
)


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 0

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.transparency.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["thresholds"] == {
        "max_evidence_bytes": MODULE.MAX_EVIDENCE_BYTES,
        "max_evidence_age_secs": MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS,
    }
    assert payload["recognized_artifact_count"] == 5
    assert payload["required"]["publication"]["valid"] is True
    assert payload["valid_source_batch_digests"] == [DIGEST]
    assert payload["valid_cycle_digests"] == [DIGEST]
    assert payload["valid_publication_bindings"] == [
        {
            "source_batch_digest_hex": DIGEST,
            "cycle_digest_hex": DIGEST,
        }
    ]
    assert payload["required"]["publication"]["artifacts"][0]["fingerprint"][
        "deployment_id"
    ] == DEPLOYMENT_ID


def test_generated_at_unix_rejects_future_and_stale_artifacts(tmp_path: Path) -> None:
    cases = (
        (
            "future",
            NOW_UNIX + 1,
            "generated_at_unix must not be in the future",
        ),
        (
            "stale",
            NOW_UNIX - MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS - 1,
            f"generated_at_unix is older than {MODULE.DEFAULT_MAX_EVIDENCE_AGE_SECS} seconds",
        ),
    )

    for label, generated_at, expected_error in cases:
        root = tmp_path / label
        root.mkdir()
        payload = source_entry_evidence()
        payload["generated_at_unix"] = generated_at
        write_json(root / "source-entry.json", payload)
        summary = root / "summary.json"

        assert run_gate(root, "--require-kind", "source_entry", "--summary-out", str(summary)) == 1

        report = json.loads(summary.read_text(encoding="utf-8"))
        assert expected_error in json.dumps(report)


def test_bound_fixture_tables_cover_checker_bound_kind_sets() -> None:
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in SOURCE_BOUND_FIXTURES)
        == MODULE.SOURCE_BOUND_KINDS
    )
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in CYCLE_BOUND_FIXTURES)
        == MODULE.CYCLE_BOUND_KINDS
    )


def test_fixture_inventories_cover_checker_required_sets() -> None:
    assert tuple(route["name"] for route in publication_evidence()["routes"]) == (
        MODULE.REQUIRED_PUBLICATION_ROUTES
    )
    assert tuple(
        probe["name"] for probe in publication_evidence()["cycle_detail_probes"]
    ) == MODULE.REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES
    assert tuple(route["name"] for route in explorer_evidence()["routes"]) == (
        MODULE.REQUIRED_EXPLORER_ROUTES
    )
    assert tuple(probe["action"] for probe in privacy_aggregate_evidence()["probes"]) == (
        MODULE.REQUIRED_PRIVACY_AGGREGATE_ACTIONS
    )
    assert tuple(
        probe["action"] for probe in proof_token_issuance_evidence()["probes"]
    ) == (
        MODULE.REQUIRED_PROOF_TOKEN_ISSUANCE_ACTIONS
    )


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "transparency.args"
    args.write_text(f"--evidence-dir {tmp_path}\n", encoding="utf-8")

    assert CHECKER(["--now-unix", str(NOW_UNIX), f"@{args}"]) == 0


def test_missing_required_kind_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "explorer.json").unlink()

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_route_count_must_match_unique_routes_for_route_artifacts(
    tmp_path: Path,
) -> None:
    route_artifacts = (
        ("publication", "publication.json", publication_evidence),
        ("explorer", "explorer.json", explorer_evidence),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["route_count"] += 1
        if "passed_route_count" in payload:
            payload["passed_route_count"] = payload["route_count"]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert (
            CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), "--summary-out", str(summary)])
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "route_count must match unique routes count" in artifact["errors"]


def test_routes_must_not_duplicate_for_route_artifacts(tmp_path: Path) -> None:
    route_artifacts = (
        ("publication", "publication.json", publication_evidence),
        ("explorer", "explorer.json", explorer_evidence),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["routes"].append(dict(payload["routes"][0]))
        payload["route_count"] = len(payload["routes"])
        if "passed_route_count" in payload:
            payload["passed_route_count"] = len(payload["routes"])
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert (
            CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), "--summary-out", str(summary)])
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "routes must not contain duplicate values" in artifact["errors"]
        assert "route_count must match unique routes count" in artifact["errors"]


def test_routes_must_not_include_unknown_values_for_route_artifacts(
    tmp_path: Path,
) -> None:
    route_artifacts = (
        ("publication", "publication.json", publication_evidence),
        ("explorer", "explorer.json", explorer_evidence),
    )
    for kind, filename, factory in route_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        unknown = dict(payload["routes"][0])
        unknown["name"] = "unexpected_transparency_route"
        unknown["body_blake3_hex"] = "4" * 64
        payload["routes"].append(unknown)
        payload["route_count"] = len(payload["routes"])
        if "passed_route_count" in payload:
            payload["passed_route_count"] = len(payload["routes"])
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert (
            CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), "--summary-out", str(summary)])
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "routes must not include unknown values" in artifact["errors"]


def test_source_entry_probes_must_not_duplicate_source_kind(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    payload["probes"].append(dict(payload["probes"][0]))
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_entry_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "source-entry.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["source_entry"]["artifacts"][0]
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert (
        "source_entry_probe_count must match unique probes count"
        in artifact["errors"]
    )


def test_source_entry_probe_kinds_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    unknown = dict(payload["probes"][0])
    unknown["source_kind"] = "unreviewed-source-kind"
    unknown["request_body_blake3"] = "c" * 64
    unknown["response_body_blake3"] = "d" * 64
    payload["probes"].append(unknown)
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_entry_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "source-entry.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["source_entry"]["artifacts"][0]
    assert "probes must not include unknown values" in artifact["errors"]


def test_probe_count_must_match_probe_inventory_for_probe_artifacts(
    tmp_path: Path,
) -> None:
    probe_artifacts = (
        ("source_entry", "source-entry.json", source_entry_evidence),
        ("privacy_aggregate", "privacy-aggregate.json", privacy_aggregate_evidence),
        (
            "proof_token_issuance",
            "proof-token-issuance.json",
            proof_token_issuance_evidence,
        ),
    )
    for kind, filename, factory in probe_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload["probe_count"] += 1
        payload["passed_probe_count"] = payload["probe_count"]
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert (
            CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), "--summary-out", str(summary)])
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert "probe_count must equal probes length" in artifact["errors"]
        if kind in {"privacy_aggregate", "proof_token_issuance"}:
            assert "probe_count must match unique probes count" in artifact["errors"]


def test_specific_probe_counts_must_match_probe_roles(tmp_path: Path) -> None:
    role_artifacts = (
        (
            "source_entry",
            "source-entry.json",
            source_entry_evidence,
            "source_entry_probe_count",
            "source_entry probes count",
        ),
        (
            "privacy_aggregate",
            "privacy-aggregate.json",
            privacy_aggregate_evidence,
            "source_event_probe_count",
            "source_event probes count",
        ),
        (
            "proof_token_issuance",
            "proof-token-issuance.json",
            proof_token_issuance_evidence,
            "issuance_probe_count",
            "issuance probes count",
        ),
    )
    for kind, filename, factory, count_field, expected_label in role_artifacts:
        root = tmp_path / kind
        root.mkdir()
        write_complete_evidence(root)
        payload = factory()
        payload[count_field] += 1
        write_json(root / filename, payload)
        summary = root / "summary.json"

        assert (
            CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(root), "--summary-out", str(summary)])
            == 1
        )

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert f"{count_field} must equal {expected_label}" in artifact["errors"]


def test_privacy_aggregate_probe_role_counts_must_sum_to_probe_count(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = privacy_aggregate_evidence()
    payload["source_event_probe_count"] = 2
    payload["publish_due_probe_count"] = 1
    write_json(tmp_path / "privacy-aggregate.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["privacy_aggregate"]["artifacts"][0]
    assert (
        "source_event_probe_count plus publish_due_probe_count must equal probe_count"
        in artifact["errors"]
    )


def test_privacy_aggregate_actions_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = privacy_aggregate_evidence()
    duplicate = dict(payload["probes"][0])
    duplicate["request_body_blake3"] = "e" * 64
    duplicate["response_body_blake3"] = "f" * 64
    payload["probes"].append(duplicate)
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_event_probe_count"] = 2
    payload["publish_due_probe_count"] = 1
    write_json(tmp_path / "privacy-aggregate.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["privacy_aggregate"]["artifacts"][0]
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert "probe_count must match unique probes count" in artifact["errors"]


def test_privacy_aggregate_actions_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = privacy_aggregate_evidence()
    unknown = dict(payload["probes"][0])
    unknown["action"] = "privacy_refresh"
    unknown["request_body_blake3"] = "e" * 64
    unknown["response_body_blake3"] = "f" * 64
    payload["probes"].append(unknown)
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "privacy-aggregate.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["privacy_aggregate"]["artifacts"][0]
    assert "probes must not include unknown values" in artifact["errors"]


def test_proof_token_issuance_actions_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_token_issuance_evidence()
    duplicate = dict(payload["probes"][0])
    duplicate["request_body_blake3"] = "1" * 64
    duplicate["response_body_blake3"] = "2" * 64
    payload["probes"].append(duplicate)
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["issuance_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "proof-token-issuance.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_token_issuance"]["artifacts"][0]
    assert "probes must not contain duplicate values" in artifact["errors"]
    assert "probe_count must match unique probes count" in artifact["errors"]
    assert "issuance_probe_count must match unique probes count" in artifact["errors"]


def test_proof_token_issuance_actions_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_token_issuance_evidence()
    unknown = dict(payload["probes"][0])
    unknown["action"] = "proof_token_replay"
    unknown["request_body_blake3"] = "1" * 64
    unknown["response_body_blake3"] = "2" * 64
    payload["probes"].append(unknown)
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "proof-token-issuance.json", payload)
    summary = tmp_path / "summary.json"

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["proof_token_issuance"]["artifacts"][0]
    assert "probes must not include unknown values" in artifact["errors"]


def test_proof_token_issuance_requires_action_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_token_issuance_evidence()
    del payload["probes"][0]["action"]
    write_json(tmp_path / "proof-token-issuance.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["proof_token_issuance"]["artifacts"][0]
    assert "probes must include action `proof_token_issuance`" in artifact["errors"]


def test_deployment_context_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = explorer_evidence()
    del payload["deployment_id"]
    write_json(tmp_path / "explorer.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["explorer"]["artifacts"][0]
    assert "deployment_id must be a non-empty canonical string" in artifact["errors"]


def test_unreviewed_deployment_context_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["deployment_id"] = "transparency-dev-a"
    payload["environment"] = "dev"
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert (
        "deployment_id must not contain non-reviewed deployment markers ['dev']"
        in artifact["errors"]
    )
    assert "environment must be one of" in "\n".join(artifact["errors"])


def test_missing_evidence_directory_reports_directory_error(tmp_path: Path) -> None:
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence-dir",
                str(tmp_path / "missing"),
                "--summary-out",
                str(summary),
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert "must exist" in "\n".join(payload["errors"])


def test_privacy_aggregate_requires_publish_due_probe(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "privacy-aggregate.json",
        privacy_aggregate_evidence(publish_due_count=0),
    )

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_privacy_aggregate_requires_action_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = privacy_aggregate_evidence()
    payload["probes"][1]["action"] = "source_event"
    write_json(tmp_path / "privacy-aggregate.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["privacy_aggregate"]["artifacts"][0]
    assert "probes must include action `publish_due`" in artifact["errors"]


def test_probe_evidence_requires_request_and_response_hashes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = proof_token_issuance_evidence()
    del payload["probes"][0]["response_body_blake3"]
    write_json(tmp_path / "proof-token-issuance.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["proof_token_issuance"]["artifacts"][0]
    assert (
        "probes[0].response_body_blake3 must be a non-empty string"
        in artifact["errors"]
    )


def test_route_evidence_requires_response_hashes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = explorer_evidence()
    del payload["routes"][1]["body_blake3_hex"]
    write_json(tmp_path / "explorer.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["explorer"]["artifacts"][0]
    assert (
        "routes[1].body_blake3_hex must be a non-empty string"
        in artifact["errors"]
    )


def test_publication_requires_publisher_identity(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "publication.json", publication_evidence(publisher_identity=False))

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_publication_requires_publisher_identity_policy_flag(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = publication_evidence()
    payload["publisher_identity_required"] = False
    write_json(tmp_path / "publication.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)]) == 1

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert "publisher_identity_required must be true" in artifact["errors"]


def test_publication_requires_explicit_route_fields(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    del payload["routes"][0]["publisher_identity_present"]
    write_json(tmp_path / "publication.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_publication_requires_cycle_detail_probe(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probe_count"] = 0
    write_json(tmp_path / "publication.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_publication_cycle_detail_probes_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probes"].append(dict(payload["cycle_detail_probes"][0]))
    payload["cycle_detail_probe_count"] = len(payload["cycle_detail_probes"])
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert "cycle_detail_probes must not contain duplicate values" in artifact[
        "errors"
    ]
    assert (
        "cycle_detail_probe_count must match unique cycle_detail_probes count"
        in artifact["errors"]
    )


def test_publication_cycle_detail_probes_must_not_include_unknown_values(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    unknown = dict(payload["cycle_detail_probes"][0])
    unknown["name"] = "unexpected_cycle_detail_probe"
    unknown["body_blake3_hex"] = "4" * 64
    payload["cycle_detail_probes"].append(unknown)
    payload["cycle_detail_probe_count"] = len(payload["cycle_detail_probes"])
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert "cycle_detail_probes must not include unknown values" in artifact[
        "errors"
    ]


def test_publication_cycle_detail_probe_names_must_use_production_family(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probes"][0]["name"] = "cycle_detail_readback"
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert MODULE.CYCLE_DETAIL_PROBE_LABEL_ERROR in artifact["errors"]


def test_publication_cycle_detail_probe_names_reject_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probes"][0]["name"] = "transparency-cycle-detail-placeholder"
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert (
        "cycle_detail_probes[0].name must not contain non-production markers "
        "['placeholder']"
    ) in artifact["errors"]


def test_publication_requires_cycle_detail_probe_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probes"] = []
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert (
        "cycle_detail_probes must include name `transparency-cycle-detail-readback`"
        in artifact["errors"]
    )
    assert (
        "cycle_detail_probe_count must match unique cycle_detail_probes count"
        in artifact["errors"]
    )


def test_publication_cycle_detail_probe_requires_publication_proofs(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_detail_probes"][0]["publisher_identity_present"] = False
    payload["cycle_detail_probes"][0]["verification_valid"] = False
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    summary_payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = summary_payload["required"]["publication"]["artifacts"][0]
    assert (
        "cycle_detail_probes[0].publisher_identity_present must be true"
        in artifact["errors"]
    )
    assert (
        "cycle_detail_probes[0].verification_valid must be true"
        in artifact["errors"]
    )


def test_publication_requires_source_batch_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload.pop("source_batch_digest_hex")
    write_json(tmp_path / "publication.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_all_source_bound_artifacts_reject_source_entry_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in SOURCE_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["source_batch_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} source_batch_digest_hex must match "
            "a valid source_entry artifact"
        ) in artifact["errors"]


def test_multiple_valid_source_batch_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    payload["source_batch_digest_hex"] = DIGEST_2
    write_json(tmp_path / "source-entry-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_source_batch_digests"] == []
    assert (
        "valid_source_batch_digests must contain exactly one active digest"
        in result["errors"]
    )


def test_explorer_cycle_binding_must_match_publication(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = explorer_evidence()
    payload["cycle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "explorer.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_all_cycle_bound_artifacts_reject_publication_cycle_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in CYCLE_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        payload["cycle_digest_hex"] = DIGEST_2
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate(case_dir, "--summary-out", str(summary)) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert (
            f"{kind_name} cycle_digest_hex must match "
            "a valid source-bound publication artifact"
        ) in artifact["errors"]


def test_multiple_valid_publication_cycle_anchors_fail_closed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["cycle_digest_hex"] = DIGEST_2
    write_json(tmp_path / "publication-alt.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    assert result["valid_cycle_digests"] == []
    assert result["valid_publication_bindings"] == []
    assert (
        "valid_cycle_digests must contain exactly one active digest"
        in result["errors"]
    )
    assert (
        "valid_publication_bindings must contain exactly one active binding"
        in result["errors"]
    )


def test_invalid_publication_does_not_anchor_cycle_bound_evidence(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["source_batch_digest_hex"] = DIGEST_2
    write_json(tmp_path / "publication.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--summary-out", str(summary)])
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["valid_source_batch_digests"] == [DIGEST]
    assert payload["valid_cycle_digests"] == []
    assert payload["valid_publication_bindings"] == []
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

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path), "--require-kind", "explorer"]) == 1


def test_source_entry_requires_all_supported_source_kinds(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    payload["probes"] = payload["probes"][:-1]
    payload["probe_count"] = len(payload["probes"])
    payload["passed_probe_count"] = len(payload["probes"])
    payload["source_entry_probe_count"] = len(payload["probes"])
    write_json(tmp_path / "source-entry.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_explorer_requires_named_route_coverage(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = explorer_evidence()
    payload["routes"][2]["name"] = "unexpected_route"
    write_json(tmp_path / "explorer.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_sensitive_response_body_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = source_entry_evidence()
    payload["probes"][0]["response_body"] = {"secret": "leaked"}
    write_json(tmp_path / "source-entry.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_sensitive_authorization_token_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = publication_evidence()
    payload["authorization"] = "Bearer runtime-token"
    write_json(tmp_path / "publication.json", payload)

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence-dir", str(tmp_path)]) == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.unknown.v1"})

    assert CHECKER(["--now-unix", str(NOW_UNIX), "--evidence", str(path)]) == 1
