"""Tests for scripts/build_sorafs_reputation_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_reputation_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_reputation_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_reputation_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reputation_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_100_000
GENERATED_AT = NOW_UNIX - 120
SNAPSHOT_ID = "11" * 16
MERKLE_ROOT = "22" * 32
PROVIDER_ID = "provider-a"
PROOF_SIBLING = "33" * 32


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "sorafs-mainnet-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
        "--snapshot-id-hex",
        SNAPSHOT_ID,
        "--merkle-root-hex",
        MERKLE_ROOT,
        "--provider-id",
        PROVIDER_ID,
        "--provider-name",
        "provider-a",
        "--provider-name",
        "provider-b",
    ]
    if kind == "provider":
        args.extend(["--sibling-hex", PROOF_SIBLING])
    if kind == "metrics":
        for metric in CHECKER.REQUIRED_METRICS:
            args.extend(["--metric", metric])
    if kind == "transport":
        args.extend(["--sse-event", "reputation-sse-event-snapshot-00"])
        args.extend(
            ["--websocket-event", "reputation-websocket-event-snapshot-00"]
        )
    return args


def assert_rejected_without_artifact(
    args: list[str],
    *,
    kind: str,
    tmp_path: Path,
    capsys,
    expected_error: str,
) -> None:
    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert expected_error in captured.err
    assert not canary_path(tmp_path, kind).exists()


def test_builds_payload_free_metrics_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("metrics", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "metrics").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reputation.metrics_canary.v1"
    assert payload["status"] == "passed"
    assert payload["metrics_scrape_success"] is True
    assert payload["provider_count"] == 2
    assert payload["providers"] == [{"name": "provider-a"}, {"name": "provider-b"}]
    assert payload["metric_count"] == len(CHECKER.REQUIRED_METRICS)
    assert payload["metrics"] == list(CHECKER.REQUIRED_METRICS)
    assert payload["response_bodies_included"] is False
    errors = MODULE.validate_generated_payload(payload, MODULE.parse_args(args_for("metrics", tmp_path)))
    assert errors == []


def test_builds_payload_free_transport_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("transport", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "transport").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reputation.transport_canary.v1"
    assert payload["sse_event_count"] == 1
    assert payload["sse_events"] == [{"name": "reputation-sse-event-snapshot-00"}]
    assert payload["websocket_event_count"] == 1
    assert payload["websocket_events"] == [
        {"name": "reputation-websocket-event-snapshot-00"}
    ]
    assert payload["response_bodies_included"] is False
    errors = MODULE.validate_generated_payload(
        payload,
        MODULE.parse_args(args_for("transport", tmp_path)),
    )
    assert errors == []


def test_generated_canaries_pass_full_reputation_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = ["--now-unix", str(NOW_UNIX), "--require-provider", PROVIDER_ID]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["snapshot_id_hex"] == SNAPSHOT_ID
    assert payload["merkle_root_hex"] == MERKLE_ROOT
    assert payload["provider_ids"] == [PROVIDER_ID]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_provider_id_must_be_canonical(tmp_path: Path, capsys) -> None:
    args = args_for("provider", tmp_path)
    args[args.index("--provider-id") + 1] = "provider_alpha"

    assert_rejected_without_artifact(
        args,
        kind="provider",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error="--provider-id must match canonical lowercase `provider-*`",
    )


def test_provider_id_rejects_non_production_markers(tmp_path: Path, capsys) -> None:
    args = args_for("provider", tmp_path)
    args[args.index("--provider-id") + 1] = "provider-prod-placeholder"

    assert_rejected_without_artifact(
        args,
        kind="provider",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=(
            "--provider-id must not contain non-production markers ['placeholder']"
        ),
    )


def test_provider_id_accepts_future_production_label(tmp_path: Path) -> None:
    provider_id = "provider-prod-a-202607"
    args = args_for("provider", tmp_path)
    args[args.index("--provider-id") + 1] = provider_id

    assert MODULE.main(args) == 0

    payload = json.loads(canary_path(tmp_path, "provider").read_text("utf-8"))
    assert payload["provider"]["provider_id"] == provider_id
    assert payload["proof"]["provider_id"] == provider_id


def test_response_file_can_build_provider_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "provider.args"
    args_file.write_text(
        "\n".join(args_for("provider", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "provider").read_text("utf-8"))
    assert payload["provider"]["provider_id"] == PROVIDER_ID
    assert payload["proof"]["siblings_hex"] == [PROOF_SIBLING]


def test_provider_requires_proof_sibling_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("provider", tmp_path)
    index = args.index("--sibling-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sibling-hex is required for provider" in captured.err
    assert not canary_path(tmp_path, "provider").exists()


def test_duplicate_provider_proof_sibling_fails_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("provider", tmp_path)
    args.extend(["--sibling-hex", PROOF_SIBLING])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "duplicate --sibling-hex" in captured.err
    assert not canary_path(tmp_path, "provider").exists()


def test_metrics_thresholds_fail_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--snapshot-age-seconds", "691201", "--ingest-lag-seconds", "901"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--snapshot-age-seconds must be <=" in captured.err
    assert "--ingest-lag-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_is_required_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("metrics", tmp_path)
    while "--provider-name" in args:
        index = args.index("--provider-name")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-name is required" in captured.err
    assert (
        "--provider-count must match the number of unique --provider-name values"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_must_match_count_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("metrics", tmp_path)
    provider_count_index = args.index("--provider-count") if "--provider-count" in args else -1
    if provider_count_index == -1:
        args.extend(["--provider-count", "3"])
    else:
        args[provider_count_index + 1] = "3"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider-count must match the number of unique --provider-name values"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_must_not_duplicate_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--provider-name", "provider-a"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--provider-name must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_must_use_provider_ids_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args[args.index("--provider-name") + 1] = "provider_alpha"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider-name must match canonical lowercase `provider-*`"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_provider_inventory_rejects_non_production_markers_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args[args.index("--provider-name") + 1] = "provider-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--provider-name must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "metrics").exists()


def test_metrics_inventory_is_required_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("metrics", tmp_path)
    while "--metric" in args:
        index = args.index("--metric")
        del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must include every required value" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_metrics_inventory_must_not_duplicate_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--metric", CHECKER.REQUIRED_METRICS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


def test_metrics_inventory_must_not_include_unknown_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("metrics", tmp_path)
    args.extend(["--metric", "sorafs_reputation_unknown_metric_total"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--metric contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "metrics").exists()


@pytest.mark.parametrize(
    ("option", "duplicate_value", "unknown_value"),
    (
        (
            "--metric",
            CHECKER.REQUIRED_METRICS[0],
            "sorafs_reputation_unknown_metric_total",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for("metrics", tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind="metrics",
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for("metrics", unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind="metrics",
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_transport_sse_event_inventory_must_match_count_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--sse-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sse-event unique values must match --sse-event-count" in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_sse_event_inventory_must_not_duplicate_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--sse-event", "reputation-sse-event-snapshot-00"])
    args.extend(["--sse-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--sse-event must not contain duplicates" in captured.err
    assert "--sse-event unique values must match --sse-event-count" in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_sse_event_inventory_requires_production_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--sse-event") + 1] = "sse-snapshot-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert MODULE.SSE_EVENT_LABEL_ERROR in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_sse_event_inventory_rejects_placeholder_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--sse-event") + 1] = "reputation-sse-event-placeholder"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--sse-event[0] must not contain non-production markers ['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_must_match_count_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--websocket-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--websocket-event unique values must match --websocket-event-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_must_not_duplicate_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args.extend(["--websocket-event", "reputation-websocket-event-snapshot-00"])
    args.extend(["--websocket-event-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--websocket-event must not contain duplicates" in captured.err
    assert (
        "--websocket-event unique values must match --websocket-event-count"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_requires_production_family_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--websocket-event") + 1] = "websocket-snapshot-00"

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert MODULE.WEBSOCKET_EVENT_LABEL_ERROR in captured.err
    assert not canary_path(tmp_path, "transport").exists()


def test_transport_websocket_event_inventory_rejects_placeholder_before_write(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("transport", tmp_path)
    args[args.index("--websocket-event") + 1] = (
        "reputation-websocket-event-placeholder"
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--websocket-event[0] must not contain non-production markers "
        "['placeholder']"
        in captured.err
    )
    assert not canary_path(tmp_path, "transport").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("latest", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_rejected(tmp_path: Path, capsys) -> None:
    output_dir = tmp_path / "latest-output"
    output_dir.mkdir()
    args = args_for("latest", tmp_path)
    args[args.index("--out") + 1] = str(output_dir)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--out" in captured.err
    assert "must not be a directory" in captured.err
    assert output_dir.is_dir()
