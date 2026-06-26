"""Tests for scripts/run_sorafs_reference_sdk_release_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_reference_sdk_release_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_reference_sdk_release_evidence",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    payload_dir.mkdir()
    return [
        "--out-dir",
        str(tmp_path / "evidence"),
        "--now-unix",
        "1800700000",
        "--max-evidence-age-secs",
        "1209600",
        "--min-release-targets",
        "4",
        "--min-downstream-packages",
        "5",
        "--max-smoke-duration-secs",
        "1800",
        "--release-archive-evidence",
        str(write_payload(payload_dir / "release-archive.json")),
        "--signed-manifest-evidence",
        str(write_payload(payload_dir / "signed-manifest.json")),
        "--downstream-bindings-evidence",
        str(write_payload(payload_dir / "downstream-bindings.json")),
        "--cookbook-smoke-evidence",
        str(write_payload(payload_dir / "cookbook-smoke.json")),
        "--ffi-header-contract-evidence",
        str(write_payload(payload_dir / "ffi-header-contract.json")),
        "--governance-approval-evidence",
        str(write_payload(payload_dir / "governance-approval.json")),
    ]


def write_args_file(path: Path, args: list[str]) -> Path:
    lines = [
        "# comments and blank lines are ignored",
        "",
    ]
    for index in range(0, len(args), 2):
        option = args[index]
        value = args[index + 1]
        lines.append(f"{option} {json.dumps(value)}")
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def write_split_args_file(path: Path, args: list[str]) -> Path:
    path.write_text(
        "\n".join(["# one token per line also works for long reviewed inputs", *args]),
        encoding="utf-8",
    )
    return path


def test_dry_run_prints_complete_reference_sdk_release_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reference_sdk.release_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.reference_sdk.release_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 1209600,
        "max_smoke_duration_secs": 1800,
        "min_downstream_packages": 5,
        "min_release_targets": 4,
        "now_unix": 1800700000,
    }
    assert plan["external_evidence"]["release_archive"] == [
        str(tmp_path / "payloads" / "release-archive.json")
    ]
    assert [step["label"] for step in plan["steps"]] == ["release_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_reference_sdk_release_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "cookbook-smoke.json") in verifier
    assert verifier.count("--release-archive-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-smoke-duration-secs" in verifier
    assert "--now-unix" in verifier


def test_response_file_dry_run_prints_complete_reference_sdk_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "reference-sdk-release.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "release_evidence_gate"
    assert plan["external_evidence"]["release_archive"]


def test_split_response_file_dry_run_prints_complete_reference_sdk_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-reference-sdk-release.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reference_sdk.release_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "release_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--release-archive-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --release-archive-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-cookbook-smoke.json"
    evidence_index = args.index("--cookbook-smoke-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--cookbook-smoke-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "release-archive.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "release_archive",
            "--release-archive-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["release_archive"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "release_archive" in verifier


def test_unknown_required_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence"),
                "--require-kind",
                "unknown",
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind `unknown`" in captured.err
    assert captured.out == ""
