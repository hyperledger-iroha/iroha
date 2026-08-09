#!/usr/bin/env python3
"""Authenticate one Linux Taira authority, then extract its reviewed privacy inputs.

The command is intentionally narrow: it accepts the signed Linux/aarch64
rollout archive transferred between the release jobs, verifies its detached
authority with the checksum-pinned native verifier, and only then inspects the
tar stream.  It publishes a fresh owner-private directory containing the exact
four privacy bootstrap inputs plus a canonical extraction manifest.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tarfile
import tempfile
from typing import NoReturn, Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

try:
    from . import taira_rollout_admission as admission
    from .release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        create_fresh_directory,
        exclusive_output_fd,
        exclusive_write_bytes,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
    )
except ImportError:
    import taira_rollout_admission as admission
    from release_artifact_contract import (
        ReleaseArtifactError,
        StableFile,
        canonical_json_bytes,
        create_fresh_directory,
        exclusive_output_fd,
        exclusive_write_bytes,
        scan_inventory_paths,
        stable_hash_path,
        stable_hash_relative,
        stable_open_relative,
    )


SCHEMA = "iroha.taira.authenticated_privacy_release"
SCHEMA_VERSION = 1
OUTPUT_MANIFEST = "authenticated-privacy-release-v1.json"
ROLLOUT_MANIFEST_PATH = "rollout.manifest.json"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
MAX_ROLLOUT_MANIFEST_BYTES = 4 * 1024 * 1024
PRIVACY_INPUTS = {
    "privacy_bootstrap_plan.json": {
        "manifest_key": "plan",
        "rollout_path": "provenance/privacy-bootstrap/privacy_bootstrap_plan.json",
        "operator_copy": "configs/soranexus/taira/privacy_bootstrap_plan.json",
        "max_bytes": 16 * 1024 * 1024,
    },
    "config.toml": {
        "manifest_key": "peer_1_config",
        "rollout_path": "provenance/privacy-bootstrap/config.toml",
        "operator_copy": "configs/soranexus/taira/config.toml",
        "max_bytes": 4 * 1024 * 1024,
    },
    "genesis.json": {
        "manifest_key": "genesis",
        "rollout_path": "provenance/privacy-bootstrap/genesis.json",
        "operator_copy": "configs/soranexus/taira/genesis.json",
        "max_bytes": 64 * 1024 * 1024,
    },
    "bootle_lantern_broker_public.json": {
        "manifest_key": "broker_public_export",
        "rollout_path": (
            "provenance/privacy-bootstrap/bootle_lantern_broker_public.json"
        ),
        "operator_copy": None,
        "max_bytes": 4 * 1024 * 1024,
    },
}


class PrivacyReleaseExtractionError(RuntimeError):
    """The transferred release is unauthenticated, mutable, or malformed."""


def _fail(message: str) -> NoReturn:
    raise PrivacyReleaseExtractionError(message)


def _sha256(value: object, label: str) -> str:
    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        _fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def _commit(value: object) -> str:
    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        _fail("source commit must be one nonzero lowercase 40-hex object id")
    return value


def _canonical_path(path: Path, label: str, *, directory: bool = False) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    try:
        resolved = path.resolve(strict=True)
        info = path.lstat()
    except OSError as exc:
        raise PrivacyReleaseExtractionError(f"cannot inspect {label}: {exc}") from exc
    if resolved != path or stat.S_ISLNK(info.st_mode):
        _fail(f"{label} must be one canonical non-symlink path")
    expected = stat.S_ISDIR if directory else stat.S_ISREG
    if not expected(info.st_mode):
        _fail(f"{label} has the wrong file type")
    if not directory and info.st_nlink != 1:
        _fail(f"{label} must have exactly one hard link")
    if info.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        _fail(f"{label} must not be group- or world-writable")
    return path


def _require_private_output_parent(path: Path) -> None:
    parent = path.parent
    _canonical_path(parent, "privacy release output parent", directory=True)
    info = parent.lstat()
    if (
        info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
        or stat.S_IMODE(info.st_mode) & 0o077
    ):
        _fail("privacy release output parent must be owner-controlled and mode 0700")
    if path.exists() or path.is_symlink():
        _fail("privacy release output already exists")


def _copy_stable_file(
    source_root: Path,
    relative: str,
    destination: Path,
    info: StableFile,
) -> None:
    destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with stable_open_relative(source_root, relative, expected=info) as source:
        with exclusive_output_fd(destination, mode=0o600) as target:
            while chunk := os.read(source, 1024 * 1024):
                view = memoryview(chunk)
                while view:
                    written = os.write(target, view)
                    if written <= 0:
                        _fail("short write while staging Linux release authority")
                    view = view[written:]
            os.fsync(target)


def authenticate_linux_release(
    archive: Path,
    authority_dir: Path,
    *,
    source: admission.SourceIdentity,
    trusted_signing_fingerprint: str,
    verifier: Path,
    verifier_sha256: str,
    staging_parent: Path,
) -> tuple[StableFile, dict[str, object]]:
    """Verify the signed authority and exact-12 evidence without extracting inputs."""

    try:
        inventory = scan_inventory_paths(authority_dir)
    except ReleaseArtifactError as exc:
        raise PrivacyReleaseExtractionError(
            f"cannot scan transferred Linux authority: {exc}"
        ) from exc
    if inventory != sorted(admission.LINUX_AUTHORITY_FILES):
        _fail("transferred Linux authority inventory is not exact")
    archive_info = stable_hash_path(
        archive,
        max_size=admission.taira_release_authority.MAX_ARCHIVE_LOGICAL_BYTES,
    )

    with tempfile.TemporaryDirectory(
        prefix="taira-authenticated-privacy-authority-", dir=staging_parent
    ) as temporary:
        root = Path(temporary).resolve(strict=True)
        staged_authority = root / admission.LINUX_AUTHORITY_DIRECTORY
        for relative in inventory:
            info = stable_hash_relative(authority_dir, relative)
            _copy_stable_file(
                authority_dir,
                relative,
                staged_authority / relative,
                info,
            )
        manifest_sha256 = stable_hash_relative(
            staged_authority, "release_manifest.json"
        ).sha256
        verified = admission._verify_closed_linux_authority(
            root,
            expected_source=source,
            expected_manifest_sha256=manifest_sha256,
            expected_native_verifier_sha256=verifier_sha256,
            trusted_signing_fingerprint=trusted_signing_fingerprint,
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=verifier_sha256,
            linux_archive_path=archive,
        )
        # This second verifier checks the archive's native exact-12 evidence.
        # It runs only after the signed manifest has authenticated the archive
        # byte identity above.
        admission._verify_existing_linux_authority(
            root,
            linux_archive_path=archive,
            expected_source=source,
            trusted_signing_fingerprint=trusted_signing_fingerprint,
            native_verifier_sha256=verifier_sha256,
        )
        if stable_hash_path(archive) != archive_info:
            _fail("Linux rollout archive changed during authority verification")
        return archive_info, verified


def _read_tar_member(
    archive: tarfile.TarFile,
    member: tarfile.TarInfo,
    *,
    maximum_bytes: int,
) -> bytes:
    if member.size <= 0 or member.size > maximum_bytes:
        _fail(f"rollout archive member {member.name!r} violates its size bound")
    stream = archive.extractfile(member)
    if stream is None:
        _fail(f"cannot read rollout archive member {member.name!r}")
    payload = stream.read(member.size + 1)
    if len(payload) != member.size:
        _fail(f"rollout archive member {member.name!r} is truncated or oversized")
    return payload


def _strict_json_object(payload: bytes, label: str) -> dict[str, object]:
    def object_from_pairs(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                _fail(f"{label} repeats JSON field {key!r}")
            result[key] = value
        return result

    try:
        value = json.loads(payload.decode("utf-8"), object_pairs_hook=object_from_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise PrivacyReleaseExtractionError(f"{label} is not strict UTF-8 JSON") from exc
    if not isinstance(value, dict):
        _fail(f"{label} must be a JSON object")
    return value


def _validated_privacy_rows(
    rollout_payload: bytes,
    extracted: dict[str, bytes],
    *,
    source: admission.SourceIdentity,
) -> dict[str, dict[str, object]]:
    manifest = _strict_json_object(rollout_payload, "rollout manifest")
    if (
        manifest.get("git_head") != source.commit
        or manifest.get("dpn_validator_release_commit")
        != source.dpn_validator_release_commit
        or manifest.get("validator_lock_sha256") != source.cargo_lock_sha256
        or manifest.get("workspace_source_manifest_sha256")
        != source.workspace_source_manifest_sha256
        or manifest.get("cargo_profile") != "release"
    ):
        _fail("rollout manifest differs from the authenticated release source")
    release = manifest.get("privacy_bootstrap_release")
    if not isinstance(release, dict):
        _fail("rollout manifest lacks the privacy bootstrap release binding")
    if set(release) != {
        "schema",
        "native_recomposition_passed",
        "bundled_release_validation_passed",
        "secret_free",
        "plan",
        "peer_1_config",
        "genesis",
        "broker_public_export",
    }:
        _fail("rollout privacy release binding fields are not exact")
    if (
        release.get("schema")
        != "iroha.taira.privacy-bootstrap-release-bundle.v1"
        or release.get("native_recomposition_passed") is not True
        or release.get("bundled_release_validation_passed") is not True
        or release.get("secret_free") is not True
    ):
        _fail("rollout privacy release was not natively recomposed and validated")

    rows: dict[str, dict[str, object]] = {}
    for output_name, contract in PRIVACY_INPUTS.items():
        manifest_key = str(contract["manifest_key"])
        row = release.get(manifest_key)
        if not isinstance(row, dict):
            _fail(f"rollout privacy release lacks {manifest_key!r}")
        operator_copy = contract["operator_copy"]
        expected_fields = {"path", "sha256"}
        if manifest_key == "plan":
            expected_fields.add("operator_copy")
        elif manifest_key == "peer_1_config":
            expected_fields |= {"operator_copy", "designated_validator"}
        elif manifest_key == "genesis":
            expected_fields.add("operator_copy")
        else:
            expected_fields.add("bound_by_plan_sha256")
        if set(row) != expected_fields:
            _fail(f"rollout privacy release row {manifest_key!r} is not exact")
        expected_path = str(contract["rollout_path"])
        if row.get("path") != expected_path:
            _fail(f"rollout privacy release row {manifest_key!r} changed path")
        if operator_copy is not None and row.get("operator_copy") != operator_copy:
            _fail(f"rollout privacy release row {manifest_key!r} changed operator copy")
        if manifest_key == "peer_1_config" and (
            row.get("designated_validator") != "taira-validator-1"
        ):
            _fail("rollout privacy issuer is not designated to validator 1")
        if manifest_key == "broker_public_export" and (
            row.get("bound_by_plan_sha256") is not True
        ):
            _fail("rollout broker export is not bound by the privacy plan")
        digest = _sha256(row.get("sha256"), f"{manifest_key} digest")
        payload = extracted[expected_path]
        actual_digest = hashlib.sha256(payload).hexdigest()
        if actual_digest != digest:
            _fail(f"rollout privacy release digest mismatch for {manifest_key!r}")
        if operator_copy is not None and extracted[str(operator_copy)] != payload:
            _fail(f"rollout operator copy differs for {manifest_key!r}")
        rows[output_name] = {
            "rollout_path": expected_path,
            "sha256": digest,
            "size": len(payload),
        }
    return rows


def extract_privacy_release(
    archive_path: Path,
    archive_info: StableFile,
    *,
    source: admission.SourceIdentity,
) -> tuple[dict[str, bytes], bytes, dict[str, dict[str, object]]]:
    """Inspect the authenticated tar stream and return its four reviewed inputs."""

    prefix = archive_path.name.removesuffix(".tar.gz")
    wanted_bounds = {ROLLOUT_MANIFEST_PATH: MAX_ROLLOUT_MANIFEST_BYTES}
    for contract in PRIVACY_INPUTS.values():
        wanted_bounds[str(contract["rollout_path"])] = int(contract["max_bytes"])
        if contract["operator_copy"] is not None:
            wanted_bounds[str(contract["operator_copy"])] = int(contract["max_bytes"])
    wanted_archive_names = {
        f"{prefix}/{relative}": relative for relative in wanted_bounds
    }
    seen: set[str] = set()
    extracted: dict[str, bytes] = {}
    member_count = 0
    logical_bytes = 0
    try:
        with stable_open_relative(
            archive_path.parent, archive_path.name, expected=archive_info
        ) as descriptor:
            with os.fdopen(os.dup(descriptor), "rb") as stream:
                with tarfile.open(fileobj=stream, mode="r:gz") as archive:
                    for member in archive:
                        member_count += 1
                        if member_count > admission.taira_release_authority.MAX_ARCHIVE_MEMBERS:
                            _fail("Linux rollout archive exceeds its member-count bound")
                        name = admission._safe_member_name(member, prefix)
                        if name in seen:
                            _fail(f"Linux rollout archive repeats member {name!r}")
                        seen.add(name)
                        if member.isdir():
                            if member.size != 0:
                                _fail("Linux rollout archive directory has nonzero size")
                            if name in wanted_archive_names:
                                _fail("required privacy release member is not a regular file")
                            continue
                        if not member.isfile() or member.issparse():
                            _fail(
                                "Linux rollout archive contains a link, sparse file, "
                                "device, FIFO, or socket"
                            )
                        logical_bytes += member.size
                        if (
                            logical_bytes
                            > admission.taira_release_authority.MAX_ARCHIVE_LOGICAL_BYTES
                        ):
                            _fail("Linux rollout archive exceeds its logical-size bound")
                        relative = wanted_archive_names.get(name)
                        if relative is not None:
                            extracted[relative] = _read_tar_member(
                                archive,
                                member,
                                maximum_bytes=wanted_bounds[relative],
                            )
    except (
        OSError,
        tarfile.TarError,
        ReleaseArtifactError,
        admission.TairaRolloutAdmissionError,
    ) as exc:
        raise PrivacyReleaseExtractionError(
            f"cannot safely inspect authenticated Linux rollout: {exc}"
        ) from exc
    missing = sorted(set(wanted_bounds) - set(extracted))
    if missing:
        _fail(f"Linux rollout archive omits privacy release members: {missing}")
    rollout = extracted.pop(ROLLOUT_MANIFEST_PATH)
    rows = _validated_privacy_rows(rollout, extracted, source=source)
    outputs = {
        output_name: extracted[str(contract["rollout_path"])]
        for output_name, contract in PRIVACY_INPUTS.items()
    }
    if stable_hash_path(archive_path) != archive_info:
        _fail("Linux rollout archive changed during privacy release extraction")
    return outputs, rollout, rows


def run(args: argparse.Namespace) -> dict[str, object]:
    archive = _canonical_path(args.archive, "Linux rollout archive")
    if not archive.name.endswith(".tar.gz"):
        _fail("Linux rollout archive must use the .tar.gz suffix")
    authority = _canonical_path(
        args.authority_dir, "Linux authority directory", directory=True
    )
    verifier = _canonical_path(args.release_manifest_verifier, "native verifier")
    output = args.output_dir
    if not output.is_absolute():
        _fail("privacy release output must be an absolute path")
    _require_private_output_parent(output)
    source = admission.SourceIdentity(
        _commit(args.source_commit),
        _commit(args.dpn_validator_release_commit),
        _sha256(args.cargo_lock_sha256, "Cargo.lock digest"),
        _sha256(args.workspace_source_manifest_sha256, "workspace source digest"),
    )
    fingerprint = _sha256(
        args.trusted_signing_fingerprint, "trusted signing fingerprint"
    )
    verifier_sha256 = _sha256(
        args.trusted_release_manifest_verifier_sha256,
        "trusted native verifier digest",
    )
    if stable_hash_path(verifier).sha256 != verifier_sha256:
        _fail("native verifier differs from its pinned digest")

    archive_info, authority_result = authenticate_linux_release(
        archive,
        authority,
        source=source,
        trusted_signing_fingerprint=fingerprint,
        verifier=verifier,
        verifier_sha256=verifier_sha256,
        staging_parent=output.parent,
    )
    payloads, rollout, rows = extract_privacy_release(
        archive, archive_info, source=source
    )
    if stable_hash_path(verifier).sha256 != verifier_sha256:
        _fail("native verifier changed after release authentication")

    create_fresh_directory(output, mode=0o700)
    try:
        for name, payload in payloads.items():
            exclusive_write_bytes(output / name, payload, mode=0o600)
        manifest = {
            "schema": SCHEMA,
            "schema_version": SCHEMA_VERSION,
            "source": source.as_dict(),
            "linux_archive": {
                "name": archive.name,
                "sha256": archive_info.sha256,
                "size": archive_info.size,
            },
            "authority": {
                "manifest_sha256": authority_result["manifest_sha256"],
                "native_verifier_sha256": verifier_sha256,
                "signer_fingerprint_sha256": fingerprint,
            },
            "rollout_manifest_sha256": hashlib.sha256(rollout).hexdigest(),
            "privacy_inputs": rows,
        }
        exclusive_write_bytes(
            output / OUTPUT_MANIFEST,
            canonical_json_bytes(manifest),
            mode=0o600,
        )
        if scan_inventory_paths(output) != sorted({OUTPUT_MANIFEST, *PRIVACY_INPUTS}):
            _fail("published privacy release snapshot inventory is not exact")
        return {
            "archive_sha256": archive_info.sha256,
            "output_dir": str(output),
            "privacy_inputs": rows,
            "rollout_manifest_sha256": manifest["rollout_manifest_sha256"],
        }
    except BaseException:
        shutil.rmtree(output)
        raise


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--authority-dir", type=Path, required=True)
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--dpn-validator-release-commit", required=True)
    parser.add_argument("--cargo-lock-sha256", required=True)
    parser.add_argument("--workspace-source-manifest-sha256", required=True)
    parser.add_argument("--trusted-signing-fingerprint", required=True)
    parser.add_argument("--release-manifest-verifier", type=Path, required=True)
    parser.add_argument(
        "--trusted-release-manifest-verifier-sha256", required=True
    )
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        result = run(args)
    except (
        OSError,
        ReleaseArtifactError,
        admission.TairaRolloutAdmissionError,
        PrivacyReleaseExtractionError,
    ) as exc:
        print(f"authenticated Taira privacy extraction refused: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
