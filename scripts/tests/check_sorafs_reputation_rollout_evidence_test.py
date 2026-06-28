"""Tests for scripts/check_sorafs_reputation_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_reputation_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reputation_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


SNAPSHOT_ID = "11" * 16
MERKLE_ROOT = "22" * 32
MERKLE_ROOT_2 = "44" * 32
NOW_UNIX = 1_800_100_000
GENERATED_AT = NOW_UNIX - 120


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def snapshot_summary(*, snapshot_id: str = SNAPSHOT_ID, generated_at: int = GENERATED_AT) -> dict:
    return {
        "status": "accepted",
        "snapshot_id_hex": snapshot_id,
        "generated_at_unix": generated_at,
        "provider_count": 2,
        "merkle_root_hex": MERKLE_ROOT,
    }


def provider_evidence(*, provider_id: str = "provider-a", snapshot_id: str = SNAPSHOT_ID) -> dict:
    return {
        "snapshot_id_hex": snapshot_id,
        "merkle_root_hex": MERKLE_ROOT,
        "provider": {
            "provider_id": provider_id,
            "score_bps": 9_400,
        },
        "proof": {
            "provider_id": provider_id,
            "leaf_index": 1,
            "siblings_hex": ["33" * 32],
        },
    }


def events_evidence() -> dict:
    return {
        "since": 0,
        "limit": 10,
        "count": 1,
        "next_since": 1,
        "events": [
            {
                "version": 1,
                "sequence": 1,
                "snapshot_id_hex": SNAPSHOT_ID,
                "generated_at_unix": GENERATED_AT,
                "merkle_root_hex": MERKLE_ROOT,
                "provider_count": 2,
            }
        ],
    }


def verify_evidence(provider_id: str = "provider-a") -> dict:
    return {
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "provider_count": 2,
        "valid": True,
        "provider_id": provider_id,
        "provider_score_bps": 9_400,
        "proof_verified": True,
    }


def metrics_evidence(*, snapshot_age: int = 120, ingest_lag: int = 60) -> dict:
    return {
        "schema": "sorafs.reputation.metrics_canary.v1",
        "status": "passed",
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "metrics_scrape_success": True,
        "snapshot_age_seconds": snapshot_age,
        "ingest_lag_seconds": ingest_lag,
        "provider_count": 2,
        "response_bodies_included": False,
    }


def transport_evidence() -> dict:
    return {
        "schema": "sorafs.reputation.transport_canary.v1",
        "status": "passed",
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "sse_connected": True,
        "sse_event_count": 1,
        "websocket_connected": True,
        "websocket_event_count": 1,
        "response_bodies_included": False,
    }


def consumption_evidence() -> dict:
    return {
        "schema": "sorafs.reputation.routing_incentive_consumption.v1",
        "status": "passed",
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "provider_count": 2,
        "routing_score_consumed": True,
        "routing_weight_changed": True,
        "incentive_score_consumed": True,
        "raw_provider_records_included": False,
    }


def write_complete_evidence(root: Path) -> None:
    write_json(root / "publish.json", snapshot_summary())
    write_json(root / "latest.json", snapshot_summary())
    write_json(root / "provider-provider-a.json", provider_evidence())
    write_json(root / "events.json", events_evidence())
    write_json(root / "verify-provider-a.json", verify_evidence())
    write_json(root / "metrics.json", metrics_evidence())
    write_json(root / "transport.json", transport_evidence())
    write_json(root / "routing-consumption.json", consumption_evidence())


def run_gate(root: Path, *extra: str) -> int:
    return MODULE.main(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def test_complete_rollout_evidence_passes(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"

    assert (
        run_gate(tmp_path, "--require-provider", "provider-a", "--summary-out", str(summary))
        == 0
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["schema"] == "sorafs.reputation.rollout_evidence_gate.v1"
    assert payload["status"] == "ready"
    assert payload["required"]["transport"]["valid"] is True
    assert payload["provider_ids"] == ["provider-a"]
    assert payload["valid_snapshot_bindings"] == [
        {
            "snapshot_id_hex": SNAPSHOT_ID,
            "merkle_root_hex": MERKLE_ROOT,
        }
    ]


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "reputation.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args}"]) == 0


def test_missing_evidence_sources_fail_shared_preflight(capsys) -> None:
    assert MODULE.main(["--now-unix", str(NOW_UNIX)]) == 2

    captured = capsys.readouterr()
    assert "ERROR: provide --evidence-dir or --evidence" in captured.err


def test_missing_transport_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    (tmp_path / "transport.json").unlink()

    assert run_gate(tmp_path) == 1


def test_stale_latest_snapshot_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "latest.json", snapshot_summary(generated_at=NOW_UNIX - 900_000))

    assert run_gate(tmp_path) == 1


def test_snapshot_status_must_be_allowed_when_present(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    payload["status"] = "failed"
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert (
        "latest.status must be accepted/published/ready/ok when present"
        in artifact["errors"]
    )


def test_provider_proof_provider_id_must_match_provider(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = provider_evidence()
    payload["proof"]["provider_id"] = "provider-b"
    write_json(tmp_path / "provider-provider-a.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "proof.provider_id must match provider.provider_id" in artifact["errors"]


def test_invalid_duplicate_artifact_fails_even_with_valid_artifact(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "latest-stale.json",
        snapshot_summary(generated_at=NOW_UNIX - 900_000),
    )

    assert run_gate(tmp_path) == 1


def test_snapshot_id_mismatch_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "provider-provider-a.json", provider_evidence(snapshot_id="44" * 16))

    assert run_gate(tmp_path) == 1


def test_required_provider_must_have_proof(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)

    assert run_gate(tmp_path, "--require-provider", "provider-b") == 1


def test_high_ingest_lag_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "metrics.json", metrics_evidence(ingest_lag=2_000))
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        f"ingest_lag_seconds must be <= {MODULE.DEFAULT_MAX_INGEST_LAG_SECS}"
        in artifact["errors"]
    )


def test_high_snapshot_age_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "metrics.json",
        metrics_evidence(snapshot_age=MODULE.DEFAULT_MAX_SNAPSHOT_AGE_SECS + 1),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        f"snapshot_age_seconds must be <= {MODULE.DEFAULT_MAX_SNAPSHOT_AGE_SECS}"
        in artifact["errors"]
    )


def test_metrics_status_must_be_passed(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    payload["status"] = "failed"
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert "metrics.status must be passed" in artifact["errors"]


def test_metrics_schema_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    payload["schema"] = "sorafs.reputation.other.v1"
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert (
        "metrics.schema must be sorafs.reputation.metrics_canary.v1"
        in artifact["errors"]
    )


def test_metrics_requires_merkle_root_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = metrics_evidence()
    payload.pop("merkle_root_hex")
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path) == 1


def test_transport_merkle_root_must_match_snapshot_anchor(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["merkle_root_hex"] = MERKLE_ROOT_2
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["transport"]
    artifact = required["artifacts"][0]
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "transport.merkle_root_hex "
        f"`{MERKLE_ROOT_2}` does not match `{MERKLE_ROOT}`",
    ]


def test_transport_schema_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["schema"] = "sorafs.reputation.other.v1"
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert (
        "transport.schema must be sorafs.reputation.transport_canary.v1"
        in artifact["errors"]
    )


def test_stale_publish_latest_do_not_anchor_snapshot_bound_evidence(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    stale_generated_at = NOW_UNIX - MODULE.DEFAULT_MAX_SNAPSHOT_AGE_SECS - 1
    write_json(
        tmp_path / "publish.json",
        snapshot_summary(generated_at=stale_generated_at),
    )
    write_json(
        tmp_path / "latest.json",
        snapshot_summary(generated_at=stale_generated_at),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    required = payload["required"]["transport"]
    artifact = required["artifacts"][0]
    assert payload["valid_snapshot_bindings"] == []
    assert required["valid"] is False
    assert artifact["valid"] is False
    assert artifact["errors"] == [
        "snapshot_id_hex and merkle_root_hex require a valid publish/latest artifact"
    ]


def test_sensitive_response_body_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = transport_evidence()
    payload["response_body"] = {"event": "raw frame"}
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path) == 1


def test_sensitive_authorization_token_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = metrics_evidence()
    payload["authorization"] = "Bearer runtime-token"
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path) == 1


def test_consumption_schema_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = consumption_evidence()
    payload["schema"] = "sorafs.reputation.other.v1"
    write_json(tmp_path / "routing-consumption.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["consumption"]["artifacts"][0]
    assert (
        "consumption.schema must be "
        "sorafs.reputation.routing_incentive_consumption.v1"
        in artifact["errors"]
    )


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "latest.json", snapshot_summary())
    payload = transport_evidence()
    payload["response_body"] = {"event": "raw frame"}
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--require-kind", "latest") == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    path = write_json(tmp_path / "unknown.json", {"schema": "sorafs.reputation.unknown.v1"})

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1


def test_malformed_explicit_evidence_kind_sanitizes_exception_text(
    monkeypatch,
) -> None:
    bad_message = "latest\nshadow"

    def raise_malformed_evidence_spec(_spec: str):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "parse_evidence_spec", raise_malformed_evidence_spec)
    loaded, errors = MODULE.load_evidence([], ["latest=/tmp/evidence.json"])

    assert loaded == []
    assert "<non-canonical-error>" in errors
    assert bad_message not in "\n".join(errors)


def test_prefixed_explicit_evidence_matching_summary_out_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    path = write_json(tmp_path / "evidence.json", snapshot_summary())
    original = path.read_text(encoding="utf-8")
    summary_out = tmp_path / "nested" / ".." / "evidence.json"

    assert (
        MODULE.main(
            [
                "--evidence",
                f"latest={path}",
                "--summary-out",
                str(summary_out),
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 2
    )

    assert path.read_text(encoding="utf-8") == original
    assert "conflicts with reserved output" in capsys.readouterr().err


def test_explicit_malformed_json_reports_shared_load_error(
    tmp_path: Path, capsys
) -> None:
    path = tmp_path / "malformed.json"
    path.write_text("{not-json", encoding="utf-8")

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1

    captured = capsys.readouterr()
    summary = json.loads(captured.out)
    assert any(
        error.startswith(f"{path}: failed to load evidence JSON:")
        for error in summary["load_errors"]
    )


def test_explicit_missing_file_reports_shared_load_error(
    tmp_path: Path, capsys
) -> None:
    path = tmp_path / "missing.json"

    assert MODULE.main(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1

    captured = capsys.readouterr()
    summary = json.loads(captured.out)
    assert any(
        error.startswith(f"{path}: failed to load evidence JSON:")
        for error in summary["load_errors"]
    )
