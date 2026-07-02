"""Tests for scripts/build_sorafs_reference_sdk_release_canary.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = SCRIPT_ROOT / "build_sorafs_reference_sdk_release_canary.py"
CHECKER_PATH = SCRIPT_ROOT / "check_sorafs_reference_sdk_release_evidence.py"

SPEC = importlib.util.spec_from_file_location(
    "build_sorafs_reference_sdk_release_canary",
    MODULE_PATH,
)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)

CHECKER_SPEC = importlib.util.spec_from_file_location(
    "check_sorafs_reference_sdk_release_evidence",
    CHECKER_PATH,
)
CHECKER = importlib.util.module_from_spec(CHECKER_SPEC)
assert CHECKER_SPEC and CHECKER_SPEC.loader  # pragma: no cover - defensive
sys.modules[CHECKER_SPEC.name] = CHECKER
CHECKER_SPEC.loader.exec_module(CHECKER)


NOW_UNIX = 1_800_700_000
GENERATED_AT = NOW_UNIX - 120
MANIFEST_DIGEST = "a" * 64
ARCHIVE_DIGEST = "b" * 64
PACKAGE_DIGEST = "c" * 64
SMOKE_DIGEST = "d" * 64
HEADER_DIGEST = "e" * 64
FFI_DIGEST = "f" * 64
POLICY_DIGEST = "1" * 64
PUBLIC_KEY_DIGEST = "2" * 64


def canary_path(tmp_path: Path, kind: str) -> Path:
    return tmp_path / f"{kind}.json"


def args_for(kind: str, tmp_path: Path) -> list[str]:
    args = [
        "--kind",
        kind,
        "--out",
        str(canary_path(tmp_path, kind)),
        "--deployment-id",
        "reference-sdk-release-20260701",
        "--environment",
        "production",
        "--generated-at-unix",
        str(GENERATED_AT),
        "--now-unix",
        str(NOW_UNIX),
    ]
    if kind in MODULE.RELEASE_MANIFEST_BOUND_KINDS:
        args.extend(["--release-manifest-digest-hex", MANIFEST_DIGEST])
    if kind == "release_archive":
        args.extend(["--archive-index-digest-hex", ARCHIVE_DIGEST])
        for target in MODULE.REQUIRED_RELEASE_TARGETS:
            args.extend(["--target", target])
    elif kind == "signed_manifest":
        args.extend(
            [
                "--manifest-digest-hex",
                MANIFEST_DIGEST,
                "--public-key-fingerprint-hex",
                PUBLIC_KEY_DIGEST,
                "--policy-digest-hex",
                POLICY_DIGEST,
            ]
        )
    elif kind == "downstream_bindings":
        args.extend(["--package-index-digest-hex", PACKAGE_DIGEST])
        for package in MODULE.REQUIRED_DOWNSTREAM_PACKAGES:
            args.extend(["--package", package])
    elif kind == "cookbook_smoke":
        args.extend(["--smoke-output-digest-hex", SMOKE_DIGEST])
    elif kind == "ffi_header_contract":
        args.extend(
            [
                "--header-digest-hex",
                HEADER_DIGEST,
                "--ffi-contract-digest-hex",
                FFI_DIGEST,
            ]
        )
    elif kind == "governance_approval":
        args.extend(["--policy-digest-hex", POLICY_DIGEST])
    return args


def test_builds_payload_free_release_archive_canary(tmp_path: Path) -> None:
    assert MODULE.main(args_for("release_archive", tmp_path)) == 0

    payload = json.loads(canary_path(tmp_path, "release_archive").read_text("utf-8"))

    assert payload["schema"] == "sorafs.reference_sdk.release_archive_canary.v1"
    assert payload["status"] == "passed"
    assert payload["release_manifest_digest_hex"] == MANIFEST_DIGEST
    assert payload["archive_index_digest_hex"] == ARCHIVE_DIGEST
    assert payload["raw_archives_included"] is False
    errors = MODULE.validate_generated_payload(
        payload,
        MODULE.parse_args(args_for("release_archive", tmp_path)),
    )
    assert errors == []


def test_generated_canaries_pass_full_reference_sdk_release_gate(
    tmp_path: Path,
) -> None:
    evidence_paths: list[Path] = []
    for kind in MODULE.CANARY_KINDS:
        assert MODULE.main(args_for(kind, tmp_path)) == 0
        evidence_paths.append(canary_path(tmp_path, kind))
    summary = tmp_path / "summary.json"

    command = ["--now-unix", str(NOW_UNIX)]
    for path in evidence_paths:
        command.extend(["--evidence", str(path)])
    command.extend(["--summary-out", str(summary)])

    assert CHECKER.main(command) == 0

    payload = json.loads(summary.read_text("utf-8"))
    assert payload["status"] == "ready"
    assert payload["valid_release_manifest_digests"] == [MANIFEST_DIGEST]
    assert payload["valid_release_manifest_reference_digests"] == [MANIFEST_DIGEST]
    assert payload["valid_policy_digests"] == [POLICY_DIGEST]
    for kind in MODULE.CANARY_KINDS:
        assert payload["required"][kind]["artifact_count"] == 1
        assert payload["required"][kind]["artifacts"][0]["valid"] is True


def test_response_file_can_build_signed_manifest_canary(tmp_path: Path) -> None:
    args_file = tmp_path / "signed-manifest.args"
    args_file.write_text(
        "\n".join(args_for("signed_manifest", tmp_path)),
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 0

    payload = json.loads(canary_path(tmp_path, "signed_manifest").read_text("utf-8"))
    assert payload["manifest_digest_hex"] == MANIFEST_DIGEST
    assert payload["policy_digest_hex"] == POLICY_DIGEST
    assert payload["private_key_absent"] is True
    assert payload["raw_manifest_included"] is False


def test_signed_manifest_requires_policy_digest_before_write(
    tmp_path: Path, capsys
) -> None:
    args = args_for("signed_manifest", tmp_path)
    index = args.index("--policy-digest-hex")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--policy-digest-hex is required for signed_manifest" in captured.err
    assert not canary_path(tmp_path, "signed_manifest").exists()


def test_missing_release_target_coverage_fails_closed(tmp_path: Path, capsys) -> None:
    args = args_for("release_archive", tmp_path)
    index = args.index("--target")
    del args[index : index + 2]

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--target must include every required value" in captured.err
    assert not canary_path(tmp_path, "release_archive").exists()


def test_smoke_duration_threshold_fails_before_write(tmp_path: Path, capsys) -> None:
    args = args_for("cookbook_smoke", tmp_path)
    args.extend(["--smoke-duration-seconds", "1801"])

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "--smoke-duration-seconds must be <=" in captured.err
    assert not canary_path(tmp_path, "cookbook_smoke").exists()


def test_output_symlink_is_refused(tmp_path: Path, capsys) -> None:
    target = tmp_path / "target.json"
    link = tmp_path / "link.json"
    link.symlink_to(target)
    args = args_for("governance_approval", tmp_path)
    index = args.index("--out")
    args[index + 1] = str(link)

    assert MODULE.main(args) == 2

    captured = capsys.readouterr()
    assert "must not be a symlink" in captured.err
    assert not target.exists()
