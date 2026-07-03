"""Tests for scripts/build_sorafs_transparency_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import pytest


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_transparency_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_transparency_rollout_evidence.py"

SPEC = importlib.util.spec_from_file_location("build_sorafs_transparency_canary", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_transparency_rollout_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


GENERATED_AT = 1_800_000_120
SOURCE_BATCH_DIGEST = "a" * 64
CYCLE_DIGEST = "b" * 64


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "transparency-mainnet-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
    ]
    if kind in MODULE.SOURCE_BATCH_DIGEST_KINDS:
        args.extend(["--source-batch-digest-hex", SOURCE_BATCH_DIGEST])
    if kind in MODULE.CYCLE_DIGEST_KINDS:
        args.extend(["--cycle-digest-hex", CYCLE_DIGEST])
    if kind == "source_entry":
        for source_kind in MODULE.DEFAULT_REQUIRED_SOURCE_KINDS:
            args.extend(["--source-kind", source_kind])
    elif kind == "publication":
        for route in MODULE.REQUIRED_PUBLICATION_ROUTES:
            args.extend(["--publication-route", route])
        for probe in MODULE.REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES:
            args.extend(["--cycle-detail-probe", probe])
    elif kind == "privacy_aggregate":
        for action in MODULE.REQUIRED_PRIVACY_AGGREGATE_ACTIONS:
            args.extend(["--privacy-action", action])
    elif kind == "explorer":
        for route in MODULE.REQUIRED_EXPLORER_ROUTES:
            args.extend(["--explorer-route", route])
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


def test_builds_payload_free_source_entry_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("source_entry", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "source_entry").read_text("utf-8"))

    assert payload["schema"] == "sorafs.transparency.source_entry.canary.v1"
    assert payload["status"] == "passed"
    assert payload["source_batch_digest_hex"] == SOURCE_BATCH_DIGEST
    assert payload["payload_bytes_included"] is False
    assert payload["private_payloads_included"] is False
    assert payload["response_bodies_included"] is False
    errors = MODULE.validate_generated_payload(
        payload,
        MODULE.parse_args(args_for("source_entry", tmp_path)),
    )
    assert errors == []


def test_builds_payload_free_proof_token_issuance_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("proof_token_issuance", tmp_path)) == 0

    payload = json.loads(
        canary_path(tmp_path, "proof_token_issuance").read_text("utf-8")
    )

    assert payload["schema"] == "sorafs.transparency.proof_token_issuance.canary.v1"
    assert payload["cycle_digest_hex"] == CYCLE_DIGEST
    assert payload["probe_count"] == 1
    assert payload["issuance_probe_count"] == 1
    assert [probe["action"] for probe in payload["probes"]] == [
        "proof_token_issuance"
    ]
    errors = MODULE.validate_generated_payload(
        payload,
        MODULE.parse_args(args_for("proof_token_issuance", tmp_path)),
    )
    assert errors == []


def test_generated_canaries_pass_full_transparency_gate(tmp_path: Path) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = []
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_source_batch_digests"] == [SOURCE_BATCH_DIGEST]
    assert payload["valid_cycle_digests"] == [CYCLE_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_publication_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "publication.args"
    args_file.write_text(
        "\n".join(args_for("publication", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "publication").read_text("utf-8"))
    assert payload["source_batch_digest_hex"] == SOURCE_BATCH_DIGEST
    assert payload["cycle_digest_hex"] == CYCLE_DIGEST
    assert [route["name"] for route in payload["routes"]] == list(
        MODULE.REQUIRED_PUBLICATION_ROUTES
    )
    assert [probe["name"] for probe in payload["cycle_detail_probes"]] == list(
        MODULE.REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES
    )


def test_missing_source_kind_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("source_entry", tmp_path)
    index = args.index("--source-kind")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--source-kind must include every required value" in captured.err
    assert not canary_path(tmp_path, "source_entry").exists()


def test_duplicate_source_kind_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("source_entry", tmp_path)
    args.extend(["--source-kind", MODULE.DEFAULT_REQUIRED_SOURCE_KINDS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--source-kind must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "source_entry").exists()


def test_unknown_source_kind_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("source_entry", tmp_path)
    args.extend(["--source-kind", "shadow-source-kind"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--source-kind contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "source_entry").exists()


def test_duplicate_publication_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publication", tmp_path)
    args.extend(["--publication-route", MODULE.REQUIRED_PUBLICATION_ROUTES[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--publication-route must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "publication").exists()


def test_unknown_publication_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publication", tmp_path)
    args.extend(["--publication-route", "shadow-publication-route"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--publication-route contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "publication").exists()


def test_missing_cycle_detail_probe_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publication", tmp_path)
    index = args.index("--cycle-detail-probe")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--cycle-detail-probe must include every required value" in captured.err
    assert not canary_path(tmp_path, "publication").exists()


def test_duplicate_cycle_detail_probe_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publication", tmp_path)
    args.extend(
        ["--cycle-detail-probe", MODULE.REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES[0]]
    )

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--cycle-detail-probe must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "publication").exists()


def test_unknown_cycle_detail_probe_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publication", tmp_path)
    args.extend(["--cycle-detail-probe", "shadow-cycle-detail-probe"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--cycle-detail-probe contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "publication").exists()


def test_duplicate_privacy_action_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("privacy_aggregate", tmp_path)
    args.extend(["--privacy-action", MODULE.REQUIRED_PRIVACY_AGGREGATE_ACTIONS[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--privacy-action must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "privacy_aggregate").exists()


def test_unknown_privacy_action_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("privacy_aggregate", tmp_path)
    args.extend(["--privacy-action", "shadow-privacy-action"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--privacy-action contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "privacy_aggregate").exists()


def test_duplicate_explorer_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("explorer", tmp_path)
    args.extend(["--explorer-route", MODULE.REQUIRED_EXPLORER_ROUTES[0]])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--explorer-route must not contain duplicates" in captured.err
    assert not canary_path(tmp_path, "explorer").exists()


def test_unknown_explorer_route_coverage_fails_closed(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("explorer", tmp_path)
    args.extend(["--explorer-route", "shadow-explorer-route"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--explorer-route contains an unknown value" in captured.err
    assert not canary_path(tmp_path, "explorer").exists()


@pytest.mark.parametrize(
    ("kind", "option", "duplicate_value", "unknown_value"),
    (
        (
            "source_entry",
            "--source-kind",
            MODULE.DEFAULT_REQUIRED_SOURCE_KINDS[0],
            "shadow-source-kind",
        ),
        (
            "publication",
            "--publication-route",
            MODULE.REQUIRED_PUBLICATION_ROUTES[0],
            "shadow-publication-route",
        ),
        (
            "publication",
            "--cycle-detail-probe",
            MODULE.REQUIRED_PUBLICATION_CYCLE_DETAIL_PROBES[0],
            "shadow-cycle-detail-probe",
        ),
        (
            "privacy_aggregate",
            "--privacy-action",
            MODULE.REQUIRED_PRIVACY_AGGREGATE_ACTIONS[0],
            "shadow-privacy-action",
        ),
        (
            "explorer",
            "--explorer-route",
            MODULE.REQUIRED_EXPLORER_ROUTES[0],
            "shadow-explorer-route",
        ),
    ),
)
def test_closed_set_inputs_reject_duplicate_and_unknown_values_before_write(
    kind: str,
    option: str,
    duplicate_value: str,
    unknown_value: str,
    tmp_path: Path,
    capsys,
) -> None:
    duplicate_args = args_for(kind, tmp_path)
    duplicate_args.extend([option, duplicate_value])
    assert_rejected_without_artifact(
        duplicate_args,
        kind=kind,
        tmp_path=tmp_path,
        capsys=capsys,
        expected_error=f"{option} must not contain duplicates",
    )

    unknown_dir = tmp_path / "unknown"
    unknown_dir.mkdir()
    unknown_args = args_for(kind, unknown_dir)
    unknown_args.extend([option, unknown_value])
    assert_rejected_without_artifact(
        unknown_args,
        kind=kind,
        tmp_path=unknown_dir,
        capsys=capsys,
        expected_error=f"{option} contains an unknown value",
    )


def test_cycle_detail_probe_count_must_match_inventory(
    tmp_path: Path,
    capsys,
) -> None:
    args = args_for("publication", tmp_path)
    args.extend(["--cycle-detail-probe-count", "2"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert (
        "--cycle-detail-probe-count must match --cycle-detail-probe inventory"
        in captured.err
    )
    assert not canary_path(tmp_path, "publication").exists()


def test_non_2xx_statuses_fail_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("publication", tmp_path)
    args.extend(["--route-status-code", "503", "--probe-status-code", "500"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--route-status-code must be a 2xx HTTP status code" in captured.err
    assert "--probe-status-code must be a 2xx HTTP status code" in captured.err
    assert not canary_path(tmp_path, "publication").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("explorer", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()


def test_output_directory_is_refused(tmp_path: Path, capsys) -> None:
    directory = tmp_path / "out-dir"
    directory.mkdir()
    args = args_for("explorer", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(directory)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a directory" in captured.err
