"""Tests for scripts/check_sorafs_reputation_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_reputation_rollout_evidence.py"
PRODUCTION_READINESS_PATH = (
    Path(__file__).resolve().parents[1] / "check_sorafs_production_readiness.py"
)
SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reputation_rollout_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

TEST_DIR = Path(__file__).resolve().parent
if str(TEST_DIR) not in sys.path:
    sys.path.insert(0, str(TEST_DIR))
from sorafs_rollout_runner_test_support import TopologyBoundChecker  # noqa: E402


SNAPSHOT_ID = "11" * 16
MERKLE_ROOT = "22" * 32
MERKLE_ROOT_2 = "44" * 32
WEIGHTS_DIGEST = "55" * 32
NOW_UNIX = 1_800_100_000
GENERATED_AT = NOW_UNIX - 120
DEPLOYMENT_ID = "sorafs-mainnet-2026-06"
ENVIRONMENT = "production"
CHECKER = TopologyBoundChecker(
    MODULE.main,
    deployment_id=DEPLOYMENT_ID,
    environment=ENVIRONMENT,
    name="reputation-checker",
)


def provider_inventory() -> list[dict[str, str]]:
    return [{"name": "provider-a"}, {"name": "provider-b"}]


def write_json(path: Path, payload: dict) -> Path:
    path.write_text(json.dumps(payload), encoding="utf-8")
    return path


def assert_reputation_stderr_is_sanitized(stderr: str, *forbidden: str) -> None:
    assert "ERROR: SoraFS reputation rollout evidence is incomplete:" in stderr
    for token in (
        "Traceback",
        "FileNotFoundError",
        "PermissionError",
        "RuntimeError",
        "SystemExit",
        "ValueError",
    ):
        assert token not in stderr
    for value in forbidden:
        assert value not in stderr


def load_production_readiness_module():
    spec = importlib.util.spec_from_file_location(
        "check_sorafs_production_readiness_for_reputation_test",
        PRODUCTION_READINESS_PATH,
    )
    module = importlib.util.module_from_spec(spec)
    assert spec and spec.loader  # pragma: no cover
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def deployment_context() -> dict:
    return {
        "generated_at_unix": GENERATED_AT,
        "deployment_id": DEPLOYMENT_ID,
        "environment": ENVIRONMENT,
        "deployment_context_reviewed": True,
    }


def snapshot_summary(*, snapshot_id: str = SNAPSHOT_ID, generated_at: int = GENERATED_AT) -> dict:
    return {
        **deployment_context(),
        "status": "accepted",
        "snapshot_id_hex": snapshot_id,
        "generated_at_unix": generated_at,
        "weights_digest_hex": WEIGHTS_DIGEST,
        "provider_count": 2,
        "providers": provider_inventory(),
        "merkle_root_hex": MERKLE_ROOT,
    }


def provider_evidence(*, provider_id: str = "provider-a", snapshot_id: str = SNAPSHOT_ID) -> dict:
    return {
        **deployment_context(),
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
        **deployment_context(),
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
        **deployment_context(),
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "provider_count": 2,
        "providers": provider_inventory(),
        "valid": True,
        "provider_id": provider_id,
        "provider_score_bps": 9_400,
        "proof_verified": True,
    }


def metrics_evidence(*, snapshot_age: int = 120, ingest_lag: int = 60) -> dict:
    return {
        **deployment_context(),
        "schema": "sorafs.reputation.metrics_canary.v1",
        "status": "passed",
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "metrics_scrape_success": True,
        "snapshot_age_seconds": snapshot_age,
        "ingest_lag_seconds": ingest_lag,
        "provider_count": 2,
        "providers": provider_inventory(),
        "metric_count": len(MODULE.REQUIRED_METRICS),
        "metrics": list(MODULE.REQUIRED_METRICS),
        "response_bodies_included": False,
    }


def transport_evidence() -> dict:
    return {
        **deployment_context(),
        "schema": "sorafs.reputation.transport_canary.v1",
        "status": "passed",
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "sse_connected": True,
        "sse_event_count": 1,
        "sse_events": [{"name": "reputation-sse-event-snapshot-00"}],
        "websocket_connected": True,
        "websocket_event_count": 1,
        "websocket_events": [{"name": "reputation-websocket-event-snapshot-00"}],
        "response_bodies_included": False,
    }


def consumption_evidence() -> dict:
    return {
        **deployment_context(),
        "schema": "sorafs.reputation.routing_incentive_consumption.v1",
        "status": "passed",
        "snapshot_id_hex": SNAPSHOT_ID,
        "merkle_root_hex": MERKLE_ROOT,
        "provider_count": 2,
        "providers": provider_inventory(),
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


SNAPSHOT_BOUND_FIXTURES = (
    ("provider", "provider-provider-a.json", provider_evidence),
    ("events", "events.json", events_evidence),
    ("verify", "verify-provider-a.json", verify_evidence),
    ("metrics", "metrics.json", metrics_evidence),
    ("transport", "transport.json", transport_evidence),
    ("consumption", "routing-consumption.json", consumption_evidence),
)

EXPLICIT_EVIDENCE_FILES = (
    ("publish", "publish.json"),
    ("latest", "latest.json"),
    ("provider", "provider-provider-a.json"),
    ("events", "events.json"),
    ("verify", "verify-provider-a.json"),
    ("metrics", "metrics.json"),
    ("transport", "transport.json"),
    ("consumption", "routing-consumption.json"),
)


def test_schema_less_filename_fallback_accepts_only_reviewed_shapes() -> None:
    expected = {
        "publish.json": "publish",
        "latest.json": "latest",
        "snapshot.json": "latest",
        "events.json": "events",
        "metrics.json": "metrics",
        "transport.json": "transport",
        "consumption.json": "consumption",
        "routing-consumption.json": "consumption",
        "provider-provider-a.json": "provider",
        "provider-cli-output.json": "provider",
        "verify-provider-a.json": "verify",
    }

    for filename, kind in expected.items():
        assert MODULE.artifact_kind_from_name(Path(filename)) == kind


def test_schema_less_filename_fallback_rejects_alias_shapes() -> None:
    rejected = (
        "PROVIDER-provider-a.json",
        "provider_provider-a.json",
        "fetch-provider-a.json",
        "proof-provider-a.json",
        "proof-replay-provider-a.json",
        "metrics-extra.json",
        "event-log.json",
        "sse-transport.json",
        "routing.json",
        "publish-output.json",
        "latest-output.json",
    )

    for filename in rejected:
        assert MODULE.artifact_kind_from_name(Path(filename)) is None


def run_gate(root: Path, *extra: str) -> int:
    return CHECKER(["--evidence-dir", str(root), "--now-unix", str(NOW_UNIX), *extra])


def run_gate_with_explicit_evidence(root: Path, *extra: str) -> int:
    args = ["--now-unix", str(NOW_UNIX), "--require-provider", "provider-a"]
    for kind_name, file_name in EXPLICIT_EVIDENCE_FILES:
        args.extend(["--evidence", f"{kind_name}={root / file_name}"])
    args.extend(extra)
    return CHECKER(args)


def set_merkle_root_mismatch(payload: dict) -> None:
    if "events" in payload:
        payload["events"][-1]["merkle_root_hex"] = MERKLE_ROOT_2
    else:
        payload["merkle_root_hex"] = MERKLE_ROOT_2


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
    assert payload["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert payload["thresholds"] == {
        "max_snapshot_age_secs": MODULE.DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        "max_ingest_lag_secs": MODULE.DEFAULT_MAX_INGEST_LAG_SECS,
        "max_evidence_bytes": MODULE.MAX_EVIDENCE_BYTES,
    }
    assert payload["evidence_file_count"] == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert payload["recognized_artifact_count"] == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert payload["errors"] == []
    assert payload["required"]["transport"]["valid"] is True
    for kind_name, row in payload["required"].items():
        expected_schema = MODULE.KIND_BY_NAME[kind_name].schema
        assert row["schema"] == expected_schema
        assert row["present"] is True
        assert row["artifact_count"] == len(row["artifacts"])
        for artifact in row["artifacts"]:
            assert artifact["schema"] == expected_schema
            assert artifact["status"] == "passed"
            assert not artifact["path"].startswith("/")
            assert "\\" not in artifact["path"]
            assert "." not in artifact["path"].split("/")
            assert ".." not in artifact["path"].split("/")
            assert artifact["fingerprint"]["deployment_id"] == DEPLOYMENT_ID
            assert artifact["fingerprint"]["environment"] == ENVIRONMENT
            assert artifact["fingerprint"]["deployment_context_reviewed"] is True
    assert all(
        artifact["status"] == "passed" for artifact in payload["recognized_artifacts"]
    )
    assert payload["provider_ids"] == ["provider-a"]
    assert payload["metrics"] == sorted(MODULE.REQUIRED_METRICS)
    assert payload["metric_count_values"] == [len(MODULE.REQUIRED_METRICS)]
    assert payload["valid_reputation_weight_digests"] == [WEIGHTS_DIGEST]
    assert payload["valid_snapshot_bindings"] == [
        {
            "snapshot_id_hex": SNAPSHOT_ID,
            "merkle_root_hex": MERKLE_ROOT,
        }
    ]
    metrics_artifact = payload["required"]["metrics"]["artifacts"][0]
    assert metrics_artifact["fingerprint"]["metric_count"] == len(
        MODULE.REQUIRED_METRICS
    )
    assert metrics_artifact["fingerprint"]["metrics"] == list(MODULE.REQUIRED_METRICS)
    latest_artifact = payload["required"]["latest"]["artifacts"][0]
    assert latest_artifact["fingerprint"]["weights_digest_hex"] == WEIGHTS_DIGEST
    production_readiness = load_production_readiness_module()
    _aggregate_row, aggregate_errors = production_readiness.validate_gate_summary(
        production_readiness.GATE_BY_NAME["reputation"],
        payload,
        production_readiness.ValidationOptions(
            now_unix=NOW_UNIX,
            max_summary_artifact_age_secs=(
                production_readiness.DEFAULT_MAX_SUMMARY_ARTIFACT_AGE_SECS
            ),
            deployment_id=DEPLOYMENT_ID,
            environment=ENVIRONMENT,
        ),
    )
    assert aggregate_errors == []


def test_zero_based_provider_leaf_index_is_canonical(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    provider_path = tmp_path / "provider-provider-a.json"
    payload = json.loads(provider_path.read_text(encoding="utf-8"))
    payload["proof"]["leaf_index"] = 0
    write_json(provider_path, payload)

    assert run_gate(tmp_path, "--require-provider", "provider-a") == 0


def test_rollout_context_must_match_across_lane_artifacts(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    metrics_path = tmp_path / "metrics.json"
    payload = json.loads(metrics_path.read_text(encoding="utf-8"))
    payload["environment"] = "staging"
    write_json(metrics_path, payload)
    summary = tmp_path / "summary.json"

    assert run_gate(
        tmp_path,
        "--require-provider",
        "provider-a",
        "--summary-out",
        str(summary),
    ) == 1

    result = json.loads(summary.read_text(encoding="utf-8"))
    artifact = result["required"]["metrics"]["artifacts"][0]
    assert "metrics.environment does not match previous value" in artifact["errors"]


def test_snapshot_bound_fixture_table_covers_checker_bound_kind_set() -> None:
    assert (
        tuple(kind_name for kind_name, _file_name, _factory in SNAPSHOT_BOUND_FIXTURES)
        == MODULE.SNAPSHOT_BOUND_KINDS
    )


def test_all_snapshot_bound_artifacts_reject_publish_latest_mismatch(
    tmp_path: Path,
) -> None:
    for kind_name, file_name, factory in SNAPSHOT_BOUND_FIXTURES:
        case_dir = tmp_path / kind_name
        case_dir.mkdir()
        write_complete_evidence(case_dir)
        payload = factory()
        set_merkle_root_mismatch(payload)
        write_json(case_dir / file_name, payload)
        summary = case_dir / "summary.json"

        assert run_gate_with_explicit_evidence(
            case_dir,
            "--summary-out",
            str(summary),
        ) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        required = result["required"][kind_name]
        artifact = required["artifacts"][0]
        assert result["valid_snapshot_bindings"] == [
            {
                "snapshot_id_hex": SNAPSHOT_ID,
                "merkle_root_hex": MERKLE_ROOT,
            }
        ]
        assert required["valid"] is False
        assert artifact["valid"] is False
        assert artifact["status"] == "failed"
        recognized_artifact = next(
            artifact
            for artifact in result["recognized_artifacts"]
            if artifact["kind"] == kind_name
        )
        assert recognized_artifact["status"] == "failed"
        assert (
            f"{kind_name}.merkle_root_hex does not match previous value"
            in artifact["errors"]
        )


def test_payload_safety_flags_are_required(tmp_path: Path) -> None:
    cases = (
        ("metrics", "metrics.json", metrics_evidence, "response_bodies_included"),
        (
            "transport",
            "transport.json",
            transport_evidence,
            "response_bodies_included",
        ),
        (
            "consumption",
            "routing-consumption.json",
            consumption_evidence,
            "raw_provider_records_included",
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

        assert run_gate(
            root,
            "--require-provider",
            "provider-a",
            "--summary-out",
            str(summary),
        ) == 1

        result = json.loads(summary.read_text(encoding="utf-8"))
        artifact = result["required"][kind]["artifacts"][0]
        assert artifact["valid"] is False
        assert f"{field} must be false" in artifact["errors"]


def test_schema_less_explicit_evidence_advertises_required_schema(tmp_path: Path) -> None:
    latest_path = write_json(tmp_path / "latest-cli-output.json", snapshot_summary())
    provider_path = write_json(tmp_path / "provider-cli-output.json", provider_evidence())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence",
                f"latest={latest_path}",
                "--evidence",
                f"provider={provider_path}",
                "--require-kind",
                "latest,provider",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 0
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "ready"
    for kind_name, path in (
        ("latest", latest_path),
        ("provider", provider_path),
    ):
        expected_schema = MODULE.KIND_BY_NAME[kind_name].schema
        row = payload["required"][kind_name]
        assert "schema" not in json.loads(path.read_text(encoding="utf-8"))
        assert row["schema"] == expected_schema
        assert row["artifacts"][0]["schema"] == expected_schema
    recognized_schema_by_kind = {
        artifact["kind"]: artifact["schema"] for artifact in payload["recognized_artifacts"]
    }
    assert recognized_schema_by_kind == {
        "latest": MODULE.KIND_BY_NAME["latest"].schema,
        "provider": MODULE.KIND_BY_NAME["provider"].schema,
    }


def test_schema_less_directory_alias_filename_does_not_satisfy_provider(
    tmp_path: Path,
) -> None:
    payload = provider_evidence()
    write_json(tmp_path / "PROVIDER-provider-a.json", payload)
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence-dir",
                str(tmp_path),
                "--require-kind",
                "provider",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    report = json.loads(summary.read_text(encoding="utf-8"))
    assert report["status"] == "failed"
    assert report["recognized_artifacts"] == []
    provider_row = report["required"]["provider"]
    assert provider_row["present"] is False
    assert provider_row["artifact_count"] == 0
    assert "missing required `provider` evidence" in provider_row["errors"]


def test_response_file_arguments_pass(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    args = tmp_path / "reputation.args"
    args.write_text(
        f"--evidence-dir {tmp_path}\n--now-unix {NOW_UNIX}\n",
        encoding="utf-8",
    )

    assert CHECKER([f"@{args}"]) == 0


def test_deployment_context_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    payload = json.loads((tmp_path / "transport.json").read_text(encoding="utf-8"))
    payload.pop("deployment_context_reviewed")
    write_json(tmp_path / "transport.json", payload)
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    report = json.loads(summary.read_text(encoding="utf-8"))
    assert report["status"] == "failed"
    assert "deployment_context_reviewed must be true" in report["required"][
        "transport"
    ]["errors"]


def test_missing_evidence_sources_fail_shared_preflight(capsys) -> None:
    assert CHECKER(["--now-unix", str(NOW_UNIX)]) == 2

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


def test_snapshot_provider_count_must_match_unique_providers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    payload["provider_count"] = 3
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_snapshot_weight_digest_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    del payload["weights_digest_hex"]
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "weights_digest_hex must be a non-empty string" in artifact["errors"]
    assert payload["valid_reputation_weight_digests"] == [WEIGHTS_DIGEST]


def test_snapshot_weight_digest_must_be_lowercase_hex(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    payload["weights_digest_hex"] = "AB" * 32
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "weights_digest_hex must be 64 lowercase hex characters" in artifact["errors"]
    assert "AB" * 32 not in "\n".join(artifact["errors"])


def test_publish_and_latest_weight_digests_must_match(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    other_digest = "66" * 32
    payload["weights_digest_hex"] = other_digest
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["publish"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "publish.weights_digest_hex does not match previous value"
        in artifact["errors"]
    )
    assert other_digest not in "\n".join(artifact["errors"])


def test_snapshot_providers_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    payload["providers"].append({"name": "provider-a"})
    payload["provider_count"] = len(payload["providers"])
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert "providers must not contain duplicate values" in artifact["errors"]
    assert "provider_count must match unique providers count" in artifact["errors"]


def test_snapshot_provider_inventory_must_use_provider_ids(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    payload["providers"][0] = {"name": "provider_alpha"}
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert (
        "providers[].name must match canonical lowercase `provider-*`"
        in artifact["errors"]
    )


def test_snapshot_provider_inventory_rejects_non_production_markers(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = snapshot_summary()
    payload["providers"][0] = {"name": "provider-placeholder"}
    write_json(tmp_path / "latest.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["latest"]["artifacts"][0]
    assert (
        "providers[].name must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_events_must_carry_positive_limit(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = events_evidence()
    payload.pop("limit")
    write_json(tmp_path / "events.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["events"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "limit must be a positive integer" in artifact["errors"]


def test_events_count_must_not_exceed_limit(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = events_evidence()
    payload["limit"] = 1
    payload["events"].append(
        {
            "version": 1,
            "sequence": 2,
            "snapshot_id_hex": SNAPSHOT_ID,
            "generated_at_unix": GENERATED_AT,
            "merkle_root_hex": MERKLE_ROOT,
            "provider_count": 2,
        }
    )
    payload["count"] = 2
    payload["next_since"] = 2
    write_json(tmp_path / "events.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["events"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "count must be <= limit" in artifact["errors"]


def test_events_sequences_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = events_evidence()
    payload["events"].append(
        {
            "version": 1,
            "sequence": 1,
            "snapshot_id_hex": SNAPSHOT_ID,
            "generated_at_unix": GENERATED_AT,
            "merkle_root_hex": MERKLE_ROOT,
            "provider_count": 2,
        }
    )
    payload["count"] = 2
    payload["next_since"] = 2
    write_json(tmp_path / "events.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["events"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "events must not contain duplicate sequence values" in artifact["errors"]
    assert "count must match unique events sequence count" in artifact["errors"]


def test_events_all_rows_must_match_snapshot_binding(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = events_evidence()
    payload["events"][0]["merkle_root_hex"] = MERKLE_ROOT_2
    payload["events"].append(
        {
            "version": 1,
            "sequence": 2,
            "snapshot_id_hex": SNAPSHOT_ID,
            "generated_at_unix": GENERATED_AT,
            "merkle_root_hex": MERKLE_ROOT,
            "provider_count": 2,
        }
    )
    payload["count"] = 2
    payload["next_since"] = 2
    write_json(tmp_path / "events.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["events"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "events[].merkle_root_hex must match first event merkle_root_hex"
        in artifact["errors"]
    )


def test_events_provider_count_must_match_across_rows(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = events_evidence()
    payload["events"].append(
        {
            "version": 1,
            "sequence": 2,
            "snapshot_id_hex": SNAPSHOT_ID,
            "generated_at_unix": GENERATED_AT,
            "merkle_root_hex": MERKLE_ROOT,
            "provider_count": 3,
        }
    )
    payload["count"] = 2
    payload["next_since"] = 2
    write_json(tmp_path / "events.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["events"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "events[].provider_count must match first event provider_count"
        in artifact["errors"]
    )


def test_events_rows_must_be_v1(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = events_evidence()
    payload["events"][0]["version"] = 2
    write_json(tmp_path / "events.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["events"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "events[].version must be 1" in artifact["errors"]


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


def test_provider_id_must_be_canonical(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "provider-provider-a.json",
        provider_evidence(provider_id="provider_alpha"),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "provider.provider_id must match canonical lowercase `provider-*`"
        in artifact["errors"]
    )


def test_provider_id_rejects_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "provider-provider-a.json",
        provider_evidence(provider_id="provider-prod-placeholder"),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "provider.provider_id must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )
    assert (
        "proof.provider_id must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_provider_id_non_production_marker_stdout_does_not_echo_provider_id(
    tmp_path: Path,
    capsys,
) -> None:
    write_complete_evidence(tmp_path)
    invalid_provider_id = "provider-prod-placeholder"
    write_json(
        tmp_path / "provider-provider-a.json",
        provider_evidence(provider_id=invalid_provider_id),
    )

    assert run_gate(tmp_path) == 1

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert (
        "provider.provider_id must not contain non-production markers ['placeholder']"
        in diagnostics
    )
    assert (
        "proof.provider_id must not contain non-production markers ['placeholder']"
        in diagnostics
    )
    assert invalid_provider_id not in diagnostics
    assert_reputation_stderr_is_sanitized(captured.err, invalid_provider_id)


def test_verify_provider_id_rejects_non_production_markers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "verify-provider-a.json",
        verify_evidence(provider_id="provider-prod-placeholder"),
    )

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["verify"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "provider_id must not contain non-production markers ['placeholder']"
        in artifact["errors"]
    )


def test_verify_provider_id_non_production_marker_stdout_does_not_echo_provider_id(
    tmp_path: Path,
    capsys,
) -> None:
    write_complete_evidence(tmp_path)
    invalid_provider_id = "provider-prod-placeholder"
    write_json(
        tmp_path / "verify-provider-a.json",
        verify_evidence(provider_id=invalid_provider_id),
    )

    assert run_gate(tmp_path) == 1

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert (
        "provider_id must not contain non-production markers ['placeholder']"
        in diagnostics
    )
    assert invalid_provider_id not in diagnostics
    assert_reputation_stderr_is_sanitized(captured.err, invalid_provider_id)


def test_provider_id_accepts_future_production_label(tmp_path: Path) -> None:
    provider_id = "provider-prod-a-202607"
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    write_json(
        tmp_path / "provider-provider-a.json",
        provider_evidence(provider_id=provider_id),
    )
    write_json(
        tmp_path / "verify-provider-a.json",
        verify_evidence(provider_id=provider_id),
    )

    assert (
        run_gate(
            tmp_path,
            "--require-provider",
            provider_id,
            "--summary-out",
            str(summary),
        )
        == 0
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "ready"
    assert payload["provider_ids"] == [provider_id]


def test_provider_proof_siblings_must_be_unique(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = provider_evidence()
    payload["proof"]["siblings_hex"].append(payload["proof"]["siblings_hex"][0])
    write_json(tmp_path / "provider-provider-a.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["provider"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "proof.siblings_hex[1] must be unique" in artifact["errors"]


def test_invalid_duplicate_artifact_fails_even_with_valid_artifact(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "snapshot.json",
        snapshot_summary(generated_at=NOW_UNIX - 900_000),
    )

    assert run_gate(tmp_path) == 1


def test_snapshot_id_mismatch_fails(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(tmp_path / "provider-provider-a.json", provider_evidence(snapshot_id="44" * 16))

    assert run_gate(tmp_path) == 1


def test_required_provider_must_have_proof(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    provider_id = "provider-b-private-key-placeholder"
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-provider",
            provider_id,
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    diagnostics = json.dumps(payload, sort_keys=True)
    assert "missing provider/proof evidence for required provider" in diagnostics
    assert provider_id not in diagnostics


def test_required_provider_stdout_does_not_echo_provider_id(
    tmp_path: Path,
    capsys,
) -> None:
    write_complete_evidence(tmp_path)
    provider_id = "provider-b-private-key-placeholder"

    assert run_gate(tmp_path, "--require-provider", provider_id) == 1

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert "missing provider/proof evidence for required provider" in diagnostics
    assert "missing proof verification evidence for required provider" in diagnostics
    assert provider_id not in diagnostics
    assert_reputation_stderr_is_sanitized(captured.err, provider_id)


def test_required_provider_needs_matching_proof_not_only_verification(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "verify-provider-a.json",
        verify_evidence(provider_id="provider-b"),
    )
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-provider",
            "provider-b",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    provider_row = payload["required"]["provider"]
    assert provider_row["valid"] is False
    assert (
        "missing provider/proof evidence for required provider"
        in provider_row["errors"]
    )


def test_required_provider_needs_matching_verification_not_only_proof(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "provider-provider-a.json",
        provider_evidence(provider_id="provider-b"),
    )
    summary = tmp_path / "summary.json"

    assert (
        run_gate(
            tmp_path,
            "--require-provider",
            "provider-b",
            "--summary-out",
            str(summary),
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    verify_row = payload["required"]["verify"]
    assert verify_row["valid"] is False
    assert (
        "missing proof verification evidence for required provider"
        in verify_row["errors"]
    )


def test_fallback_requires_same_provider_to_have_proof_and_verification(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "verify-provider-a.json",
        verify_evidence(provider_id="provider-b"),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    provider_row = payload["required"]["provider"]
    assert provider_row["valid"] is False
    assert (
        "at least one provider proof must be verified"
        in provider_row["errors"]
    )


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


def test_metrics_freshness_and_lag_must_be_integers(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    write_json(
        tmp_path / "metrics.json",
        metrics_evidence(snapshot_age=12.5, ingest_lag=7.5),
    )
    summary = tmp_path / "summary.json"

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert artifact["valid"] is False
    assert "snapshot_age_seconds must be a non-negative integer" in artifact["errors"]
    assert "ingest_lag_seconds must be a non-negative integer" in artifact["errors"]


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


def test_metrics_response_bodies_flag_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    del payload["response_bodies_included"]
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert "response_bodies_included must be false" in artifact["errors"]


def test_metrics_metric_count_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    payload.pop("metric_count")
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert "metric_count must be a positive integer" in artifact["errors"]


def test_metrics_metric_count_must_match_unique_metrics(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    payload["metric_count"] += 1
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_metrics_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    payload["metrics"].append(payload["metrics"][0])
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert "metrics must not contain duplicate values" in artifact["errors"]
    assert "metric_count must match unique metrics count" in artifact["errors"]


def test_metrics_must_cover_required_metrics(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    missing = payload["metrics"].pop()
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert f"metrics must include value `{missing}`" in artifact["errors"]


def test_metrics_must_not_include_unknown_values(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = metrics_evidence()
    payload["metrics"].append("sorafs_reputation_unknown_metric_total")
    payload["metric_count"] = len(payload["metrics"])
    write_json(tmp_path / "metrics.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["metrics"]["artifacts"][0]
    assert "metrics must not include unknown values" in artifact["errors"]


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
        "transport.merkle_root_hex does not match previous value",
    ]
    joined = "\n".join(artifact["errors"])
    assert MERKLE_ROOT_2 not in joined
    assert MERKLE_ROOT not in joined


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


def test_transport_response_bodies_flag_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    del payload["response_bodies_included"]
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert "response_bodies_included must be false" in artifact["errors"]


def test_transport_sse_event_count_must_match_unique_events(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["sse_event_count"] += 1
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert "sse_event_count must match unique sse_events count" in artifact["errors"]


def test_transport_sse_events_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["sse_events"].append(dict(payload["sse_events"][0]))
    payload["sse_event_count"] = len(payload["sse_events"])
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert "sse_events must not contain duplicate values" in artifact["errors"]
    assert "sse_event_count must match unique sse_events count" in artifact["errors"]


def test_transport_sse_events_must_use_production_family(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["sse_events"][0]["name"] = "sse-snapshot-00"
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.SSE_EVENT_LABEL_ERROR in artifact["errors"]


def test_transport_sse_events_reject_placeholder_marker(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["sse_events"][0]["name"] = "reputation-sse-event-placeholder"
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "sse_events[0].name must not contain non-production markers "
        "['placeholder']"
        in artifact["errors"]
    )


def test_transport_websocket_event_count_must_match_unique_events(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["websocket_event_count"] += 1
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert (
        "websocket_event_count must match unique websocket_events count"
        in artifact["errors"]
    )


def test_transport_websocket_events_must_not_duplicate(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["websocket_events"].append(dict(payload["websocket_events"][0]))
    payload["websocket_event_count"] = len(payload["websocket_events"])
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert "websocket_events must not contain duplicate values" in artifact["errors"]
    assert (
        "websocket_event_count must match unique websocket_events count"
        in artifact["errors"]
    )


def test_transport_websocket_events_must_use_production_family(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["websocket_events"][0]["name"] = "websocket-snapshot-00"
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert MODULE.WEBSOCKET_EVENT_LABEL_ERROR in artifact["errors"]


def test_transport_websocket_events_reject_placeholder_marker(
    tmp_path: Path,
) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = transport_evidence()
    payload["websocket_events"][0]["name"] = (
        "reputation-websocket-event-placeholder"
    )
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["transport"]["artifacts"][0]
    assert artifact["valid"] is False
    assert (
        "websocket_events[0].name must not contain non-production markers "
        "['placeholder']"
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


def test_consumption_raw_provider_records_flag_is_required(tmp_path: Path) -> None:
    write_complete_evidence(tmp_path)
    summary = tmp_path / "summary.json"
    payload = consumption_evidence()
    del payload["raw_provider_records_included"]
    write_json(tmp_path / "routing-consumption.json", payload)

    assert run_gate(tmp_path, "--summary-out", str(summary)) == 1

    payload = json.loads(summary.read_text(encoding="utf-8"))
    artifact = payload["required"]["consumption"]["artifacts"][0]
    assert "raw_provider_records_included must be false" in artifact["errors"]


def test_invalid_optional_artifact_fails_subset_gate(tmp_path: Path) -> None:
    write_json(tmp_path / "latest.json", snapshot_summary())
    payload = transport_evidence()
    payload["response_body"] = {"event": "raw frame"}
    write_json(tmp_path / "transport.json", payload)

    assert run_gate(tmp_path, "--require-kind", "latest") == 1


def test_explicit_unknown_schema_fails(tmp_path: Path) -> None:
    unknown_schema = "sorafs.reputation.unknown.private-key-placeholder.v1"
    path = write_json(tmp_path / "unknown.json", {"schema": unknown_schema})
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence",
                str(path),
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    load_errors = "\n".join(payload["load_errors"])
    assert "unknown reputation evidence schema" in load_errors
    assert unknown_schema not in load_errors
    assert str(path) not in load_errors


def test_explicit_unknown_schema_stdout_does_not_echo_schema_or_path(
    tmp_path: Path,
    capsys,
) -> None:
    unknown_schema = "sorafs.reputation.unknown.private-key-placeholder.v1"
    path = write_json(tmp_path / "unknown.json", {"schema": unknown_schema})

    assert (
        CHECKER(
            [
                "--evidence",
                str(path),
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    load_errors = "\n".join(payload["load_errors"])
    assert "unknown reputation evidence schema" in load_errors
    assert unknown_schema not in load_errors
    assert "unknown.json" not in load_errors
    assert str(path) not in load_errors
    assert_reputation_stderr_is_sanitized(
        captured.err,
        unknown_schema,
        "unknown.json",
        str(path),
    )


def test_explicit_untyped_evidence_without_inferred_kind_does_not_echo_path(
    tmp_path: Path,
) -> None:
    path = write_json(tmp_path / "opaque.json", deployment_context())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence",
                str(path),
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    load_errors = "\n".join(payload["load_errors"])
    assert "cannot infer evidence kind" in load_errors
    assert "opaque.json" not in load_errors
    assert str(path) not in load_errors


def test_explicit_untyped_evidence_stdout_without_inferred_kind_does_not_echo_path(
    tmp_path: Path,
    capsys,
) -> None:
    path = write_json(tmp_path / "opaque.json", deployment_context())

    assert (
        CHECKER(
            [
                "--evidence",
                str(path),
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    load_errors = "\n".join(payload["load_errors"])
    assert "cannot infer evidence kind" in load_errors
    assert "opaque.json" not in load_errors
    assert str(path) not in load_errors
    assert_reputation_stderr_is_sanitized(
        captured.err,
        "opaque.json",
        str(path),
    )


def test_explicit_kind_must_match_recognized_schema(tmp_path: Path) -> None:
    path = write_json(tmp_path / "typed.json", transport_evidence())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence",
                f"metrics={path}",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "failed"
    assert any(
        error == "evidence schema does not match explicit kind"
        for error in payload["load_errors"]
    )
    assert "transport" not in "\n".join(payload["load_errors"])
    assert "metrics" not in "\n".join(payload["load_errors"])
    assert str(path) not in "\n".join(payload["load_errors"])


def test_explicit_kind_schema_mismatch_stdout_does_not_echo_kind_or_path(
    tmp_path: Path,
    capsys,
) -> None:
    path = write_json(tmp_path / "typed.json", transport_evidence())

    assert (
        CHECKER(
            [
                "--evidence",
                f"metrics={path}",
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    load_errors = "\n".join(payload["load_errors"])
    assert "evidence schema does not match explicit kind" in load_errors
    assert "transport" not in load_errors
    assert "metrics" not in load_errors
    assert "typed.json" not in load_errors
    assert str(path) not in load_errors
    assert_reputation_stderr_is_sanitized(
        captured.err,
        "transport",
        "metrics",
        "typed.json",
        str(path),
    )


def test_explicit_same_path_conflicting_kinds_fail(tmp_path: Path) -> None:
    path = write_json(tmp_path / "typed.json", transport_evidence())
    summary = tmp_path / "summary.json"

    assert (
        CHECKER(
            [
                "--evidence",
                f"transport={path}",
                "--evidence",
                f"metrics={path}",
                "--now-unix",
                str(NOW_UNIX),
                "--summary-out",
                str(summary),
            ]
        )
        == 1
    )

    payload = json.loads(summary.read_text(encoding="utf-8"))
    assert payload["status"] == "failed"
    assert any(
        error == "explicit evidence kind conflicts with previous kind"
        for error in payload["load_errors"]
    )
    assert "transport" not in "\n".join(payload["load_errors"])
    assert "metrics" not in "\n".join(payload["load_errors"])
    assert str(path) not in "\n".join(payload["load_errors"])


def test_explicit_conflicting_kinds_stdout_does_not_echo_kind_or_path(
    tmp_path: Path,
    capsys,
) -> None:
    path = write_json(tmp_path / "typed.json", transport_evidence())

    assert (
        CHECKER(
            [
                "--evidence",
                f"transport={path}",
                "--evidence",
                f"metrics={path}",
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    load_errors = "\n".join(payload["load_errors"])
    assert "explicit evidence kind conflicts with previous kind" in load_errors
    assert "transport" not in load_errors
    assert "metrics" not in load_errors
    assert "typed.json" not in load_errors
    assert str(path) not in load_errors
    assert_reputation_stderr_is_sanitized(
        captured.err,
        "transport",
        "metrics",
        "typed.json",
        str(path),
    )


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


def test_malformed_explicit_evidence_kind_main_summary_sanitizes_exception_text(
    monkeypatch,
    capsys,
) -> None:
    bad_message = "latest\nshadow"

    def raise_malformed_evidence_spec(_spec: str):
        raise ValueError(bad_message)

    monkeypatch.setattr(MODULE, "parse_evidence_spec", raise_malformed_evidence_spec)

    assert (
        CHECKER(
            [
                "--evidence",
                "latest=/tmp/evidence.json",
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert "<non-canonical-error>" in diagnostics
    assert bad_message not in diagnostics
    assert_reputation_stderr_is_sanitized(captured.err, bad_message)


def test_unknown_explicit_evidence_kind_does_not_echo_kind() -> None:
    unknown_kind = "private-key-placeholder"

    loaded, errors = MODULE.load_evidence([], [f"{unknown_kind}=/tmp/evidence.json"])

    diagnostics = "\n".join(errors)
    assert loaded == []
    assert "unknown evidence kind" in diagnostics
    assert unknown_kind not in diagnostics


def test_typed_explicit_evidence_rejects_padded_kind_or_path_without_trimming(
    tmp_path: Path,
) -> None:
    path = write_json(tmp_path / "latest.json", snapshot_summary())
    cases = (
        f"latest ={path}",
        f"latest={path} ",
        f"latest\u200d={path}",
        f"latest={path}\u202e",
    )

    for spec in cases:
        loaded, errors = MODULE.load_evidence([], [spec])
        diagnostics = "\n".join(errors)
        escaped_spec = spec.encode("unicode_escape").decode("ascii")

        assert loaded == []
        assert "evidence spec must use KIND=PATH form" in diagnostics
        assert spec not in diagnostics
        assert escaped_spec not in diagnostics


def test_unknown_explicit_evidence_kind_main_summary_does_not_echo_kind(
    capsys,
) -> None:
    unknown_kind = "private-key-placeholder"

    assert (
        CHECKER(
            [
                "--evidence",
                f"{unknown_kind}=/tmp/evidence.json",
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 1
    )

    captured = capsys.readouterr()
    payload = json.loads(captured.out)
    diagnostics = json.dumps(payload, sort_keys=True)
    assert "unknown evidence kind" in diagnostics
    assert unknown_kind not in diagnostics
    assert_reputation_stderr_is_sanitized(captured.err, unknown_kind)


def test_typed_explicit_evidence_unsafe_path_fails_before_load_without_leaking(
    tmp_path: Path,
    capsys,
) -> None:
    assert (
        CHECKER(
            [
                "--evidence",
                f"latest={tmp_path / 'private%26%2395%3Bkey.json'}",
                "--now-unix",
                str(NOW_UNIX),
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "SoraFS checker-rendered paths must not contain secret-looking"
        in captured.err
    )
    assert "private%26%2395%3Bkey" not in captured.err
    assert "private&#95;key" not in captured.err
    assert "private_key" not in captured.err
    assert "unknown evidence kind" not in captured.err
    assert captured.out == ""


def test_typed_explicit_evidence_empty_path_fails_before_load_without_leaking(
    capsys,
) -> None:
    assert CHECKER(["--evidence", "latest=", "--now-unix", str(NOW_UNIX)]) == 2

    captured = capsys.readouterr()
    assert "--evidence must use canonical path or KIND=PATH spec" in captured.err
    assert "latest=" not in captured.err
    assert "unknown evidence kind" not in captured.err
    assert captured.out == ""


def test_unsupported_loaded_evidence_kind_is_sanitized(tmp_path: Path) -> None:
    unsupported_kind = "unsupported-private-key-placeholder"
    summary = MODULE.validate_evidence_set(
        [
            MODULE.LoadedEvidence(
                unsupported_kind,
                tmp_path / "unsupported.json",
                {
                    **deployment_context(),
                    "schema": "sorafs.reputation.unsupported.v1",
                    "status": "ready",
                },
                "ab" * 32,
            )
        ],
        required_kinds=("latest",),
        required_providers=(),
        now_unix=NOW_UNIX,
        max_snapshot_age_secs=MODULE.DEFAULT_MAX_SNAPSHOT_AGE_SECS,
        max_ingest_lag_secs=MODULE.DEFAULT_MAX_INGEST_LAG_SECS,
    )

    diagnostics = json.dumps(summary, sort_keys=True)
    assert summary["recognized_artifacts"][0]["kind"] == "<unknown>"
    assert "unsupported evidence kind" in diagnostics
    assert unsupported_kind not in diagnostics


def test_prefixed_explicit_evidence_matching_summary_out_fails_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    path = write_json(tmp_path / "evidence.json", snapshot_summary())
    original = path.read_text(encoding="utf-8")
    summary_out = path

    assert (
        CHECKER(
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

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1

    captured = capsys.readouterr()
    summary = json.loads(captured.out)
    assert any(
        error.startswith("failed to load evidence JSON:")
        for error in summary["load_errors"]
    )
    assert str(path) not in "\n".join(summary["load_errors"])


def test_explicit_missing_file_reports_discovery_error(
    tmp_path: Path, capsys
) -> None:
    path = tmp_path / "missing.json"

    assert CHECKER(["--evidence", str(path), "--now-unix", str(NOW_UNIX)]) == 1

    captured = capsys.readouterr()
    summary = json.loads(captured.out)
    assert summary["load_errors"] == ["evidence file must exist and be a file"]
    assert str(path) not in "\n".join(summary["load_errors"])
