"""Build ABI-7 recursive compact key-artifact release evidence JSON."""

from __future__ import annotations

import argparse
import hashlib
import json
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


DEFAULT_COMPACT_KEY_COMMAND = readiness.expected_compact_key_command()
PLACEHOLDER_ARTIFACT_MESSAGE_FRAGMENT = "not a placeholder fixture"


def _secret_path_error(path: str | None, label: str) -> str | None:
    if path is not None and device_lab.SECRET_RE.search(path):
        return f"{label} must not contain secret-looking material"
    return None


def _sha256_file(path: Path, label: str) -> tuple[str | None, list[str]]:
    file_errors = readiness.validate_lineage_local_file(path, label)
    if file_errors:
        return None, file_errors
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError:
        return None, [f"{label} could not be read"]
    return digest.hexdigest(), []


def _validate_generated_at_utc(value: str) -> list[str]:
    if device_lab.SIGNED_AT_UTC_RE.fullmatch(value) is None:
        return ["--generated-at-utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ"]
    return []


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


def build_evidence(
    *,
    artifact_dir: Path,
    command: str,
    generated_at_utc: str,
    generator_log_path: Path | None = None,
) -> tuple[dict[str, Any] | None, list[str]]:
    """Build an ABI-7 recursive compact key evidence document from local artifacts."""

    errors = validate_artifact_dir_path(artifact_dir)
    errors.extend(_validate_generated_at_utc(generated_at_utc))
    generated_at, timestamp_error = readiness.parse_utc_timestamp(
        generated_at_utc,
        "--generated-at-utc",
    )
    if timestamp_error is not None:
        errors.append(timestamp_error["message"])
    errors.extend(readiness.validate_compact_key_command(command))
    if generator_log_path is None:
        generator_log_path = artifact_dir / readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME
    if generator_log_path.name != readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME:
        errors.append(
            f"--generator-log must be named {readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME}"
        )
    try:
        generator_log_parent = generator_log_path.resolve().parent
        artifact_dir_resolved = artifact_dir.resolve()
    except OSError:
        errors.append("--generator-log parent could not be resolved")
    else:
        if generator_log_parent != artifact_dir_resolved:
            errors.append("--generator-log must live directly under --artifact-dir")

    artifact_digests: dict[str, str] = {}
    artifact_sizes: dict[str, int] = {}
    for artifact in readiness.COMPACT_KEY_REQUIRED_ARTIFACTS:
        path = artifact_dir / artifact
        digest, file_errors = _sha256_file(
            path,
            f"recursive compact key artifact {artifact}",
        )
        if file_errors:
            if file_errors == [f"recursive compact key artifact {artifact} is missing"]:
                errors.append(f"missing recursive compact key artifact {artifact}")
            else:
                errors.extend(file_errors)
            continue
        assert digest is not None
        try:
            artifact_size = path.stat().st_size
        except OSError:
            errors.append(f"recursive compact key artifact {artifact} size could not be read")
            continue
        if artifact_size <= 0:
            errors.append(f"recursive compact key artifact {artifact} must be non-empty")
            continue
        errors.extend(readiness.validate_compact_key_artifact_content(path, artifact))
        artifact_digests[artifact] = digest
        artifact_sizes[artifact] = artifact_size

    generator_log_digest: str | None = None
    if generator_log_path.name == readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME:
        generator_log_digest, generator_log_errors = _sha256_file(
            generator_log_path,
            "recursive compact key generator log",
        )
        if generator_log_errors:
            if generator_log_errors == ["recursive compact key generator log is missing"]:
                errors.append("missing recursive compact key generator log")
            else:
                errors.extend(generator_log_errors)
        else:
            try:
                if (
                    generator_log_path.stat().st_size
                    > readiness.MAX_COMPACT_KEY_GENERATOR_LOG_BYTES
                ):
                    errors.append(
                        "recursive compact key generator log exceeds maximum size"
                    )
            except OSError:
                errors.append("recursive compact key generator log metadata could not be read")
            try:
                generator_log_text = generator_log_path.read_text(encoding="utf-8")
            except (OSError, UnicodeDecodeError):
                errors.append("recursive compact key generator log could not be read")
            else:
                generator_sizes, generator_parse_errors = (
                    readiness.parse_compact_key_generator_log(generator_log_text)
                )
                errors.extend(generator_parse_errors)
                for artifact, local_size in artifact_sizes.items():
                    logged_size = generator_sizes.get(artifact)
                    if logged_size is not None and logged_size != local_size:
                        errors.append(
                            f"recursive compact key generator log size does not match local artifact {artifact}"
                        )

    if errors:
        return None, errors

    assert generated_at is not None
    assert generator_log_digest is not None
    return (
        {
            "schema": readiness.COMPACT_KEY_EVIDENCE_SCHEMA,
            "generated_at_utc": generated_at.isoformat().replace("+00:00", "Z"),
            "opening_len": readiness.EXPECTED_COMPACT_KEY_OPENING_LEN,
            "ipa_k": readiness.EXPECTED_COMPACT_KEY_IPA_K,
            "verifier_backend": readiness.EXPECTED_COMPACT_KEY_BACKEND,
            "circuit_id": readiness.EXPECTED_COMPACT_KEY_CIRCUIT_ID,
            "record_namespace": readiness.EXPECTED_COMPACT_KEY_RECORD_NAMESPACE,
            "record_version": readiness.EXPECTED_COMPACT_KEY_RECORD_VERSION,
            "command": command,
            "generator_log_path": readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME,
            "generator_log_sha256": generator_log_digest,
            "artifacts": artifact_digests,
            "artifact_size_bytes": artifact_sizes,
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
            prefix=".recursive-compact-key-evidence-",
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
        return ["recursive compact key evidence validation file could not be written"]
    result = readiness.check_compact_key_evidence(
        path,
        require_canonical_filename=False,
    )
    try:
        path.unlink(missing_ok=True)
    except OSError:
        return ["recursive compact key evidence validation file could not be removed"]
    return [item["message"] for item in result["blockers"]]


def _resolve_corridor_path(path: Path, label: str) -> tuple[Path | None, list[str]]:
    try:
        return path.resolve(), []
    except OSError:
        return None, [f"{label} could not be resolved"]


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


def write_evidence(path: Path, evidence: dict[str, Any]) -> list[str]:
    errors = validate_output_path(path, "--out")
    if errors:
        return errors
    try:
        path.write_text(
            json.dumps(evidence, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except OSError:
        return ["--out could not be written"]
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Build ABI-7 recursive compact key-artifact evidence JSON."
    )
    parser.add_argument(
        "--artifact-dir",
        default="artifacts/kagemusha",
        help="Directory containing ABI-7 recursive compact key artifacts.",
    )
    parser.add_argument(
        "--command",
        default=DEFAULT_COMPACT_KEY_COMMAND,
        help="Exact command used to generate ABI-7 recursive compact key artifacts.",
    )
    parser.add_argument(
        "--generator-log",
        default=None,
        help=(
            "Path to the captured recursive-compact-key-artifacts stdout log. "
            f"Defaults to {readiness.COMPACT_KEY_GENERATOR_LOG_FILENAME} under --artifact-dir."
        ),
    )
    parser.add_argument(
        "--generated-at-utc",
        default=readiness.utc_now(),
        help="Canonical ISO-8601 UTC timestamp for the evidence document.",
    )
    parser.add_argument(
        "--out",
        default=readiness.DEFAULT_COMPACT_KEY_EVIDENCE_PATH,
        help="Output evidence JSON path.",
    )
    args = parser.parse_args(argv)

    path_errors = [
        error
        for error in (
            _secret_path_error(args.artifact_dir, "--artifact-dir"),
            _secret_path_error(args.generator_log, "--generator-log"),
            _secret_path_error(args.out, "--out"),
        )
        if error is not None
    ]
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-compact-key-evidence] error: {error}", file=sys.stderr)
        return 1

    artifact_dir = Path(args.artifact_dir)
    out_path = Path(args.out)
    path_errors.extend(validate_artifact_dir_path(artifact_dir))
    path_errors.extend(validate_output_corridor(out_path, artifact_dir))
    if out_path.name != readiness.COMPACT_KEY_EVIDENCE_FILENAME:
        path_errors.append(
            f"--out must be named {readiness.COMPACT_KEY_EVIDENCE_FILENAME}"
        )
    path_errors.extend(preflight_output_path(out_path, "--out"))
    if path_errors:
        for error in path_errors:
            print(f"[kagemusha-compact-key-evidence] error: {error}", file=sys.stderr)
        return 1

    evidence, errors = build_evidence(
        artifact_dir=artifact_dir,
        command=args.command,
        generated_at_utc=args.generated_at_utc,
        generator_log_path=Path(args.generator_log) if args.generator_log else None,
    )
    if errors:
        for error in errors:
            print(f"[kagemusha-compact-key-evidence] error: {error}", file=sys.stderr)
        return 1

    assert evidence is not None
    validation_errors = validate_evidence_document(evidence, artifact_dir)
    if validation_errors:
        for error in validation_errors:
            print(f"[kagemusha-compact-key-evidence] error: {error}", file=sys.stderr)
        return 1

    write_errors = write_evidence(out_path, evidence)
    if write_errors:
        for error in write_errors:
            print(f"[kagemusha-compact-key-evidence] error: {error}", file=sys.stderr)
        return 1
    print("[kagemusha-compact-key-evidence] wrote evidence")
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
