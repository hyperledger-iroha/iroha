"""Build 4-peer localnet lifecycle evidence JSON for Kagemusha."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

import check_android_device_lab_slot as device_lab  # noqa: E402
import kagemusha_lineage_proof_evidence as lineage_helper  # noqa: E402
import kagemusha_production_readiness as readiness  # noqa: E402


LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME = (
    "kagemusha-localnet-lifecycle-acceptance.json"
)
DEFAULT_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_PATH = (
    f"artifacts/kagemusha/{LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME}"
)
MAX_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_JSON_BYTES = (
    readiness.MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES
)


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _blocker_messages(blockers: list[dict[str, Any]]) -> list[str]:
    return [str(item.get("message", "Kagemusha localnet lifecycle evidence failed")) for item in blockers]


def _resolve_corridor_path(path: Path, label: str) -> tuple[Path | None, list[str]]:
    try:
        return path.resolve(), []
    except OSError:
        return None, [f"{label} could not be resolved"]


def _same_resolved_parent(child: Path, parent: Path) -> tuple[bool | None, list[str]]:
    child_parent, child_errors = _resolve_corridor_path(
        child.parent,
        "--acceptance-report parent",
    )
    if child_errors:
        return None, child_errors
    parent_resolved, parent_errors = _resolve_corridor_path(parent, "--artifact-dir")
    if parent_errors:
        return None, parent_errors
    assert child_parent is not None
    assert parent_resolved is not None
    return child_parent == parent_resolved, []


def validate_acceptance_report_path_shape(acceptance_report: Path) -> list[str]:
    """Reject unsafe acceptance-report path strings before filesystem metadata."""

    path_text = str(acceptance_report)
    report_secret_error = lineage_helper._secret_path_error(
        path_text,
        "--acceptance-report",
    )
    if report_secret_error is not None:
        return [report_secret_error]
    if device_lab._contains_control_character(path_text):
        return ["--acceptance-report must not contain control characters"]
    if "\\" in path_text:
        return ["--acceptance-report must not contain backslashes"]
    if ".." in acceptance_report.parts:
        return ["--acceptance-report must be canonical"]
    return []


def validate_localnet_input_paths(
    artifact_dir: Path,
    acceptance_report: Path,
) -> list[str]:
    """Reject detached or aliased localnet lifecycle acceptance inputs."""

    report_shape_errors = validate_acceptance_report_path_shape(acceptance_report)
    if report_shape_errors:
        return report_shape_errors
    if acceptance_report.name == readiness.LOCALNET_LIFECYCLE_EVIDENCE_FILENAME:
        return ["--acceptance-report must not use the release evidence filename"]
    if acceptance_report.name != LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME:
        return [
            "--acceptance-report must be named "
            f"{LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME}"
        ]
    errors = lineage_helper.validate_artifact_dir_path(artifact_dir)
    if errors:
        return errors
    report_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        acceptance_report,
        "--acceptance-report ancestor directory",
    )
    if report_ancestor_errors:
        return report_ancestor_errors
    same_parent, corridor_errors = _same_resolved_parent(acceptance_report, artifact_dir)
    if corridor_errors:
        return corridor_errors
    if not same_parent:
        return ["--acceptance-report must be written directly under --artifact-dir"]
    return []


def _load_acceptance_report(path: Path) -> tuple[dict[str, Any] | None, list[str]]:
    report, blockers = readiness._load_json_artifact(
        path,
        missing_code="localnet_lifecycle_acceptance_missing",
        invalid_code="localnet_lifecycle_acceptance_invalid_json",
        unreadable_code="localnet_lifecycle_acceptance_unreadable",
        shape_code="localnet_lifecycle_acceptance_file_shape",
        not_object_code="localnet_lifecycle_acceptance_not_object",
        label="Kagemusha localnet lifecycle acceptance report",
        max_bytes=MAX_LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_JSON_BYTES,
    )
    if blockers:
        return None, _blocker_messages(blockers)
    return report, []


def _validate_generated_at_utc(value: Any) -> list[str]:
    return lineage_helper._validate_generated_at_utc(value)


def _validate_generated_at_future_skew(
    generated_at: dt.datetime | None,
    max_future_skew_seconds: Any,
) -> list[str]:
    return lineage_helper._validate_generated_at_future_skew(
        generated_at,
        max_future_skew_seconds,
    )


def build_evidence(
    *,
    artifact_dir: Path,
    acceptance_report: Path,
    generated_at_utc: Any,
    max_generated_at_future_skew_seconds: Any = (
        readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS
    ),
) -> tuple[dict[str, Any] | None, list[str]]:
    """Build a localnet lifecycle evidence document from an acceptance report."""

    errors: list[str] = []
    generated_at_errors = _validate_generated_at_utc(generated_at_utc)
    errors.extend(generated_at_errors)
    generated_at: dt.datetime | None = None
    if not generated_at_errors:
        generated_at, timestamp_error = readiness.parse_utc_timestamp(
            generated_at_utc,
            "--generated-at-utc",
        )
        if timestamp_error is not None:
            errors.append(timestamp_error["message"])
    errors.extend(
        _validate_generated_at_future_skew(
            generated_at,
            max_generated_at_future_skew_seconds,
        )
    )
    if errors:
        return None, errors

    errors.extend(validate_localnet_input_paths(artifact_dir, acceptance_report))
    if errors:
        return None, errors

    acceptance, report_errors = _load_acceptance_report(acceptance_report)
    if report_errors:
        return None, report_errors
    assert acceptance is not None
    assert generated_at is not None

    return (
        {
            "schema": readiness.LOCALNET_LIFECYCLE_EVIDENCE_SCHEMA,
            "generated_at_utc": generated_at.isoformat().replace("+00:00", "Z"),
            "localnet_run_id": acceptance.get("run_id"),
            "chain_id": acceptance.get("chain_id"),
            "localnet_acceptance": acceptance,
        },
        [],
    )


def _cleanup_validation_temp_output(
    path: Path,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return ["localnet lifecycle evidence validation file could not be removed"]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return ["localnet lifecycle evidence validation file could not be removed"]
    try:
        try:
            validation_temp_stat = os.stat(
                path.name,
                dir_fd=parent_fd,
                follow_symlinks=False,
            )
        except FileNotFoundError:
            return []
        except OSError:
            return ["localnet lifecycle evidence validation file could not be removed"]
        if (
            not stat.S_ISREG(validation_temp_stat.st_mode)
            or _file_identity(validation_temp_stat) != expected_identity
        ):
            return ["localnet lifecycle evidence validation file changed before cleanup"]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return ["localnet lifecycle evidence validation file could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return [
                "localnet lifecycle evidence validation file cleanup could not be synced"
            ]
    finally:
        os.close(parent_fd)
    return []


def _validation_temp_identity(handle: Any, path: Path) -> tuple[int, int] | None:
    try:
        return _file_identity(os.fstat(handle.fileno()))
    except (AttributeError, OSError):
        pass
    try:
        path_stat = path.lstat()
    except OSError:
        return None
    if not stat.S_ISREG(path_stat.st_mode):
        return None
    return _file_identity(path_stat)


def validate_evidence_document(evidence: dict[str, Any], artifact_dir: Path) -> list[str]:
    """Return readiness-validator blocker messages for generated localnet evidence."""

    secret_error = lineage_helper._secret_path_error(str(artifact_dir), "--artifact-dir")
    if secret_error is not None:
        return [secret_error]
    pre_create_dir_errors = lineage_helper.validate_artifact_dir_path(artifact_dir)
    if pre_create_dir_errors:
        return pre_create_dir_errors
    try:
        evidence_text = json.dumps(
            evidence,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return ["localnet lifecycle evidence validation file is not strict JSON"]
    if (
        len(evidence_text.encode("utf-8"))
        > readiness.MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES
    ):
        return [
            "localnet lifecycle evidence validation file must be no more than "
            f"{readiness.MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES} bytes"
        ]
    try:
        artifact_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    except OSError:
        return ["--artifact-dir could not be created for evidence validation"]
    post_create_dir_errors = lineage_helper.validate_artifact_dir_path(artifact_dir)
    if post_create_dir_errors:
        return post_create_dir_errors
    permission_errors = lineage_helper._set_private_directory_permissions(
        artifact_dir,
        "--artifact-dir",
    )
    if permission_errors:
        return permission_errors
    path: Path | None = None
    tmp_identity: tuple[int, int] | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            dir=artifact_dir,
            prefix=".localnet-lifecycle-evidence-",
            suffix=".json",
            delete=False,
        ) as handle:
            path = Path(handle.name)
            tmp_identity = _validation_temp_identity(handle, path)
            os.fchmod(handle.fileno(), 0o600)
            handle.write(evidence_text)
            handle.flush()
            os.fsync(handle.fileno())
    except (AttributeError, OSError):
        errors = ["localnet lifecycle evidence validation file could not be written"]
        if path is not None:
            errors.extend(_cleanup_validation_temp_output(path, tmp_identity))
        return errors
    result = readiness.check_localnet_lifecycle_evidence(
        path,
        require_canonical_filename=False,
    )
    cleanup_errors = _cleanup_validation_temp_output(path, tmp_identity)
    if cleanup_errors:
        return cleanup_errors
    return _blocker_messages(result["blockers"])


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build Kagemusha 4-peer localnet lifecycle evidence JSON."
    )
    parser.add_argument(
        "--artifact-dir",
        default="artifacts/kagemusha",
        help="Directory containing the production localnet lifecycle acceptance report.",
    )
    parser.add_argument(
        "--acceptance-report",
        default=None,
        help=(
            "Acceptance JSON containing the localnet_acceptance fields, written "
            "directly under --artifact-dir. Defaults to "
            f"{LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME} under --artifact-dir."
        ),
    )
    parser.add_argument(
        "--generated-at-utc",
        default=readiness.utc_now(),
        help="Canonical ISO-8601 UTC timestamp for the evidence document.",
    )
    parser.add_argument(
        "--max-generated-at-future-skew-seconds",
        type=int,
        default=readiness.DEFAULT_MAX_SIGNED_AT_FUTURE_SKEW_SECONDS,
        help=(
            "Maximum number of seconds generated_at_utc may be ahead of the "
            "helper clock."
        ),
    )
    parser.add_argument(
        "--out",
        default=readiness.DEFAULT_LOCALNET_LIFECYCLE_EVIDENCE_PATH,
        help="Output localnet lifecycle evidence JSON path.",
    )
    args = parser.parse_args(argv)

    path_errors = [
        error
        for error in (
            lineage_helper._secret_path_error(args.artifact_dir, "--artifact-dir"),
            lineage_helper._secret_path_error(
                args.acceptance_report,
                "--acceptance-report",
            ),
            lineage_helper._secret_path_error(args.out, "--out"),
        )
        if error is not None
    ]
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    artifact_dir = Path(args.artifact_dir)
    acceptance_report = (
        Path(args.acceptance_report)
        if args.acceptance_report
        else artifact_dir / LOCALNET_LIFECYCLE_ACCEPTANCE_REPORT_FILENAME
    )
    out_path = Path(args.out)

    if out_path.name != readiness.LOCALNET_LIFECYCLE_EVIDENCE_FILENAME:
        path_errors.append(
            f"--out must be named {readiness.LOCALNET_LIFECYCLE_EVIDENCE_FILENAME}"
        )
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    scalar_errors: list[str] = []
    scalar_errors.extend(_validate_generated_at_utc(args.generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        args.generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        scalar_errors.append(timestamp_error["message"])
    scalar_errors.extend(
        _validate_generated_at_future_skew(
            generated_at,
            args.max_generated_at_future_skew_seconds,
        )
    )
    if scalar_errors:
        for error in scalar_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    path_errors.extend(lineage_helper.validate_output_corridor(out_path, artifact_dir))
    early_output_errors = lineage_helper.preflight_output_path(out_path, "--out")
    path_errors.extend(early_output_errors)
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    path_errors.extend(validate_localnet_input_paths(artifact_dir, acceptance_report))
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    evidence, errors = build_evidence(
        artifact_dir=artifact_dir,
        acceptance_report=acceptance_report,
        generated_at_utc=args.generated_at_utc,
        max_generated_at_future_skew_seconds=args.max_generated_at_future_skew_seconds,
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    assert evidence is not None
    validation_errors = validate_evidence_document(evidence, artifact_dir)
    if validation_errors:
        for error in validation_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1

    write_errors = lineage_helper.write_evidence(
        out_path,
        evidence,
        max_bytes=readiness.MAX_LOCALNET_LIFECYCLE_EVIDENCE_JSON_BYTES,
    )
    if write_errors:
        for error in write_errors:
            print(f"[kagemusha-localnet-lifecycle-evidence] error: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-localnet-lifecycle-evidence] wrote evidence")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
