"""Build Reserved-lineage production proof evidence JSON for Kagemusha."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
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
import kagemusha_production_readiness as readiness  # noqa: E402


DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND = (
    readiness.expected_lineage_proof_command(
        readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    )
)


def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:
    expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(
        path,
        label,
    )
    if file_errors:
        return None, file_errors
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} could not be read"]
    return digest.hexdigest(), []


def _sha256_file_with_size(
    path: Path,
    label: str,
) -> tuple[str | None, int | None, bytes | None, list[str]]:
    expected_stat, file_errors = readiness._validate_lineage_local_file_for_read(
        path,
        label,
    )
    if file_errors:
        return None, None, None, file_errors
    digest = hashlib.sha256()
    prefix_parts: list[bytes] = []
    prefix_remaining = 4096
    size = 0
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, None, None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, None, None, [f"{label} must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, None, None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if prefix_remaining > 0:
                    prefix_parts.append(chunk[:prefix_remaining])
                    prefix_remaining -= min(prefix_remaining, len(chunk))
                digest.update(chunk)
            final_path_stat = path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, None, None, [f"{label} changed while being read"]
    except OSError:
        return None, None, None, [f"{label} could not be read"]
    if size <= 0:
        return None, None, None, [f"{label} must be non-empty"]
    return digest.hexdigest(), size, b"".join(prefix_parts), []


def _secret_path_error(path: str | None, label: str) -> str | None:
    if path is not None and device_lab.SECRET_RE.search(path):
        return f"{label} must not contain secret-looking material"
    return None


def _validate_command(command: str) -> list[str]:
    return readiness.validate_lineage_proof_command(
        command, readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    )


def _validate_elapsed_seconds(value: float) -> list[str]:
    if not math.isfinite(value) or value <= 0:
        return ["--elapsed-seconds must be a positive finite number"]
    return []


def _validate_generated_at_utc(value: str) -> list[str]:
    if device_lab.SIGNED_AT_UTC_RE.fullmatch(value) is None:
        return ["--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ"]
    return []


def _validate_proof_log(path: Path) -> tuple[str | None, list[str]]:
    return readiness.validate_lineage_proof_log(
        path, readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"]
    )


def build_evidence(
    *,
    artifact_dir: Path,
    proof_log: Path,
    command: str,
    elapsed_seconds: float,
    generated_at_utc: str,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Build a Reserved-lineage proof evidence document from local artifacts."""

    errors = validate_lineage_input_paths(artifact_dir, proof_log)
    if errors:
        return None, errors

    errors.extend(_validate_generated_at_utc(generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        errors.append(timestamp_error["message"])
    errors.extend(_validate_command(command))
    errors.extend(_validate_elapsed_seconds(elapsed_seconds))

    artifact_digests: dict[str, str] = {}
    artifact_sizes: dict[str, int] = {}
    for artifact in readiness.LINEAGE_PROOF_REQUIRED_ARTIFACTS:
        path = artifact_dir / artifact
        digest, artifact_size, artifact_prefix, file_errors = _sha256_file_with_size(
            path,
            f"lineage artifact {artifact}",
        )
        if file_errors:
            if file_errors == [f"lineage artifact {artifact} is missing"]:
                errors.append(f"missing lineage artifact {artifact}")
            else:
                errors.extend(file_errors)
            continue
        assert (
            digest is not None
            and artifact_size is not None
            and artifact_prefix is not None
        )
        content_errors = readiness.validate_lineage_artifact_prefix(artifact_prefix, artifact)
        if content_errors:
            errors.extend(content_errors)
            continue
        artifact_digests[artifact] = digest
        artifact_sizes[artifact] = artifact_size

    proof_log_digest, proof_log_errors = _validate_proof_log(proof_log)
    errors.extend(proof_log_errors)

    if errors:
        return None, errors

    assert generated_at is not None
    assert proof_log_digest is not None
    return (
        {
            "schema": readiness.LINEAGE_PROOF_EVIDENCE_SCHEMA,
            "generated_at_utc": generated_at.isoformat().replace("+00:00", "Z"),
            "opening_len": readiness.EXPECTED_LINEAGE_PROOF_OPENING_LEN,
            "ipa_k": readiness.EXPECTED_LINEAGE_PROOF_IPA_K,
            "verifier_backend": readiness.EXPECTED_LINEAGE_PROOF_BACKEND,
            "verifier_witness_profile": readiness.EXPECTED_LINEAGE_VERIFIER_WITNESS_PROFILE,
            "record_archive_proof_runtime_keygen_env": "unset",
            "circuit_ids": dict(readiness.EXPECTED_LINEAGE_CIRCUIT_IDS),
            "artifacts": artifact_digests,
            "artifact_size_bytes": artifact_sizes,
            "tests": {
                "record_archive_proof": {
                    "name": readiness.LINEAGE_PROOF_REQUIRED_TESTS["record_archive_proof"],
                    "status": "passed",
                    "ignored": True,
                    "command": command,
                    "elapsed_seconds": elapsed_seconds,
                    "log_path": readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
                        "record_archive_proof"
                    ],
                    "log_sha256": proof_log_digest,
                }
            }
        },
        [],
    )


def validate_evidence_document(evidence: dict[str, Any], artifact_dir: Path) -> list[str]:
    """Return readiness-validator blocker messages for a generated evidence document."""

    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")
    if secret_error is not None:
        return [secret_error]
    pre_create_dir_errors = validate_artifact_dir_path(artifact_dir)
    if pre_create_dir_errors:
        return pre_create_dir_errors
    try:
        artifact_dir.mkdir(parents=True, exist_ok=True)
    except OSError:
        return ["--artifact-dir could not be created for evidence validation"]
    post_create_dir_errors = validate_artifact_dir_path(artifact_dir)
    if post_create_dir_errors:
        return post_create_dir_errors
    path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            encoding="utf-8",
            dir=artifact_dir,
            prefix=".lineage-proof-evidence-",
            suffix=".json",
            delete=False,
        ) as handle:
            path = Path(handle.name)
            handle.write(json.dumps(evidence, indent=2, sort_keys=True) + "\n")
    except OSError:
        if path is not None:
            try:
                path.unlink(missing_ok=True)
            except OSError:
                pass
        return ["lineage proof evidence validation file could not be written"]
    result = readiness.check_lineage_proof_evidence(
        path, require_canonical_filename=False
    )
    try:
        path.unlink(missing_ok=True)
    except OSError:
        return ["lineage proof evidence validation file could not be removed"]
    return [item["message"] for item in result["blockers"]]


def validate_artifact_dir_path(artifact_dir: Path) -> list[str]:
    """Reject artifact directories that could alias external release bytes."""

    secret_error = _secret_path_error(str(artifact_dir), "--artifact-dir")
    if secret_error is not None:
        return [secret_error]
    try:
        artifact_dir_mode = artifact_dir.lstat().st_mode
    except FileNotFoundError:
        artifact_dir_mode = None
    except OSError:
        return ["--artifact-dir metadata could not be read"]
    if artifact_dir_mode is not None and stat.S_ISLNK(artifact_dir_mode):
        return ["--artifact-dir must not be a symlink"]
    ancestor_errors = device_lab.validate_no_symlink_ancestors(
        artifact_dir,
        "--artifact-dir ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if artifact_dir_mode is None:
        return []
    if not stat.S_ISDIR(artifact_dir_mode):
        return ["--artifact-dir must be a directory"]
    return []


def _resolve_corridor_path(path: Path, label: str) -> tuple[Path | None, list[str]]:
    try:
        return path.resolve(), []
    except OSError:
        return None, [f"{label} could not be resolved"]


def _same_resolved_parent(child: Path, parent: Path) -> tuple[bool | None, list[str]]:
    child_parent, child_errors = _resolve_corridor_path(child.parent, "--proof-log parent")
    if child_errors:
        return None, child_errors
    parent_resolved, parent_errors = _resolve_corridor_path(parent, "--artifact-dir")
    if parent_errors:
        return None, parent_errors
    assert child_parent is not None
    assert parent_resolved is not None
    return child_parent == parent_resolved, []


def validate_output_corridor(out_path: Path, artifact_dir: Path) -> list[str]:
    """Validate that --out resolves directly under --artifact-dir."""

    output_parent, output_parent_errors = _resolve_corridor_path(
        out_path.parent,
        "--out parent",
    )
    if output_parent_errors:
        return output_parent_errors
    artifact_dir_resolved, artifact_dir_errors = _resolve_corridor_path(
        artifact_dir,
        "--artifact-dir",
    )
    if artifact_dir_errors:
        return artifact_dir_errors
    assert output_parent is not None
    assert artifact_dir_resolved is not None
    if output_parent != artifact_dir_resolved:
        return ["--out must be written directly under --artifact-dir"]
    return []


def validate_lineage_input_paths(artifact_dir: Path, proof_log: Path) -> list[str]:
    """Reject detached or aliased lineage proof inputs before reading bytes."""

    errors = validate_artifact_dir_path(artifact_dir)
    proof_log_secret_error = _secret_path_error(str(proof_log), "--proof-log")
    if proof_log_secret_error is not None:
        errors.append(proof_log_secret_error)
    if errors:
        return errors
    proof_log_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        proof_log,
        "--proof-log ancestor directory",
    )
    if proof_log_ancestor_errors:
        return proof_log_ancestor_errors
    expected_proof_log_name = readiness.LINEAGE_PROOF_REQUIRED_TEST_LOGS[
        "record_archive_proof"
    ]
    same_parent, corridor_errors = _same_resolved_parent(proof_log, artifact_dir)
    if corridor_errors:
        return corridor_errors
    if proof_log.name != expected_proof_log_name or not same_parent:
        return [
            "--proof-log must be written directly under --artifact-dir as "
            f"{expected_proof_log_name}"
        ]
    return []


def preflight_output_path(path: Path, label: str) -> list[str]:
    """Reject aliased output paths before evidence inputs are read."""

    secret_error = _secret_path_error(str(path), label)
    if secret_error is not None:
        return [secret_error]
    parent = path.parent
    parent_exists, parent_errors = _validate_output_parent(path, label)
    if parent_errors:
        return parent_errors
    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if output_ancestor_errors:
        return output_ancestor_errors
    if not parent_exists:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [f"{label} parent directory could not be created"]
    parent_exists, parent_errors = _validate_output_parent(
        path,
        label,
        missing_error=f"{label} parent must be a directory",
    )
    if parent_errors:
        return parent_errors
    if not parent_exists:
        return [f"{label} parent must be a directory"]
    output_ancestor_errors = device_lab.validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if output_ancestor_errors:
        return output_ancestor_errors
    try:
        output_mode = path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(output_mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISREG(output_mode):
        return [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    return []


def _validate_output_parent(
    path: Path,
    label: str,
    *,
    missing_error: str | None = None,
) -> tuple[bool, list[str]]:
    """Classify an output parent without following symlink aliases."""

    parent = path.parent
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        if missing_error is None:
            return False, []
        return False, [missing_error]
    except OSError:
        return False, [f"{label} parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_mode):
        return True, [f"{label} parent directory must not be a symlink"]
    if not stat.S_ISDIR(parent_mode):
        return True, [f"{label} parent must be a directory"]
    return True, []


def validate_output_path(path: Path, label: str) -> list[str]:
    """Reject output paths that could overwrite aliased local files."""

    secret_error = _secret_path_error(str(path), label)
    if secret_error is not None:
        return [secret_error]
    errors = preflight_output_path(path, label)
    if errors:
        return errors
    parent = path.parent
    parent_exists, parent_errors = _validate_output_parent(path, label)
    if parent_errors:
        return parent_errors
    if not parent_exists:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [f"{label} parent directory could not be created"]
    return preflight_output_path(path, label)


def _read_output_text(
    path: Path,
    expected_stat: os.stat_result,
    label: str,
) -> tuple[str | None, list[str]]:
    """Read helper output text without trusting a stale path."""

    chunks: list[bytes] = []
    output_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [f"{label} must be a regular file"]
            output_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if output_open_identity != output_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != output_expected_identity:
                return None, [f"{label} changed while being read"]
            if open_stat.st_nlink > 1:
                return None, [f"{label} must not be hardlinked"]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != output_expected_identity:
                return None, [f"{label} changed while being read"]
    except OSError:
        return None, [f"{label} write verification failed"]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, [f"{label} write verification failed"]


def write_evidence(path: Path, evidence: dict[str, Any]) -> list[str]:
    errors = validate_output_path(path, "--out")
    if errors:
        return errors
    try:
        evidence_text = json.dumps(
            evidence,
            indent=2,
            sort_keys=True,
            allow_nan=False,
        ) + "\n"
    except ValueError:
        return ["--out evidence is not strict JSON"]
    tmp_path: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            "w",
            dir=path.parent,
            encoding="utf-8",
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            tmp_path = Path(handle.name)
            handle.write(evidence_text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = validate_output_path(path, "--out")
        if errors:
            return errors
        os.replace(tmp_path, path)
        tmp_path = None
    except OSError:
        return ["--out could not be written"]
    finally:
        if tmp_path is not None:
            try:
                tmp_path.unlink(missing_ok=True)
            except OSError:
                pass
    try:
        parent_fd = os.open(path.parent, os.O_RDONLY)
    except OSError:
        parent_fd = None
    if parent_fd is not None:
        try:
            os.fsync(parent_fd)
        except OSError:
            pass
        finally:
            os.close(parent_fd)
    errors = validate_output_path(path, "--out")
    if errors:
        return errors
    try:
        expected_stat = path.lstat()
    except (FileNotFoundError, OSError):
        return ["--out write verification failed"]
    readback_text, readback_errors = _read_output_text(path, expected_stat, "--out")
    if readback_errors:
        return readback_errors
    if readback_text != evidence_text:
        return ["--out write verification failed"]
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build Kagemusha Reserved-lineage production proof evidence JSON."
    )
    parser.add_argument(
        "--artifact-dir",
        default="artifacts/kagemusha",
        help="Directory containing lineage-init/lineage-append key packages and records.",
    )
    parser.add_argument(
        "--proof-log",
        required=True,
        help="Captured stdout/stderr log from the production ignored record-archive proof run.",
    )
    parser.add_argument(
        "--command",
        default=DEFAULT_RECORD_ARCHIVE_PROOF_COMMAND,
        help="Exact command used to run the production ignored record-archive proof test.",
    )
    parser.add_argument(
        "--elapsed-seconds",
        required=True,
        type=float,
        help="Wall-clock seconds consumed by the production proof run.",
    )
    parser.add_argument(
        "--generated-at-utc",
        default=readiness.utc_now(),
        help="Canonical ISO-8601 UTC timestamp for the evidence document.",
    )
    parser.add_argument(
        "--out",
        default=readiness.DEFAULT_LINEAGE_PROOF_EVIDENCE_PATH,
        help="Output evidence JSON path.",
    )
    args = parser.parse_args(argv)

    path_errors = [
        error
        for error in (
            _secret_path_error(args.artifact_dir, "--artifact-dir"),
            _secret_path_error(args.proof_log, "--proof-log"),
            _secret_path_error(args.out, "--out"),
        )
        if error is not None
    ]
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    artifact_dir = Path(args.artifact_dir)
    proof_log = Path(args.proof_log)
    out_path = Path(args.out)
    path_errors.extend(validate_lineage_input_paths(artifact_dir, proof_log))
    path_errors.extend(validate_output_corridor(out_path, artifact_dir))
    if out_path.name != readiness.LINEAGE_PROOF_EVIDENCE_FILENAME:
        path_errors.append(
            f"--out must be named {readiness.LINEAGE_PROOF_EVIDENCE_FILENAME}"
        )
    early_output_errors = preflight_output_path(out_path, "--out")
    path_errors.extend(early_output_errors)
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    evidence, errors = build_evidence(
        artifact_dir=artifact_dir,
        proof_log=proof_log,
        command=args.command,
        elapsed_seconds=args.elapsed_seconds,
        generated_at_utc=args.generated_at_utc,
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    assert evidence is not None
    validation_errors = validate_evidence_document(evidence, artifact_dir)
    if validation_errors:
        for error in validation_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1

    write_errors = write_evidence(out_path, evidence)
    if write_errors:
        for error in write_errors:
            print(f"[kagemusha-lineage-proof-evidence] error: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-lineage-proof-evidence] wrote evidence")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
