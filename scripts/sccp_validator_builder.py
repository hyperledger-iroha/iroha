#!/usr/bin/env python3
"""Hermetic two-party builder for the final-V1 SCCP Rust validator.

Each release role runs ``prepare`` independently from the same clean signed Git
commit and digest-pinned Linux/amd64 image.  The command publishes an unsigned
role-bound rebuild attestation and its exact signing payload.  ``finalize``
accepts only two externally signed rebuilds whose complete source, dependency,
metadata, toolchain, sysroot, linker, recipe, environment, and executable
closures are byte-identical.  No signing key is accepted or generated here.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import secrets
import signal
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from collections.abc import Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

import sccp_release_common as common

ROOT = Path(__file__).resolve().parents[1]
ORCHESTRATOR = Path(__file__).resolve()
RELEASE_COMMON = Path(common.__file__).resolve()
DRIVER = ROOT / "scripts" / "sccp_validator_builder_driver.py"
POLICY_SCHEMA = "iroha.sccp.validator-builder-policy.final-v1"
DRIVER_REPORT_SCHEMA = "iroha.sccp.validator-build-report.final-v1"
REBUILD_SCHEMA = "iroha.sccp.validator-rebuild-attestation.final-v1"
LOCK_SCHEMA = "iroha.sccp.validator-output-lock.final-v1"
RECEIPT_SCHEMA = "iroha.sccp.validator-build-receipt.final-v1"
VERIFICATION_SCHEMA = "iroha.sccp.validator-build-verification.final-v1"
SBOM_SCHEMA = "iroha.sccp.rust-validator-sbom.final-v1"
REBUILD_SIGNING_DOMAIN = b"iroha:sccp:validator-rebuild:final-v1\x00"
OUTPUT_LOCK_HASH_DOMAIN = b"iroha:sccp:validator-output-lock:final-v1\x00"
PLATFORM = "linux/amd64"
TARGET = "x86_64-unknown-linux-gnu"
CRATE = "iroha_sccp"
BINARY = "sccp_release_evidence"
FEATURES = ("dev-tools",)
ROLES = ("release-engineering", "release-security")
DRIVER_MOUNT = "/opt/iroha/sccp_validator_builder_driver.py"
HOST_TEMP_ROOT = Path("/tmp")
HEX32_RE = re.compile(r"^[0-9a-f]{64}$")
COMMIT_RE = re.compile(r"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
IMAGE_RE = re.compile(r"^[a-z0-9][a-z0-9._:/-]{0,200}@sha256:[0-9a-f]{64}$")
ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9._:+-]{0,127})$")
SEGMENT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
CONTAINER_PATH_RE = re.compile(r"^/[A-Za-z0-9][A-Za-z0-9._+/-]{0,511}$")
SHELL_INERT_HOST_PATH_RE = re.compile(
    r"^/(?:[A-Za-z0-9][A-Za-z0-9._+-]*/)*[A-Za-z0-9][A-Za-z0-9._+-]*$"
)
FINGERPRINT_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9:+/=_-]{15,199}$")
MAX_POLICY_BYTES = 4 * 1024 * 1024
MAX_REPORT_BYTES = 128 * 1024 * 1024
MAX_SIGNED_REBUILD_BYTES = 4 * 1024 * 1024
MAX_LOCK_BYTES = 4 * 1024 * 1024
MAX_RECEIPT_BYTES = 4 * 1024 * 1024
MAX_SOURCE_ARCHIVE_BYTES = 4 * 1024 * 1024 * 1024
MAX_HOST_EXECUTABLE_BYTES = 512 * 1024 * 1024
MAX_CLOSURE_DOCUMENT_BYTES = 128 * 1024 * 1024
STREAM_SECRET_SCAN_OVERLAP_BYTES = 2 * 1024 * 1024
MAX_SOURCE_SECRET_SCAN_EXCEPTIONS = 512
MAX_VALIDATOR_BUILD_AGE_MS = 7 * 24 * 60 * 60 * 1000
MAX_FUTURE_SKEW_MS = 2 * 60 * 1000
SOURCE_TREE_INVENTORY = ".sccp-source-tree-inventory.json"
APPROVED_SOURCE_CARGO_CONFIG_SHA256 = (
    "99ccdf420c7fd6f7c4abcb1908a653b832791cb169d020448fb23cda85b40014"
)
EXPECTED_CLOSURE_FILES = (
    "build-environment.json",
    "build-recipe.json",
    "cargo-config.toml",
    "cargo-metadata-closure.json",
    "dependency-inventory.json",
    "sbom.json",
    "sysroot-inventory.json",
    "toolchain-inventory.json",
)
EXPECTATION_FIELDS = (
    "dependency_inventory_sha256",
    "cargo_metadata_closure_sha256",
    "sbom_sha256",
    "toolchain_inventory_sha256",
    "sysroot_inventory_sha256",
    "linker_sha256",
    "build_recipe_sha256",
    "build_environment_sha256",
)
RECEIPT_HASH_FIELDS = (
    "validator_builder_policy_sha256",
    "validator_source_archive_sha256",
    "validator_dependency_inventory_sha256",
    "validator_cargo_metadata_closure_sha256",
    "validator_sbom_sha256",
    "validator_toolchain_inventory_sha256",
    "validator_sysroot_inventory_sha256",
    "validator_linker_sha256",
    "validator_build_recipe_sha256",
    "validator_build_environment_sha256",
    "validator_container_manifest_sha256",
    "validator_builder_report_sha256",
    "validator_executable_sha256",
    "validator_complete_build_closure_sha256",
    "validator_output_lock_sha256",
)


class ValidatorBuilderError(RuntimeError):
    """A bounded diagnostic safe for release logs."""


class _SafeArgumentParser(argparse.ArgumentParser):
    def error(self, message: str) -> None:
        del message
        raise ValidatorBuilderError("command line has an invalid final-V1 shape")


def _fail(message: str) -> None:
    raise ValidatorBuilderError(message)


def _object(value: Any, *, label: str, keys: Sequence[str]) -> dict[str, Any]:
    if type(value) is not dict or set(value) != set(keys) or len(value) != len(keys):
        _fail(f"{label} must contain the exact final-V1 fields")
    return value


def _list(value: Any, *, label: str, length: int) -> list[Any]:
    if type(value) is not list or len(value) != length:
        _fail(f"{label} must contain exactly {length} entries")
    return value


def _string(value: Any, *, label: str, maximum: int = 256) -> str:
    if type(value) is not str or not value or len(value.encode("utf-8")) > maximum:
        _fail(f"{label} must be bounded nonempty text")
    if value != value.strip() or any(ord(character) < 0x20 for character in value):
        _fail(f"{label} must use canonical printable text")
    return value


def _version_text(value: Any, *, label: str) -> str:
    if type(value) is not str or not value or len(value.encode("utf-8")) > 8192:
        _fail(f"{label} must be bounded nonempty version text")
    if value != value.strip() or "\r" in value:
        _fail(f"{label} must use canonical LF-delimited version text")
    lines = value.split("\n")
    if any(
        not line
        or line != line.rstrip()
        or any(ord(character) < 0x20 for character in line)
        for line in lines
    ):
        _fail(f"{label} contains non-canonical version text")
    return value


def _hex32(value: Any, *, label: str) -> str:
    value = _string(value, label=label, maximum=64)
    if not HEX32_RE.fullmatch(value) or value == "00" * 32:
        _fail(f"{label} must be one nonzero lowercase SHA-256 value")
    return value


def _positive(value: Any, *, label: str, maximum: int) -> int:
    if type(value) is not int or not 1 <= value <= maximum:
        _fail(f"{label} is outside its final-V1 bound")
    return value


def _container_path(value: Any, *, label: str) -> str:
    value = _string(value, label=label, maximum=512)
    path = PurePosixPath(value)
    if (
        not path.is_absolute()
        or str(path) != value
        or not CONTAINER_PATH_RE.fullmatch(value)
        or any(part in ("", ".", "..") for part in path.parts[1:])
    ):
        _fail(f"{label} must be one canonical absolute container path")
    return value


def _valid_ed25519_public_key(encoded: bytes) -> bool:
    point = common._ed_decode(encoded)
    if point is None or point == common._ED_IDENTITY:
        return False
    return common._ed_extended_equal(
        common._ed_scalar_multiply_extended(common._ed_extended(point), common._ED_L),
        common._ED_EXTENDED_IDENTITY,
    )


def _load_json(path: Path, *, label: str, maximum: int) -> tuple[dict[str, Any], bytes]:
    data = common.read_direct_file(path, label=label, maximum=maximum)
    common.reject_secret_material(data, label=label)
    value = common.parse_json_bytes(data, label=label, maximum=maximum)
    common.require_canonical_json_file(data, value, label=label)
    if type(value) is not dict:
        _fail(f"{label} must be one JSON object")
    return value, data


def validate_policy(value: Any) -> dict[str, Any]:
    """Validate one externally approved immutable builder policy."""

    policy = _object(
        value,
        label="validator builder policy",
        keys=("schema", "source", "builder", "limits", "approvers"),
    )
    if policy["schema"] != POLICY_SCHEMA:
        _fail("validator builder policy has the wrong final-V1 schema")
    source = _object(
        policy["source"],
        label="validator builder source",
        keys=(
            "commit",
            "commit_signer_fingerprint",
            "source_date_epoch",
            "secret_scan_exceptions",
        ),
    )
    commit = _string(source["commit"], label="source.commit", maximum=64)
    if not COMMIT_RE.fullmatch(commit):
        _fail("source.commit must be one full lowercase Git object id")
    fingerprint = _string(
        source["commit_signer_fingerprint"],
        label="source.commit_signer_fingerprint",
        maximum=200,
    )
    if not FINGERPRINT_RE.fullmatch(fingerprint):
        _fail("source commit signer fingerprint is not canonical")
    source_date_epoch = _positive(
        source["source_date_epoch"],
        label="source.source_date_epoch",
        maximum=4_102_444_800,
    )
    exceptions = source["secret_scan_exceptions"]
    if (
        type(exceptions) is not list
        or len(exceptions) > MAX_SOURCE_SECRET_SCAN_EXCEPTIONS
    ):
        _fail("source secret-scan exceptions exceed their final-V1 bound")
    previous_exception: tuple[str, str] | None = None
    exception_paths: set[str] = set()
    validated_exceptions: list[dict[str, str]] = []
    for exception in exceptions:
        exception = _object(
            exception,
            label="source secret-scan exception",
            keys=("path", "object_id"),
        )
        path = exception["path"]
        object_id = exception["object_id"]
        if (
            type(path) is not str
            or not path
            or len(path.encode("utf-8")) > 8192
            or PurePosixPath(path).is_absolute()
            or str(PurePosixPath(path)) != path
            or any(part in ("", ".", "..") for part in PurePosixPath(path).parts)
            or any(ord(character) < 0x20 for character in path)
            or type(object_id) is not str
            or not re.fullmatch(
                r"[0-9a-f]{40}" if len(commit) == 40 else r"[0-9a-f]{64}",
                object_id,
            )
        ):
            _fail("source secret-scan exception is not canonical")
        _reject_tracked_path_material(path.encode("utf-8"))
        identity = (path, object_id)
        if (
            previous_exception is not None and identity <= previous_exception
        ) or path in exception_paths:
            _fail("source secret-scan exceptions are not uniquely sorted")
        previous_exception = identity
        exception_paths.add(path)
        validated_exceptions.append({"path": path, "object_id": object_id})

    builder = _object(
        policy["builder"],
        label="validator builder",
        keys=(
            "image",
            "platform",
            "driver_path",
            "driver_sha256",
            "python_path",
            "python_reported_version",
            "cargo_path",
            "cargo_reported_version",
            "rustc_path",
            "rustc_reported_version",
            "linker_path",
            "linker_reported_version",
            "cargo_home_path",
            "target_triple",
            "host_python_sha256",
            "host_orchestrator_sha256",
            "host_release_common_sha256",
            "host_git_sha256",
            "host_docker_sha256",
            "docker_daemon_report_sha256",
            "host_commit_verifier_sha256",
            "closure_expectations",
        ),
    )
    image = _string(builder["image"], label="builder.image", maximum=256)
    if not IMAGE_RE.fullmatch(image) or image.endswith("@sha256:" + "0" * 64):
        _fail("builder.image must be one nonzero digest-pinned OCI reference")
    if builder["platform"] != PLATFORM or builder["target_triple"] != TARGET:
        _fail("validator builder must target exact linux/amd64 x86_64 GNU")
    if builder["driver_path"] != DRIVER_MOUNT:
        _fail("validator builder driver path is not the fixed read-only mount")
    paths = {
        field: _container_path(builder[field], label=f"builder.{field}")
        for field in (
            "python_path",
            "cargo_path",
            "rustc_path",
            "linker_path",
            "cargo_home_path",
        )
    }
    image_paths = [PurePosixPath(value) for value in paths.values()]
    forbidden_image_roots = (PurePosixPath("/work"), PurePosixPath("/input"))
    if (
        len(set(paths.values())) != len(paths)
        or any(
            candidate == root or root in candidate.parents
            for candidate in image_paths
            for root in forbidden_image_roots
        )
        or any(
            left in right.parents or right in left.parents
            for index, left in enumerate(image_paths)
            for right in image_paths[index + 1 :]
        )
    ):
        _fail("validator builder executable and cache paths must be role-distinct")
    versions = {}
    for field in (
        "python_reported_version",
        "cargo_reported_version",
        "rustc_reported_version",
        "linker_reported_version",
    ):
        versions[field] = _version_text(builder[field], label=f"builder.{field}")
    if not versions["python_reported_version"].startswith("Python 3."):
        _fail("validator builder requires a pinned Python 3 runtime")
    if not versions["cargo_reported_version"].startswith("cargo 1."):
        _fail("validator builder requires an exact stable Cargo identity")
    if not versions["rustc_reported_version"].startswith("rustc 1."):
        _fail("validator builder requires an exact stable rustc identity")
    hashes = {
        field: _hex32(builder[field], label=f"builder.{field}")
        for field in (
            "driver_sha256",
            "host_python_sha256",
            "host_orchestrator_sha256",
            "host_release_common_sha256",
            "host_git_sha256",
            "host_docker_sha256",
            "docker_daemon_report_sha256",
            "host_commit_verifier_sha256",
        )
    }
    driver_bytes = common.read_direct_file(
        DRIVER, label="validator builder driver", maximum=2 * 1024 * 1024
    )
    if hashes["driver_sha256"] != hashlib.sha256(driver_bytes).hexdigest():
        _fail("validator builder driver differs from the reviewed repository source")
    for field, path, label in (
        ("host_orchestrator_sha256", ORCHESTRATOR, "validator builder orchestrator"),
        ("host_release_common_sha256", RELEASE_COMMON, "SCCP release common module"),
    ):
        payload = common.read_direct_file(path, label=label, maximum=16 * 1024 * 1024)
        if hashes[field] != hashlib.sha256(payload).hexdigest():
            _fail(f"{label} hash does not match the reviewed repository source")
    expectations_value = _object(
        builder["closure_expectations"],
        label="validator builder closure expectations",
        keys=EXPECTATION_FIELDS,
    )
    expectations = {
        field: _hex32(expectations_value[field], label=f"closure_expectations.{field}")
        for field in EXPECTATION_FIELDS
    }
    all_hash_roles = {**hashes, **expectations}
    if len(set(all_hash_roles.values())) != len(all_hash_roles):
        _fail("validator builder hash roles must be pairwise distinct")

    limits_value = _object(
        policy["limits"],
        label="validator builder limits",
        keys=(
            "max_inventory_files",
            "max_file_bytes",
            "max_total_bytes",
            "max_log_bytes",
            "timeout_seconds",
        ),
    )
    limits = {
        "max_inventory_files": _positive(
            limits_value["max_inventory_files"],
            label="limits.max_inventory_files",
            maximum=250_000,
        ),
        "max_file_bytes": _positive(
            limits_value["max_file_bytes"],
            label="limits.max_file_bytes",
            maximum=2 * 1024**3,
        ),
        "max_total_bytes": _positive(
            limits_value["max_total_bytes"],
            label="limits.max_total_bytes",
            maximum=64 * 1024**3,
        ),
        "max_log_bytes": _positive(
            limits_value["max_log_bytes"],
            label="limits.max_log_bytes",
            maximum=64 * 1024**2,
        ),
        "timeout_seconds": _positive(
            limits_value["timeout_seconds"],
            label="limits.timeout_seconds",
            maximum=4 * 60 * 60,
        ),
    }
    if limits["max_total_bytes"] < limits["max_file_bytes"]:
        _fail("validator builder total-byte limit must cover one maximum file")

    approvers_value = _list(
        policy["approvers"], label="validator builder approvers", length=2
    )
    approvers: list[dict[str, str]] = []
    signer_ids: set[str] = set()
    public_keys: set[str] = set()
    for index, role in enumerate(ROLES):
        entry = _object(
            approvers_value[index],
            label=f"approvers[{index}]",
            keys=("role", "signer_id", "public_key_hex"),
        )
        if entry["role"] != role:
            _fail("validator builder approvers must use the exact ordered roles")
        signer_id = _string(entry["signer_id"], label=f"approvers[{index}].signer_id")
        if not ID_RE.fullmatch(signer_id):
            _fail("validator builder signer id is not canonical")
        public_key = _hex32(
            entry["public_key_hex"], label=f"approvers[{index}].public_key_hex"
        )
        if not _valid_ed25519_public_key(bytes.fromhex(public_key)):
            _fail("validator builder approver key is not prime-subgroup Ed25519")
        if signer_id in signer_ids or public_key in public_keys:
            _fail("validator rebuild roles must use independent signer identities")
        signer_ids.add(signer_id)
        public_keys.add(public_key)
        approvers.append(
            {"role": role, "signer_id": signer_id, "public_key_hex": public_key}
        )

    return {
        "schema": POLICY_SCHEMA,
        "source": {
            "commit": commit,
            "commit_signer_fingerprint": fingerprint,
            "source_date_epoch": source_date_epoch,
            "secret_scan_exceptions": validated_exceptions,
        },
        "builder": {
            "image": image,
            "platform": PLATFORM,
            "driver_path": DRIVER_MOUNT,
            "driver_sha256": hashes["driver_sha256"],
            **paths,
            **versions,
            "target_triple": TARGET,
            "host_python_sha256": hashes["host_python_sha256"],
            "host_orchestrator_sha256": hashes["host_orchestrator_sha256"],
            "host_release_common_sha256": hashes["host_release_common_sha256"],
            "host_git_sha256": hashes["host_git_sha256"],
            "host_docker_sha256": hashes["host_docker_sha256"],
            "docker_daemon_report_sha256": hashes["docker_daemon_report_sha256"],
            "host_commit_verifier_sha256": hashes["host_commit_verifier_sha256"],
            "closure_expectations": expectations,
        },
        "limits": limits,
        "approvers": approvers,
    }


def rebuild_signing_payload(unsigned: Mapping[str, Any]) -> bytes:
    """Return the exact role-bound payload signed after one independent build."""

    return REBUILD_SIGNING_DOMAIN + common.canonical_json_bytes(unsigned)


def _validate_signature(
    signed: Any,
    *,
    unsigned: Mapping[str, Any],
    policy: Mapping[str, Any],
    role_index: int,
) -> dict[str, Any]:
    value = _object(
        signed,
        label="signed validator rebuild",
        keys=(*unsigned.keys(), "provenance"),
    )
    if {key: value[key] for key in unsigned} != unsigned:
        _fail("signed validator rebuild does not match its reproduced closure")
    provenance = _object(
        value["provenance"],
        label="validator rebuild provenance",
        keys=("role", "signer_id", "algorithm", "public_key_hex", "signature_b64"),
    )
    approver = policy["approvers"][role_index]
    if (
        provenance["role"] != approver["role"]
        or provenance["signer_id"] != approver["signer_id"]
        or provenance["algorithm"] != "ed25519"
        or provenance["public_key_hex"] != approver["public_key_hex"]
    ):
        _fail("validator rebuild signature is not from the approved exact role")
    try:
        signature = base64.b64decode(provenance["signature_b64"], validate=True)
    except (binascii.Error, ValueError, TypeError):
        _fail("validator rebuild signature is not canonical base64")
    if (
        len(signature) != 64
        or base64.b64encode(signature).decode("ascii") != provenance["signature_b64"]
    ):
        _fail("validator rebuild signature has the wrong canonical length")
    if not common.verify_ed25519(
        bytes.fromhex(provenance["public_key_hex"]),
        signature,
        rebuild_signing_payload(unsigned),
    ):
        _fail("validator rebuild signature is invalid")
    return {**unsigned, "provenance": provenance}


def _open_stable_executable(
    path_text: str, *, label: str, expected_sha256: str
) -> tuple[Path, tuple[int, ...]]:
    path = Path(path_text)
    if not path.is_absolute():
        _fail(f"{label} must be one absolute executable path")
    try:
        before = path.lstat()
    except OSError:
        _fail(f"{label} is unavailable")
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or not 0 < before.st_size <= MAX_HOST_EXECUTABLE_BYTES
        or before.st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH) == 0
        or before.st_mode & (stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX | 0o022)
        or before.st_uid not in (0, os.geteuid())
    ):
        _fail(f"{label} must be one direct bounded executable")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    digest = hashlib.sha256()
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
        ):
            _fail(f"{label} changed while opening")
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail(f"{label} changed while hashing")
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail(f"{label} changed while hashing")
    except OSError:
        _fail(f"{label} could not be read safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if digest.hexdigest() != expected_sha256:
        _fail(f"{label} does not match the approved SHA-256 identity")
    return path, (
        before.st_dev,
        before.st_ino,
        before.st_size,
        before.st_mtime_ns,
        before.st_ctime_ns,
    )


def _stage_host_executable(
    source: Path,
    source_identity: tuple[int, ...],
    destination: Path,
    *,
    expected_sha256: str,
    label: str,
) -> tuple[Path, tuple[int, ...]]:
    """Copy a pinned host tool into the private run directory before first use."""

    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=False)
    parent = os.open(
        destination.parent,
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        _require_unchanged(source, source_identity, label=label)
        _copy_at(
            parent,
            destination.name,
            source,
            expected_sha256=expected_sha256,
            maximum=MAX_HOST_EXECUTABLE_BYTES,
            executable=True,
        )
        os.fsync(parent)
    finally:
        os.close(parent)
    return _open_stable_executable(
        os.fspath(destination),
        label=f"staged {label}",
        expected_sha256=expected_sha256,
    )


def _require_unchanged(path: Path, identity: tuple[int, ...], *, label: str) -> None:
    try:
        after = path.lstat()
    except OSError:
        _fail(f"{label} disappeared during the build")
    if (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
    ) != identity:
        _fail(f"{label} changed during the build")


def _closed_environment(
    *,
    source_date_epoch: int | None = None,
    docker_config: Path | None = None,
) -> dict[str, str]:
    environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "PATH": os.defpath,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_TERMINAL_PROMPT": "0",
    }
    if source_date_epoch is not None:
        environment["SOURCE_DATE_EPOCH"] = str(source_date_epoch)
    if docker_config is not None:
        environment.update(
            {
                "DOCKER_CONFIG": os.fspath(docker_config),
                "DOCKER_HOST": "unix:///var/run/docker.sock",
                "DOCKER_CONTEXT": "default",
            }
        )
    for name in ("SYSTEMROOT", "WINDIR"):
        if name in os.environ:
            environment[name] = os.environ[name]
    return environment


def _run_bounded(
    executable: Path,
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    maximum_bytes: int,
    timeout_seconds: int,
    label: str,
    scan_stdout: bool = True,
) -> tuple[bytes, bytes]:
    try:
        process = subprocess.Popen(
            [os.fspath(executable), *arguments],
            cwd=cwd,
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
        )
    except OSError:
        _fail(f"{label} could not start")
    assert process.stdout is not None and process.stderr is not None
    buffers = (bytearray(), bytearray())
    overflow = [False]
    total = [0]
    lock = threading.Lock()
    errors: list[Exception] = []

    def stop_process_group() -> None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append(error)

    def read_pipe(pipe: Any, index: int) -> None:
        try:
            while True:
                chunk = pipe.read(64 * 1024)
                if not chunk:
                    return
                with lock:
                    total[0] += len(chunk)
                    if total[0] > maximum_bytes:
                        overflow[0] = True
                        stop_process_group()
                        return
                    buffers[index].extend(chunk)
        except Exception as error:  # noqa: BLE001 - fail closed on any reader fault
            errors.append(error)
            stop_process_group()

    readers = (
        threading.Thread(target=read_pipe, args=(process.stdout, 0), daemon=True),
        threading.Thread(target=read_pipe, args=(process.stderr, 1), daemon=True),
    )
    for reader in readers:
        reader.start()
    try:
        return_code = process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        stop_process_group()
        process.wait()
        _fail(f"{label} exceeded its signed time limit")
    finally:
        # The reviewed command is not allowed to leave a child holding an
        # inherited pipe or mutating build inputs after its parent exits.
        stop_process_group()
        for reader in readers:
            reader.join(timeout=5)
    if any(reader.is_alive() for reader in readers) or overflow[0] or errors:
        _fail(f"{label} exceeded its signed output limit")
    stdout, stderr = bytes(buffers[0]), bytes(buffers[1])
    if scan_stdout:
        common.reject_secret_material(stdout, label=f"{label} stdout")
    common.reject_secret_material(stderr, label=f"{label} stderr")
    if return_code != 0:
        _fail(f"{label} failed")
    return stdout, stderr


def _run_bounded_status(
    executable: Path,
    arguments: Sequence[str],
    *,
    cwd: Path,
    environment: Mapping[str, str],
    maximum_bytes: int,
    timeout_seconds: int,
    label: str,
) -> tuple[int, bytes, bytes]:
    """Run a bounded administrative probe whose nonzero status is meaningful."""

    try:
        process = subprocess.Popen(
            [os.fspath(executable), *arguments],
            cwd=cwd,
            env=dict(environment),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
        )
    except OSError:
        _fail(f"{label} could not start")
    assert process.stdout is not None and process.stderr is not None
    buffers = (bytearray(), bytearray())
    overflow = [False]
    total = [0]
    lock = threading.Lock()
    errors: list[Exception] = []

    def stop_process_group() -> None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError as error:
            errors.append(error)

    def read_pipe(pipe: Any, index: int) -> None:
        try:
            while True:
                chunk = pipe.read(64 * 1024)
                if not chunk:
                    return
                with lock:
                    total[0] += len(chunk)
                    if total[0] > maximum_bytes:
                        overflow[0] = True
                        stop_process_group()
                        return
                    buffers[index].extend(chunk)
        except Exception as error:  # noqa: BLE001 - fail closed on any reader fault
            errors.append(error)
            stop_process_group()

    readers = (
        threading.Thread(target=read_pipe, args=(process.stdout, 0), daemon=True),
        threading.Thread(target=read_pipe, args=(process.stderr, 1), daemon=True),
    )
    for reader in readers:
        reader.start()
    try:
        return_code = process.wait(timeout=timeout_seconds)
    except subprocess.TimeoutExpired:
        stop_process_group()
        process.wait()
        _fail(f"{label} exceeded its fixed time limit")
    finally:
        stop_process_group()
        for reader in readers:
            reader.join(timeout=2)
    if any(reader.is_alive() for reader in readers) or overflow[0] or errors:
        _fail(f"{label} exceeded its fixed output limit")
    stdout, stderr = bytes(buffers[0]), bytes(buffers[1])
    common.reject_secret_material(stdout, label=f"{label} stdout")
    common.reject_secret_material(stderr, label=f"{label} stderr")
    return return_code, stdout, stderr


def _git_command(
    git: Path,
    arguments: Sequence[str],
    *,
    maximum: int = 1024 * 1024,
    scan_stdout: bool = True,
) -> bytes:
    stdout, _ = _run_bounded(
        git,
        (
            "-C",
            os.fspath(ROOT),
            "-c",
            "core.fsmonitor=false",
            "-c",
            f"core.hooksPath={os.devnull}",
            *arguments,
        ),
        cwd=ROOT,
        environment=_closed_environment(),
        maximum_bytes=maximum,
        timeout_seconds=120,
        label="pinned Git operation",
        scan_stdout=scan_stdout,
    )
    return stdout


def _reject_concrete_material(
    data: bytes,
    *,
    label: str,
    inspect_json_keys: bool,
    budget: common._SecretScanBudget | None = None,
) -> None:
    """Reject concrete credentials without rejecting security terminology.

    Source trees legitimately contain identifiers such as ``PrivateKey.java``
    and ``mldsa_private_key.rs``.  Those names describe credential-handling
    code; they are not credentials.  Closure documents and paths therefore
    retain the common scanner's bounded recursive decoding and concrete
    assignment, credential-key, header, PEM, token, and URL-userinfo detectors
    while excluding its broad prose-only sensitive-term detector.
    """

    first = True
    for variant in common._secret_scan_variants(data, label=label, budget=budget):
        if (
            common._CREDENTIAL_ASSIGNMENT_RE.search(variant)
            or (inspect_json_keys and common._contains_credential_json_key(variant))
            or common._PEM_PRIVATE_KEY_RE.search(variant)
            or common._CREDENTIAL_HEADER_RE.search(variant)
            or common._CONCRETE_TOKEN_RE.search(variant)
            or common._URL_USERINFO_RE.search(variant)
        ):
            common._secret_scan_failure(encoded=not first)
        first = False


def _reject_tracked_path_material(path: bytes) -> None:
    _reject_concrete_material(
        path,
        label="Git tracked path",
        inspect_json_keys=False,
    )


def _source_tree_inventory(git: Path, commit: str) -> bytes:
    raw = _git_command(
        git,
        ("ls-tree", "-rz", "--full-tree", commit),
        maximum=64 * 1024 * 1024,
        scan_stdout=False,
    )
    if not raw or not raw.endswith(b"\x00"):
        _fail("Git source tree inventory is empty or truncated")
    entries: list[dict[str, str]] = []
    previous_path = b""
    object_pattern = re.compile(rb"^(?:[0-9a-f]{40}|[0-9a-f]{64})$")
    for record in raw[:-1].split(b"\x00"):
        try:
            header, path_bytes = record.split(b"\t", 1)
            mode, object_type, object_id = header.split(b" ")
        except ValueError:
            _fail("Git source tree inventory has a malformed record")
        if (
            not path_bytes
            or path_bytes <= previous_path
            or not object_pattern.fullmatch(object_id)
            or len(object_id) != len(commit)
            or (mode, object_type)
            not in {
                (b"100644", b"blob"),
                (b"100755", b"blob"),
                (b"120000", b"blob"),
                (b"160000", b"commit"),
            }
        ):
            _fail("Git source tree inventory contains an unsupported tracked object")
        previous_path = path_bytes
        try:
            path = path_bytes.decode("utf-8", "strict")
        except UnicodeDecodeError:
            _fail("Git source tree paths must be canonical UTF-8")
        _reject_tracked_path_material(path_bytes)
        pure = PurePosixPath(path)
        if (
            pure.is_absolute()
            or str(pure) != path
            or any(part in ("", ".", "..") for part in pure.parts)
            or any(ord(character) < 0x20 for character in path)
            or path == SOURCE_TREE_INVENTORY
        ):
            _fail("Git source tree inventory contains a non-canonical path")
        entries.append(
            {
                "mode": mode.decode("ascii"),
                "object_type": object_type.decode("ascii"),
                "object_id": object_id.decode("ascii"),
                "path": path,
            }
        )
    if not entries:
        _fail("Git source tree inventory has no tracked entries")
    value = {
        "schema": "iroha.sccp.git-source-tree-inventory.final-v1",
        "source_commit": commit,
        "object_format": "sha1" if len(commit) == 40 else "sha256",
        "entries": entries,
    }
    try:
        payload = (
            json.dumps(
                value,
                ensure_ascii=True,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            ).encode("ascii")
            + b"\n"
        )
    except (TypeError, ValueError, RecursionError):
        _fail("Git source tree inventory could not be encoded canonically")
    if len(payload) > MAX_CLOSURE_DOCUMENT_BYTES:
        _fail("Git source tree inventory exceeds its final-V1 bound")
    return payload


def _scan_signed_source_blobs(
    inventory_payload: bytes,
    policy: Mapping[str, Any],
) -> None:
    """Secret-scan every signed blob except exact policy-approved test vectors."""

    inventory = common.parse_json_bytes(
        inventory_payload,
        label="Git source tree inventory",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    entries = inventory.get("entries") if type(inventory) is dict else None
    if type(entries) is not list or not entries:
        _fail("Git source tree inventory cannot drive the secret scan")
    exceptions = {
        item["path"]: item["object_id"]
        for item in policy["source"]["secret_scan_exceptions"]
    }
    used_exceptions: set[str] = set()
    limits = policy["limits"]
    maximum_total = min(limits["max_total_bytes"], MAX_SOURCE_ARCHIVE_BYTES)
    budget = common._SecretScanBudget(
        max_variants=min(4_000_000, limits["max_inventory_files"] * 16 + 1024),
        max_decoded_bytes=min(
            16 * 1024**3,
            maximum_total * 4 + 64 * 1024**2,
        ),
        max_decoded_tokens=1_000_000,
    )
    total = 0
    files = 0
    algorithm = "sha1" if len(policy["source"]["commit"]) == 40 else "sha256"
    for entry in entries:
        if type(entry) is not dict or entry.get("object_type") != "blob":
            continue
        relative = entry.get("path")
        expected_object = entry.get("object_id")
        mode = entry.get("mode")
        if type(relative) is not str or type(expected_object) is not str:
            _fail("Git source tree inventory cannot drive the secret scan")
        path = ROOT.joinpath(*PurePosixPath(relative).parts)
        try:
            before = path.lstat()
        except OSError:
            _fail("one signed source blob is unavailable for secret scanning")
        exempt = exceptions.get(relative)
        if exempt is not None:
            if exempt != expected_object:
                _fail("source secret-scan exception binds the wrong Git object")
            used_exceptions.add(relative)
        digest = hashlib.new(algorithm)
        if mode == "120000":
            if not stat.S_ISLNK(before.st_mode):
                _fail("signed source symbolic link changed before secret scanning")
            try:
                payload = os.fsencode(os.readlink(path))
            except OSError:
                _fail("signed source symbolic link could not be secret-scanned")
            if len(payload) > limits["max_file_bytes"]:
                _fail("signed source symbolic link exceeds its final-V1 bound")
            digest.update(f"blob {len(payload)}\0".encode("ascii"))
            digest.update(payload)
            if not exempt:
                _reject_concrete_material(
                    payload,
                    label="signed source symbolic link",
                    inspect_json_keys=False,
                    budget=budget,
                )
            size = len(payload)
        else:
            expected_executable = mode == "100755"
            if (
                not stat.S_ISREG(before.st_mode)
                or before.st_nlink != 1
                or before.st_size > limits["max_file_bytes"]
                or bool(before.st_mode & stat.S_IXUSR) != expected_executable
                or before.st_mode & (stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX)
            ):
                _fail("one signed source blob is not a safe bounded direct file")
            digest.update(f"blob {before.st_size}\0".encode("ascii"))
            flags = (
                os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
            )
            overlap = b""
            try:
                descriptor = os.open(path, flags)
                opened = os.fstat(descriptor)
                if (
                    opened.st_dev,
                    opened.st_ino,
                    opened.st_size,
                    opened.st_ctime_ns,
                ) != (
                    before.st_dev,
                    before.st_ino,
                    before.st_size,
                    before.st_ctime_ns,
                ):
                    _fail("signed source blob changed while opening for secret scan")
                remaining = opened.st_size
                while remaining:
                    chunk = os.read(descriptor, min(8 * 1024 * 1024, remaining + 1))
                    if not chunk or len(chunk) > remaining:
                        _fail("signed source blob changed while secret scanning")
                    digest.update(chunk)
                    if not exempt:
                        combined = overlap + chunk
                        _reject_concrete_material(
                            combined,
                            label="signed source blob",
                            inspect_json_keys=True,
                            budget=budget,
                        )
                        overlap = combined[-STREAM_SECRET_SCAN_OVERLAP_BYTES:]
                    remaining -= len(chunk)
                if os.read(descriptor, 1):
                    _fail("signed source blob changed while secret scanning")
                after = os.fstat(descriptor)
            except OSError:
                _fail("signed source blob could not be secret-scanned safely")
            finally:
                if "descriptor" in locals():
                    os.close(descriptor)
                    del descriptor
            if (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_ctime_ns,
            ) != (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_ctime_ns,
            ):
                _fail("signed source blob changed while secret scanning")
            size = before.st_size
        if digest.hexdigest() != expected_object:
            _fail("signed source blob differs from its Git object identity")
        files += 1
        total += size
        if files > limits["max_inventory_files"] or total > maximum_total:
            _fail("signed source secret scan exceeds its aggregate final-V1 bound")
    if used_exceptions != set(exceptions):
        _fail("source secret-scan policy contains an unused exception")


def _write_private_file(path: Path, payload: bytes, *, label: str) -> None:
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o600)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail(f"{label} write made no progress")
            view = view[written:]
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if metadata.st_nlink != 1 or metadata.st_size != len(payload):
            _fail(f"{label} inode changed during publication")
    except OSError:
        _fail(f"{label} could not be published safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)


def _verify_source_and_archive(
    git: Path,
    commit_verifier: Path,
    policy: Mapping[str, Any],
    archive_path: Path,
) -> tuple[str, int]:
    source = policy["source"]
    commit = source["commit"]
    try:
        top = (
            _git_command(git, ("rev-parse", "--show-toplevel"))
            .decode("utf-8", "strict")
            .strip()
        )
    except UnicodeDecodeError:
        _fail("Git top-level path is not UTF-8")
    if Path(top).resolve() != ROOT.resolve():
        _fail("source root is not the exact Git top-level")
    head = _git_command(git, ("rev-parse", "--verify", "HEAD")).decode().strip()
    if head != commit:
        _fail("source HEAD does not match the approved full commit")
    status = _git_command(
        git,
        ("status", "--porcelain=v1", "--untracked-files=all"),
        maximum=32 * 1024 * 1024,
    )
    if status:
        _fail("production validator builds require a completely clean source tree")
    verifier_path = os.fspath(commit_verifier)
    if (
        not commit_verifier.is_absolute()
        or len(verifier_path) > 1024
        or not SHELL_INERT_HOST_PATH_RE.fullmatch(verifier_path)
        or any(part in ("", ".", "..") for part in commit_verifier.parts[1:])
    ):
        _fail("commit signature verifier path is not canonical shell-inert text")
    signature_configuration = (
        "-c",
        "gpg.format=openpgp",
        "-c",
        f"gpg.openpgp.program={verifier_path}",
        "-c",
        f"gpg.program={verifier_path}",
    )
    _git_command(
        git,
        (*signature_configuration, "verify-commit", "--raw", commit),
        maximum=256 * 1024,
    )
    signature = _git_command(
        git,
        (
            *signature_configuration,
            "show",
            "--no-patch",
            "--format=%G?%x00%GF%x00%GP%x00",
            commit,
        ),
        maximum=8 * 1024,
    ).rstrip(b"\n")
    fields = signature.split(b"\x00")
    if len(fields) != 4 or fields[0] != b"G" or fields[3] != b"":
        _fail("source commit signature is not fully valid")
    try:
        fingerprints = {fields[1].decode("ascii"), fields[2].decode("ascii")}
    except UnicodeDecodeError:
        _fail("source commit signature fingerprint is malformed")
    if source["commit_signer_fingerprint"] not in fingerprints:
        _fail("source commit signer does not match the approved fingerprint")
    timestamp = (
        _git_command(git, ("show", "-s", "--format=%ct", commit)).decode().strip()
    )
    if timestamp != str(source["source_date_epoch"]):
        _fail("source commit time does not match approved SOURCE_DATE_EPOCH")

    inventory_path = archive_path.parent / SOURCE_TREE_INVENTORY
    inventory_payload = _source_tree_inventory(git, commit)
    _scan_signed_source_blobs(inventory_payload, policy)
    _write_private_file(
        inventory_path,
        inventory_payload,
        label="Git source tree inventory",
    )
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(archive_path, flags, 0o600)
    except OSError:
        _fail("tracked source archive could not be reserved safely")
    digest = hashlib.sha256()
    total = [0]
    archive_overflow = [False]
    diagnostic_overflow = [False]
    diagnostics = bytearray()
    stream_errors: list[Exception] = []
    try:
        process = subprocess.Popen(
            [
                os.fspath(git),
                "-C",
                os.fspath(ROOT),
                "archive",
                "--format=tar",
                "--prefix=source/",
                f"--mtime=@{source['source_date_epoch']}",
                f"--add-file={inventory_path}",
                commit,
            ],
            cwd=ROOT,
            env=_closed_environment(source_date_epoch=source["source_date_epoch"]),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            start_new_session=True,
        )
        assert process.stdout is not None and process.stderr is not None

        def stop_process_group() -> None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except OSError as error:
                stream_errors.append(error)

        def stream_archive() -> None:
            try:
                while True:
                    chunk = process.stdout.read(1024 * 1024)
                    if not chunk:
                        return
                    total[0] += len(chunk)
                    if total[0] > MAX_SOURCE_ARCHIVE_BYTES:
                        archive_overflow[0] = True
                        stop_process_group()
                        return
                    digest.update(chunk)
                    view = memoryview(chunk)
                    while view:
                        written = os.write(descriptor, view)
                        if written <= 0:
                            raise OSError("archive write made no progress")
                        view = view[written:]
            except Exception as error:  # noqa: BLE001 - fail closed on stream faults
                stream_errors.append(error)
                stop_process_group()

        def stream_diagnostics() -> None:
            try:
                while True:
                    chunk = process.stderr.read(64 * 1024)
                    if not chunk:
                        return
                    if len(diagnostics) + len(chunk) > 256 * 1024:
                        diagnostic_overflow[0] = True
                        stop_process_group()
                        return
                    diagnostics.extend(chunk)
            except Exception as error:  # noqa: BLE001 - fail closed on stream faults
                stream_errors.append(error)
                stop_process_group()

        readers = (
            threading.Thread(target=stream_archive, daemon=True),
            threading.Thread(target=stream_diagnostics, daemon=True),
        )
        for reader in readers:
            reader.start()
        try:
            return_code = process.wait(timeout=180)
        except subprocess.TimeoutExpired:
            stop_process_group()
            process.wait()
            _fail("Git source archive exceeded its fixed time limit")
        finally:
            stop_process_group()
            for reader in readers:
                reader.join(timeout=5)
        if any(reader.is_alive() for reader in readers) or stream_errors:
            _fail("Git source archive streaming failed safely")
        common.reject_secret_material(
            bytes(diagnostics), label="Git archive diagnostics"
        )
        if (
            return_code != 0
            or archive_overflow[0]
            or diagnostic_overflow[0]
            or total[0] == 0
        ):
            _fail("Git could not produce the complete bounded source archive")
        os.fsync(descriptor)
        # The fixed unprivileged container identity must be able to read this
        # bind-mounted inode after every capability is dropped.  The containing
        # host directory remains owner-only and the mount itself is read-only.
        os.fchmod(descriptor, 0o444)
        metadata = os.fstat(descriptor)
        if metadata.st_size != total[0] or metadata.st_nlink != 1:
            _fail("tracked source archive inode changed during publication")
    except OSError:
        _fail("Git source archive operation failed safely")
    finally:
        os.close(descriptor)
    if _git_command(git, ("rev-parse", "--verify", "HEAD")).decode().strip() != commit:
        _fail("source HEAD changed during archival")
    if _git_command(git, ("status", "--porcelain=v1", "--untracked-files=all")):
        _fail("source tree changed during archival")
    archive_sha256, archive_size, archive_executable = _hash_direct_file(
        archive_path,
        label="tracked source archive",
        maximum=MAX_SOURCE_ARCHIVE_BYTES,
    )
    if (
        archive_executable
        or archive_sha256 != digest.hexdigest()
        or archive_size != total[0]
    ):
        _fail("tracked source archive changed before secret scanning")
    return archive_sha256, archive_size


def _inspect_image(
    docker: Path,
    docker_identity: tuple[int, ...],
    policy: Mapping[str, Any],
    docker_environment: Mapping[str, str],
) -> None:
    stdout, _ = _run_bounded(
        docker,
        ("image", "inspect", policy["builder"]["image"]),
        cwd=ROOT,
        environment=docker_environment,
        maximum_bytes=2 * 1024 * 1024,
        timeout_seconds=120,
        label="pinned validator builder image inspection",
    )
    try:
        value = json.loads(stdout.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        _fail("validator builder image inspection returned malformed JSON")
    if type(value) is not list or len(value) != 1 or type(value[0]) is not dict:
        _fail("validator builder image inspection returned the wrong shape")
    image = value[0]
    if image.get("Os") != "linux" or image.get("Architecture") != "amd64":
        _fail("validator builder image is not linux/amd64")
    if type(image.get("Id")) is not str or not re.fullmatch(
        r"sha256:[0-9a-f]{64}", image["Id"]
    ):
        _fail("validator builder image lacks a content-addressed local identity")
    config = image.get("Config")
    if type(config) is not dict:
        _fail("validator builder image omits its runtime configuration")
    if config.get("Volumes") not in (None, {}):
        _fail("validator builder image declares ambient storage volumes")
    if config.get("Healthcheck") not in (None, {"Test": ["NONE"]}):
        _fail("validator builder image declares an active healthcheck")
    expected = policy["builder"]["image"].rsplit("@", 1)[1]
    repo_digests = image.get("RepoDigests")
    if (
        type(repo_digests) is not list
        or not repo_digests
        or any(type(item) is not str for item in repo_digests)
        or not any(item.endswith("@" + expected) for item in repo_digests)
    ):
        _fail(
            "local validator builder image does not expose the approved manifest digest"
        )
    _require_unchanged(docker, docker_identity, label="pinned Docker executable")


def _inspect_docker_daemon(
    docker: Path,
    docker_identity: tuple[int, ...],
    policy: Mapping[str, Any],
    docker_environment: Mapping[str, str],
) -> None:
    """Bind the security-relevant local Docker daemon identity to policy."""

    stdout, _ = _run_bounded(
        docker,
        ("info", "--format={{json .}}"),
        cwd=ROOT,
        environment=docker_environment,
        maximum_bytes=4 * 1024 * 1024,
        timeout_seconds=120,
        label="local validator Docker daemon inspection",
    )
    try:
        value = json.loads(stdout.decode("utf-8", "strict"))
    except (UnicodeDecodeError, json.JSONDecodeError, RecursionError):
        _fail("local validator Docker daemon inspection returned malformed JSON")
    if type(value) is not dict:
        _fail("local validator Docker daemon inspection returned the wrong shape")
    strings = {
        name: value.get(name)
        for name in (
            "ServerVersion",
            "OperatingSystem",
            "OSType",
            "Architecture",
            "KernelVersion",
            "Driver",
            "CgroupDriver",
            "CgroupVersion",
            "DefaultRuntime",
            "Isolation",
        )
    }
    if any(type(item) is not str for item in strings.values()):
        _fail("local validator Docker daemon omits a required identity field")
    if (
        strings["OSType"] != "linux"
        or strings["Architecture"]
        not in (
            "x86_64",
            "amd64",
        )
        or strings["DefaultRuntime"] != "runc"
    ):
        _fail("local validator Docker daemon is not exact Linux/amd64")
    booleans = {
        name: value.get(name) for name in ("ExperimentalBuild", "LiveRestoreEnabled")
    }
    if any(type(item) is not bool for item in booleans.values()):
        _fail("local validator Docker daemon omits a required security flag")
    security_options = value.get("SecurityOptions")
    server_errors = value.get("ServerErrors")
    if (
        type(security_options) is not list
        or any(type(item) is not str for item in security_options)
        or len(set(security_options)) != len(security_options)
        or server_errors != []
        or not any(option.startswith("name=seccomp") for option in security_options)
    ):
        _fail("local validator Docker daemon has malformed security state")
    commits: dict[str, dict[str, str]] = {}
    for name in ("ContainerdCommit", "RuncCommit", "InitCommit"):
        item = value.get(name)
        if (
            type(item) is not dict
            or set(item) != {"ID", "Expected"}
            or any(type(item[field]) is not str or not item[field] for field in item)
            or item["ID"] != item["Expected"]
        ):
            _fail("local validator Docker daemon omits a runtime commitment")
        commits[name] = {"ID": item["ID"], "Expected": item["Expected"]}
    report = {
        "schema": "iroha.sccp.validator-docker-daemon.final-v1",
        **strings,
        **booleans,
        "SecurityOptions": sorted(security_options),
        "ServerErrors": [],
        **commits,
    }
    digest = hashlib.sha256(common.canonical_json_file_bytes(report)).hexdigest()
    if digest != policy["builder"]["docker_daemon_report_sha256"]:
        _fail("local validator Docker daemon differs from immutable policy")
    _require_unchanged(docker, docker_identity, label="pinned Docker executable")


def _mount_source(path: Path, *, label: str) -> str:
    value = os.fspath(path.absolute())
    if "," in value or any(ord(character) < 0x20 for character in value):
        _fail(f"{label} cannot be represented as one Docker bind source")
    return value


_OUTPUT_ARCHIVE_DIRECTORIES = ("output", "output/closure", "output/validator")
_OUTPUT_ARCHIVE_FILES = (
    "builder-report.json",
    "closure/build-environment.json",
    "closure/build-recipe.json",
    "closure/cargo-config.toml",
    "closure/cargo-metadata-closure.json",
    "closure/dependency-inventory.json",
    "closure/sbom.json",
    "closure/sysroot-inventory.json",
    "closure/toolchain-inventory.json",
    f"validator/{BINARY}",
)
_USTAR_BLOCK_BYTES = 512
_USTAR_RECORD_BYTES = 20 * _USTAR_BLOCK_BYTES
# The base names were reviewed from Moby profiles seccomp/v0.2.1, whose tagged
# source has the fixed digest below.  SCCP narrows that base to the operations
# needed by CPython, Cargo, rustc, the linker, and the Landlock driver.  In
# particular, networking is limited to AF_UNIX, namespace creation is masked
# out of clone, and SysV IPC, POSIX message queues, Linux AIO, io_uring, BPF,
# ptrace, mounts, keyrings, and every unknown/future syscall fail closed.
_SECCOMP_BASELINE = "moby/profiles:seccomp/v0.2.1"
_SECCOMP_BASELINE_SHA256 = (
    "536529b665dd0972c37bfb569f5d4ac8a53592e7b00752bc39ff063ca9864c74"
)
_SECCOMP_ALLOWED_SYSCALLS = (
    "_llseek",
    "_newselect",
    "accept",
    "accept4",
    "access",
    "alarm",
    "arch_prctl",
    "bind",
    "brk",
    "cachestat",
    "capget",
    "capset",
    "chdir",
    "chmod",
    "chown",
    "chown32",
    "clock_getres",
    "clock_getres_time64",
    "clock_gettime",
    "clock_gettime64",
    "clock_nanosleep",
    "clock_nanosleep_time64",
    "close",
    "close_range",
    "connect",
    "copy_file_range",
    "creat",
    "dup",
    "dup2",
    "dup3",
    "epoll_create",
    "epoll_create1",
    "epoll_ctl",
    "epoll_ctl_old",
    "epoll_pwait",
    "epoll_pwait2",
    "epoll_wait",
    "epoll_wait_old",
    "eventfd",
    "eventfd2",
    "execve",
    "execveat",
    "exit",
    "exit_group",
    "faccessat",
    "faccessat2",
    "fadvise64",
    "fadvise64_64",
    "fallocate",
    "fchdir",
    "fchmod",
    "fchmodat",
    "fchmodat2",
    "fchown",
    "fchown32",
    "fchownat",
    "fcntl",
    "fcntl64",
    "fdatasync",
    "fgetxattr",
    "flistxattr",
    "flock",
    "fork",
    "fremovexattr",
    "fsetxattr",
    "fstat",
    "fstat64",
    "fstatat64",
    "fstatfs",
    "fstatfs64",
    "fsync",
    "ftruncate",
    "ftruncate64",
    "futex",
    "futex_requeue",
    "futex_time64",
    "futex_wait",
    "futex_waitv",
    "futex_wake",
    "futimesat",
    "get_robust_list",
    "get_thread_area",
    "getcpu",
    "getcwd",
    "getdents",
    "getdents64",
    "getegid",
    "getegid32",
    "geteuid",
    "geteuid32",
    "getgid",
    "getgid32",
    "getgroups",
    "getgroups32",
    "getitimer",
    "getpeername",
    "getpgid",
    "getpgrp",
    "getpid",
    "getppid",
    "getpriority",
    "getrandom",
    "getresgid",
    "getresgid32",
    "getresuid",
    "getresuid32",
    "getrlimit",
    "getrusage",
    "getsid",
    "getsockname",
    "getsockopt",
    "gettid",
    "gettimeofday",
    "getuid",
    "getuid32",
    "getxattr",
    "getxattrat",
    "inotify_add_watch",
    "inotify_init",
    "inotify_init1",
    "inotify_rm_watch",
    "ioctl",
    "ioprio_get",
    "ioprio_set",
    "kill",
    "landlock_add_rule",
    "landlock_create_ruleset",
    "landlock_restrict_self",
    "lchown",
    "lchown32",
    "lgetxattr",
    "link",
    "linkat",
    "listen",
    "listmount",
    "listxattr",
    "listxattrat",
    "llistxattr",
    "lremovexattr",
    "lseek",
    "lsetxattr",
    "lstat",
    "lstat64",
    "madvise",
    "map_shadow_stack",
    "membarrier",
    "memfd_create",
    "mincore",
    "mkdir",
    "mkdirat",
    "mlock",
    "mlock2",
    "mlockall",
    "mmap",
    "mmap2",
    "mprotect",
    "mremap",
    "mseal",
    "msync",
    "munlock",
    "munlockall",
    "munmap",
    "nanosleep",
    "newfstatat",
    "open",
    "openat",
    "openat2",
    "pause",
    "pipe",
    "pipe2",
    "pkey_alloc",
    "pkey_free",
    "pkey_mprotect",
    "poll",
    "ppoll",
    "ppoll_time64",
    "prctl",
    "pread64",
    "preadv",
    "preadv2",
    "prlimit64",
    "pselect6",
    "pselect6_time64",
    "pwrite64",
    "pwritev",
    "pwritev2",
    "read",
    "readahead",
    "readlink",
    "readlinkat",
    "readv",
    "recv",
    "recvfrom",
    "recvmmsg",
    "recvmmsg_time64",
    "recvmsg",
    "remap_file_pages",
    "removexattr",
    "removexattrat",
    "rename",
    "renameat",
    "renameat2",
    "restart_syscall",
    "riscv_hwprobe",
    "rmdir",
    "rseq",
    "rt_sigaction",
    "rt_sigpending",
    "rt_sigprocmask",
    "rt_sigqueueinfo",
    "rt_sigreturn",
    "rt_sigsuspend",
    "rt_sigtimedwait",
    "rt_sigtimedwait_time64",
    "rt_tgsigqueueinfo",
    "sched_get_priority_max",
    "sched_get_priority_min",
    "sched_getaffinity",
    "sched_getattr",
    "sched_getparam",
    "sched_getscheduler",
    "sched_rr_get_interval",
    "sched_rr_get_interval_time64",
    "sched_setaffinity",
    "sched_setattr",
    "sched_setparam",
    "sched_setscheduler",
    "sched_yield",
    "select",
    "send",
    "sendfile",
    "sendfile64",
    "sendmmsg",
    "sendmsg",
    "sendto",
    "set_robust_list",
    "set_thread_area",
    "set_tid_address",
    "setfsgid",
    "setfsgid32",
    "setfsuid",
    "setfsuid32",
    "setgid",
    "setgid32",
    "setgroups",
    "setgroups32",
    "setitimer",
    "setpgid",
    "setpriority",
    "setregid",
    "setregid32",
    "setresgid",
    "setresgid32",
    "setresuid",
    "setresuid32",
    "setreuid",
    "setreuid32",
    "setrlimit",
    "setsid",
    "setsockopt",
    "setuid",
    "setuid32",
    "setxattr",
    "setxattrat",
    "shutdown",
    "sigaltstack",
    "signalfd",
    "signalfd4",
    "sigprocmask",
    "sigreturn",
    "socketpair",
    "splice",
    "stat",
    "stat64",
    "statfs",
    "statfs64",
    "statmount",
    "statx",
    "symlink",
    "symlinkat",
    "sync",
    "sync_file_range",
    "syncfs",
    "sysinfo",
    "tee",
    "tgkill",
    "time",
    "timer_create",
    "timer_delete",
    "timer_getoverrun",
    "timer_gettime",
    "timer_gettime64",
    "timer_settime",
    "timer_settime64",
    "timerfd_create",
    "timerfd_gettime",
    "timerfd_gettime64",
    "timerfd_settime",
    "timerfd_settime64",
    "times",
    "tkill",
    "truncate",
    "truncate64",
    "ugetrlimit",
    "umask",
    "uname",
    "unlink",
    "unlinkat",
    "uretprobe",
    "utime",
    "utimensat",
    "utimensat_time64",
    "utimes",
    "vfork",
    "vmsplice",
    "wait4",
    "waitid",
    "waitpid",
    "write",
    "writev",
)
_SECCOMP_UNIX_DOMAIN = 1  # AF_UNIX
_SECCOMP_CLONE_NAMESPACE_FLAGS = (
    0x00020000,  # CLONE_NEWNS
    0x02000000,  # CLONE_NEWCGROUP
    0x04000000,  # CLONE_NEWUTS
    0x08000000,  # CLONE_NEWIPC
    0x10000000,  # CLONE_NEWUSER
    0x20000000,  # CLONE_NEWPID
    0x40000000,  # CLONE_NEWNET
)
_SECCOMP_CLONE_NAMESPACE_MASK = 0x7E020000
_SECCOMP_PROFILE = {
    "defaultAction": "SCMP_ACT_ERRNO",
    "defaultErrnoRet": 1,
    "archMap": [
        {
            "architecture": "SCMP_ARCH_X86_64",
            "subArchitectures": ["SCMP_ARCH_X86", "SCMP_ARCH_X32"],
        }
    ],
    "syscalls": [
        {
            "names": list(_SECCOMP_ALLOWED_SYSCALLS),
            "action": "SCMP_ACT_ALLOW",
        },
        {
            "names": ["socket"],
            "action": "SCMP_ACT_ALLOW",
            "args": [
                {
                    "index": 0,
                    "value": _SECCOMP_UNIX_DOMAIN,
                    "valueTwo": 0,
                    "op": "SCMP_CMP_EQ",
                }
            ],
        },
        {
            "names": ["clone"],
            "action": "SCMP_ACT_ALLOW",
            "args": [
                {
                    "index": 0,
                    "value": _SECCOMP_CLONE_NAMESPACE_MASK,
                    "valueTwo": 0,
                    "op": "SCMP_CMP_MASKED_EQ",
                }
            ],
        },
        {
            "names": ["clone3"],
            "action": "SCMP_ACT_ERRNO",
            "errnoRet": 38,
        },
    ],
}
_CONTAINER_MANIFEST_DOMAIN = b"iroha:sccp:validator-container:final-v1\x00"


def _seccomp_profile_bytes() -> bytes:
    return common.canonical_json_file_bytes(_SECCOMP_PROFILE)


def _container_manifest_sha256(builder_image: str) -> str:
    manifest = {
        "schema": "iroha.sccp.validator-container-manifest.final-v1",
        "builder_image": builder_image,
        "platform": PLATFORM,
        "runtime": "runc",
        "user": "65534:65534",
        "network": "none",
        "ipc": "none",
        "cgroup_namespace": "private",
        "log_driver": "none",
        "healthcheck": "disabled",
        "hostname": "iroha-sccp-validator",
        "read_only_root": True,
        "capabilities": [],
        "no_new_privileges": True,
        "seccomp_baseline": _SECCOMP_BASELINE,
        "seccomp_baseline_sha256": _SECCOMP_BASELINE_SHA256,
        "seccomp_profile_sha256": hashlib.sha256(_seccomp_profile_bytes()).hexdigest(),
    }
    return hashlib.sha256(
        _CONTAINER_MANIFEST_DOMAIN + common.canonical_json_bytes(manifest)
    ).hexdigest()


def _read_exact_descriptor(descriptor: int, size: int) -> bytes:
    payload = bytearray()
    while len(payload) < size:
        chunk = os.read(descriptor, size - len(payload))
        if not chunk:
            _fail("validator build output archive is truncated")
        payload.extend(chunk)
    return bytes(payload)


def _canonical_ustar_name(header: bytes) -> str:
    name = header[:100]
    terminator = name.find(b"\0")
    if terminator <= 0 or any(name[terminator:]) or any(header[345:500]):
        _fail("validator build output archive has a noncanonical USTAR name")
    try:
        return name[:terminator].decode("ascii", "strict")
    except UnicodeDecodeError:
        _fail("validator build output archive has a non-ASCII USTAR name")


def _canonical_ustar_octal(field: bytes, *, digits: int) -> int:
    if (
        len(field) != digits + 1
        or field[-1:] != b"\0"
        or any(byte < ord("0") or byte > ord("7") for byte in field[:-1])
    ):
        _fail("validator build output archive has a noncanonical USTAR number")
    return int(field[:-1], 8)


def _prevalidate_output_ustar(
    archive_path: Path,
    *,
    expected: Sequence[str],
    policy: Mapping[str, Any],
) -> None:
    """Reject extension records and oversized payloads before ``tarfile`` sees them."""

    maximum_archive_bytes = policy["limits"]["max_total_bytes"] + 16 * 1024 * 1024
    try:
        before = archive_path.lstat()
    except OSError:
        _fail("validator build output archive is unavailable")
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or not 0 < before.st_size <= maximum_archive_bytes
    ):
        _fail("validator build output archive is not one bounded direct file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(archive_path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
        ):
            _fail("validator build output archive changed while opening")
        total = 0
        for expected_name in expected:
            header = _read_exact_descriptor(descriptor, _USTAR_BLOCK_BYTES)
            if header[257:265] != b"ustar\x0000":
                _fail("validator build output is not canonical USTAR")
            type_flag = header[156:157]
            directory = expected_name in _OUTPUT_ARCHIVE_DIRECTORIES
            if type_flag != (tarfile.DIRTYPE if directory else tarfile.REGTYPE):
                _fail("validator build output contains a forbidden tar record type")
            raw_expected_name = f"{expected_name}/" if directory else expected_name
            if _canonical_ustar_name(header) != raw_expected_name:
                _fail("validator build output archive has an inexact ordered inventory")
            checksum = header[148:156]
            if not re.fullmatch(rb"[0-7]{6}\x00 ", checksum):
                _fail("validator build output archive has a malformed USTAR checksum")
            expected_checksum = int(checksum[:6], 8)
            observed_checksum = sum(header[:148]) + 8 * ord(" ") + sum(header[156:])
            if expected_checksum != observed_checksum:
                _fail("validator build output archive has an invalid USTAR checksum")
            size = _canonical_ustar_octal(header[124:136], digits=11)
            if directory:
                if size != 0:
                    _fail("validator build output archive has a malformed directory")
            else:
                relative = PurePosixPath(expected_name).relative_to("output")
                executable = expected_name == f"output/validator/{BINARY}"
                maximum = (
                    policy["limits"]["max_file_bytes"]
                    if executable
                    else MAX_REPORT_BYTES
                    if relative.as_posix() == "builder-report.json"
                    else MAX_CLOSURE_DOCUMENT_BYTES
                )
                if not 0 < size <= maximum:
                    _fail("validator build output archive has an oversized file entry")
                total += size
                if total > policy["limits"]["max_total_bytes"]:
                    _fail(
                        "validator build output archive exceeds its signed aggregate limit"
                    )
            padded_size = (size + _USTAR_BLOCK_BYTES - 1) // _USTAR_BLOCK_BYTES
            padded_size *= _USTAR_BLOCK_BYTES
            if os.lseek(descriptor, padded_size, os.SEEK_CUR) > opened.st_size:
                _fail("validator build output archive is truncated")
        if any(_read_exact_descriptor(descriptor, _USTAR_BLOCK_BYTES)) or any(
            _read_exact_descriptor(descriptor, _USTAR_BLOCK_BYTES)
        ):
            _fail("validator build output archive omits its canonical end markers")
        consumed = os.lseek(descriptor, 0, os.SEEK_CUR)
        canonical_size = (
            (consumed + _USTAR_RECORD_BYTES - 1) // _USTAR_RECORD_BYTES
        ) * _USTAR_RECORD_BYTES
        if opened.st_size != canonical_size:
            _fail("validator build output archive has noncanonical record padding")
        after = os.fstat(descriptor)
    except OSError:
        _fail("validator build output archive could not be prevalidated safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_ctime_ns,
    ) != (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns):
        _fail("validator build output archive changed during prevalidation")


def _docker_container_absent(
    docker: Path,
    reference: str,
    docker_environment: Mapping[str, str],
) -> bool:
    if re.fullmatch(r"[0-9a-f]{64}", reference):
        selector = f"id={reference}"
    elif re.fullmatch(r"[a-z0-9][a-z0-9_.-]{0,127}", reference):
        selector = f"name=^/{reference}$"
    else:
        _fail("validator builder container absence reference is not canonical")
    return_code, stdout, _ = _run_bounded_status(
        docker,
        (
            "container",
            "ls",
            "--all",
            "--no-trunc",
            f"--filter={selector}",
            "--format={{.ID}}",
        ),
        cwd=ROOT,
        environment=docker_environment,
        maximum_bytes=64 * 1024,
        timeout_seconds=30,
        label="validator builder container absence probe",
    )
    if return_code != 0:
        _fail("validator builder container absence probe failed")
    lines = stdout.splitlines()
    if any(not re.fullmatch(rb"[0-9a-f]{64}", line) for line in lines):
        _fail("validator builder container absence probe was not canonical")
    return not lines


def _remove_container_and_verify(
    docker: Path,
    name: str,
    docker_environment: Mapping[str, str],
) -> None:
    return_code, _, _ = _run_bounded_status(
        docker,
        ("container", "rm", "--force", name),
        cwd=ROOT,
        environment=docker_environment,
        maximum_bytes=64 * 1024,
        timeout_seconds=60,
        label="validator builder container cleanup",
    )
    if return_code != 0 or not _docker_container_absent(
        docker,
        name,
        docker_environment,
    ):
        _fail("validator builder container cleanup could not prove absence")


def _extract_output_archive(
    archive_path: Path,
    output: Path,
    *,
    policy: Mapping[str, Any],
) -> None:
    """Extract only the driver's exact canonical USTAR output inventory."""

    if os.path.lexists(output):
        _fail("validator build output path unexpectedly exists")
    expected = (
        *_OUTPUT_ARCHIVE_DIRECTORIES,
        *(f"output/{name}" for name in _OUTPUT_ARCHIVE_FILES),
    )
    _prevalidate_output_ustar(archive_path, expected=expected, policy=policy)
    total = 0
    try:
        stream = tarfile.open(archive_path, mode="r:")  # noqa: SIM115 - translated errors
    except (OSError, tarfile.TarError):
        _fail("validator build output is not one canonical USTAR archive")
    try:
        for expected_name in expected:
            member = stream.next()
            if member is None or member.name != expected_name:
                _fail("validator build output archive has an inexact ordered inventory")
            if (
                member.uid != 0
                or member.gid != 0
                or member.uname != ""
                or member.gname != ""
                or member.mtime != policy["source"]["source_date_epoch"]
                or member.linkname
                or member.pax_headers
                or member.sparse is not None
            ):
                _fail("validator build output archive has noncanonical metadata")
            relative = PurePosixPath(expected_name).relative_to("output")
            destination = output.joinpath(*relative.parts) if relative.parts else output
            if expected_name in _OUTPUT_ARCHIVE_DIRECTORIES:
                if not member.isdir() or member.mode != 0o700 or member.size != 0:
                    _fail("validator build output archive has a malformed directory")
                destination.mkdir(mode=0o700)
                continue
            executable = expected_name == f"output/validator/{BINARY}"
            maximum = (
                policy["limits"]["max_file_bytes"]
                if executable
                else MAX_REPORT_BYTES
                if expected_name == "output/builder-report.json"
                else MAX_CLOSURE_DOCUMENT_BYTES
            )
            if (
                not member.isfile()
                or member.mode != (0o700 if executable else 0o600)
                or not 0 < member.size <= maximum
            ):
                _fail("validator build output archive has an unsafe file entry")
            total += member.size
            if total > policy["limits"]["max_total_bytes"]:
                _fail(
                    "validator build output archive exceeds its signed aggregate limit"
                )
            source = stream.extractfile(member)
            if source is None:
                _fail("validator build output archive file has no payload")
            flags = (
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0)
            )
            try:
                descriptor = os.open(destination, flags, 0o700 if executable else 0o600)
                remaining = member.size
                while remaining:
                    chunk = source.read(min(1024 * 1024, remaining))
                    if not chunk:
                        _fail("validator build output archive file is truncated")
                    view = memoryview(chunk)
                    while view:
                        written = os.write(descriptor, view)
                        if written <= 0:
                            _fail("validator build output extraction made no progress")
                        view = view[written:]
                    remaining -= len(chunk)
                if source.read(1):
                    _fail("validator build output archive file exceeds its header size")
                os.fsync(descriptor)
                os.fchmod(descriptor, 0o700 if executable else 0o600)
                metadata = os.fstat(descriptor)
                if metadata.st_nlink != 1 or metadata.st_size != member.size:
                    _fail("validator build output extraction changed inode identity")
            except OSError:
                _fail("validator build output archive could not be extracted safely")
            finally:
                source.close()
                if "descriptor" in locals():
                    os.close(descriptor)
                    del descriptor
        if stream.next() is not None:
            _fail("validator build output archive contains an extra member")
    except (OSError, tarfile.TarError):
        _fail("validator build output archive could not be decoded safely")
    finally:
        stream.close()

    canonical_path = archive_path.with_name("canonical-output.tar")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(canonical_path, flags, 0o600)
        with (
            os.fdopen(descriptor, "wb", closefd=True) as sink,
            tarfile.open(
                fileobj=sink, mode="w|", format=tarfile.USTAR_FORMAT
            ) as archive,
        ):
            for name in _OUTPUT_ARCHIVE_DIRECTORIES:
                info = tarfile.TarInfo(name)
                info.type = tarfile.DIRTYPE
                info.mode = 0o700
                info.mtime = policy["source"]["source_date_epoch"]
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                archive.addfile(info)
            for relative_name in _OUTPUT_ARCHIVE_FILES:
                path = output / relative_name
                maximum = (
                    policy["limits"]["max_file_bytes"]
                    if relative_name == f"validator/{BINARY}"
                    else MAX_REPORT_BYTES
                    if relative_name == "builder-report.json"
                    else MAX_CLOSURE_DOCUMENT_BYTES
                )
                _, size, executable = _hash_direct_file(
                    path,
                    label="extracted validator build output",
                    maximum=maximum,
                )
                info = tarfile.TarInfo(f"output/{relative_name}")
                info.type = tarfile.REGTYPE
                info.size = size
                info.mode = 0o700 if executable else 0o600
                info.mtime = policy["source"]["source_date_epoch"]
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                source_descriptor = os.open(
                    path,
                    os.O_RDONLY
                    | getattr(os, "O_CLOEXEC", 0)
                    | getattr(os, "O_NOFOLLOW", 0),
                )
                with os.fdopen(source_descriptor, "rb", closefd=True) as source:
                    archive.addfile(info, source)
    except (OSError, tarfile.TarError):
        _fail("validator build output could not be canonicalized safely")
    archive_sha256, _, _ = _hash_direct_file(
        archive_path,
        label="streamed validator build output archive",
        maximum=policy["limits"]["max_total_bytes"] + 16 * 1024 * 1024,
    )
    _files_byte_identical(
        archive_path,
        canonical_path,
        expected_sha256=archive_sha256,
        maximum=policy["limits"]["max_total_bytes"] + 16 * 1024 * 1024,
        label="canonical validator build output archive",
    )


def _run_container_build(
    docker: Path,
    docker_identity: tuple[int, ...],
    policy: Mapping[str, Any],
    policy_sha256: str,
    source_archive: Path,
    source_archive_sha256: str,
    output: Path,
    docker_environment: Mapping[str, str] | None = None,
) -> None:
    builder = policy["builder"]
    limits = policy["limits"]
    if docker_environment is None:
        docker_environment = _closed_environment()
    source_metadata = source_archive.lstat()
    source_identity = (
        source_metadata.st_dev,
        source_metadata.st_ino,
        source_metadata.st_size,
        source_metadata.st_mtime_ns,
        source_metadata.st_ctime_ns,
    )
    observed_source_hash, observed_source_size, _ = _hash_direct_file(
        source_archive,
        label="container-mounted source archive",
        maximum=min(limits["max_total_bytes"], MAX_SOURCE_ARCHIVE_BYTES),
    )
    if (
        observed_source_hash != source_archive_sha256
        or observed_source_size != source_metadata.st_size
    ):
        _fail("container-mounted source archive differs from its host commitment")
    source_mount = (
        "type=bind,src="
        + _mount_source(source_archive, label="source archive path")
        + ",dst=/input/source.tar,readonly"
    )
    output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="iroha-sccp-validator-container-",
        dir=output.parent,
    ) as transfer_text:
        transfer = Path(transfer_text)
        os.chmod(transfer, 0o700)
        name = "iroha-sccp-validator-" + secrets.token_hex(16)
        if not _docker_container_absent(docker, name, docker_environment):
            _fail("fresh validator builder container name is already occupied")
        cidfile = transfer / "container.cid"
        archive_path = transfer / "output.tar"
        seccomp_profile = transfer / "seccomp-profile.json"
        _write_private_file(
            seccomp_profile,
            _seccomp_profile_bytes(),
            label="validator builder seccomp profile",
        )
        seccomp_hash, seccomp_size, seccomp_executable = _hash_direct_file(
            seccomp_profile,
            label="validator builder seccomp profile",
            maximum=MAX_CLOSURE_DOCUMENT_BYTES,
        )
        seccomp_metadata = seccomp_profile.lstat()
        seccomp_identity = (
            seccomp_metadata.st_dev,
            seccomp_metadata.st_ino,
            seccomp_metadata.st_size,
            seccomp_metadata.st_mtime_ns,
            seccomp_metadata.st_ctime_ns,
        )
        if (
            seccomp_executable
            or seccomp_size != len(_seccomp_profile_bytes())
            or seccomp_hash != hashlib.sha256(_seccomp_profile_bytes()).hexdigest()
        ):
            _fail("validator builder seccomp profile changed before use")
        seccomp_path = _mount_source(
            seccomp_profile,
            label="validator builder seccomp profile path",
        )
        maximum_archive_bytes = limits["max_total_bytes"] + 16 * 1024 * 1024
        memory_bytes = limits["max_total_bytes"] + 2 * 1024**3
        cpu_seconds = limits["timeout_seconds"] + 60
        arguments = (
            "run",
            "--pull=never",
            f"--name={name}",
            f"--cidfile={cidfile}",
            "--platform=linux/amd64",
            "--runtime=runc",
            "--network=none",
            "--ipc=none",
            "--cgroupns=private",
            "--log-driver=none",
            "--no-healthcheck",
            "--hostname=iroha-sccp-validator",
            "--read-only",
            "--cap-drop=ALL",
            "--security-opt=no-new-privileges",
            f"--security-opt=seccomp={seccomp_path}",
            "--pids-limit=512",
            f"--memory={memory_bytes}",
            f"--memory-swap={memory_bytes}",
            "--cpus=2.0",
            "--ulimit=nofile=1024:1024",
            f"--ulimit=cpu={cpu_seconds}:{cpu_seconds}",
            f"--ulimit=fsize={maximum_archive_bytes}:{maximum_archive_bytes}",
            "--user=65534:65534",
            (
                f"--tmpfs=/work:rw,nosuid,nodev,size={limits['max_total_bytes']},"
                "uid=65534,gid=65534,mode=0700"
            ),
            "--mount",
            source_mount,
            "--env=LANG=C",
            "--env=LC_ALL=C",
            "--env=TZ=UTC",
            f"--env=SOURCE_DATE_EPOCH={policy['source']['source_date_epoch']}",
            "--entrypoint",
            builder["python_path"],
            builder["image"],
            DRIVER_MOUNT,
            "--source-archive=/input/source.tar",
            "--output-directory=/work/output",
            f"--source-commit={policy['source']['commit']}",
            f"--source-archive-sha256={source_archive_sha256}",
            f"--source-date-epoch={policy['source']['source_date_epoch']}",
            f"--builder-image={builder['image']}",
            f"--policy-sha256={policy_sha256}",
            f"--driver-sha256={builder['driver_sha256']}",
            f"--python-path={builder['python_path']}",
            f"--cargo-path={builder['cargo_path']}",
            f"--rustc-path={builder['rustc_path']}",
            f"--linker-path={builder['linker_path']}",
            f"--cargo-home={builder['cargo_home_path']}",
            f"--max-inventory-files={limits['max_inventory_files']}",
            f"--max-file-bytes={limits['max_file_bytes']}",
            f"--max-total-bytes={limits['max_total_bytes']}",
            f"--max-log-bytes={limits['max_log_bytes']}",
        )
        timed_out = False
        overflow = [False, False]
        stream_errors: list[Exception] = []
        stderr = bytearray()
        archive_size = [0]
        process: subprocess.Popen[bytes] | None = None
        descriptor: int | None = None
        try:
            descriptor = os.open(
                archive_path,
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_CLOEXEC", 0)
                | getattr(os, "O_NOFOLLOW", 0),
                0o600,
            )
            process = subprocess.Popen(
                [os.fspath(docker), *arguments],
                cwd=ROOT,
                env=dict(docker_environment),
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                close_fds=True,
                start_new_session=True,
            )
            assert process.stdout is not None and process.stderr is not None

            def stop_cli() -> None:
                assert process is not None
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                except OSError as error:
                    stream_errors.append(error)

            def copy_archive() -> None:
                assert (
                    process is not None
                    and process.stdout is not None
                    and descriptor is not None
                )
                try:
                    while True:
                        chunk = process.stdout.read(1024 * 1024)
                        if not chunk:
                            return
                        archive_size[0] += len(chunk)
                        if archive_size[0] > maximum_archive_bytes:
                            overflow[0] = True
                            stop_cli()
                            return
                        view = memoryview(chunk)
                        while view:
                            written = os.write(descriptor, view)
                            if written <= 0:
                                raise OSError("archive write made no progress")
                            view = view[written:]
                except Exception as error:  # noqa: BLE001 - fail closed on stream faults
                    stream_errors.append(error)
                    stop_cli()

            def copy_stderr() -> None:
                assert process is not None and process.stderr is not None
                try:
                    while True:
                        chunk = process.stderr.read(64 * 1024)
                        if not chunk:
                            return
                        if len(stderr) + len(chunk) > limits["max_log_bytes"]:
                            overflow[1] = True
                            stop_cli()
                            return
                        stderr.extend(chunk)
                except Exception as error:  # noqa: BLE001 - fail closed on stream faults
                    stream_errors.append(error)
                    stop_cli()

            readers = (
                threading.Thread(target=copy_archive, daemon=True),
                threading.Thread(target=copy_stderr, daemon=True),
            )
            for reader in readers:
                reader.start()
            try:
                return_code = process.wait(timeout=limits["timeout_seconds"])
            except subprocess.TimeoutExpired:
                timed_out = True
                stop_cli()
                return_code = process.wait()
            finally:
                for reader in readers:
                    reader.join(timeout=5)
                if any(reader.is_alive() for reader in readers):
                    stop_cli()
                    for reader in readers:
                        reader.join(timeout=2)
            if descriptor is not None:
                os.fsync(descriptor)
        except OSError:
            _fail("network-disabled SCCP validator build could not run safely")
        finally:
            if descriptor is not None:
                os.close(descriptor)
            if process is not None and process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
            _remove_container_and_verify(docker, name, docker_environment)
        common.reject_secret_material(
            bytes(stderr), label="validator build diagnostics"
        )
        if (
            timed_out
            or any(overflow)
            or stream_errors
            or any(reader.is_alive() for reader in readers)
            or return_code != 0
            or archive_size[0] == 0
        ):
            _fail("network-disabled SCCP validator build failed its resource boundary")
        cid = common.read_direct_file(
            cidfile,
            label="validator builder container id",
            maximum=129,
        ).strip()
        if not re.fullmatch(rb"[0-9a-f]{64}", cid):
            _fail("validator builder container id is not canonical")
        if not _docker_container_absent(
            docker,
            cid.decode("ascii"),
            docker_environment,
        ):
            _fail("validator builder container id remains present after cleanup")
        _require_unchanged(
            seccomp_profile,
            seccomp_identity,
            label="validator builder seccomp profile",
        )
        final_seccomp_hash, final_seccomp_size, final_seccomp_executable = (
            _hash_direct_file(
                seccomp_profile,
                label="validator builder seccomp profile",
                maximum=MAX_CLOSURE_DOCUMENT_BYTES,
            )
        )
        if (
            final_seccomp_executable
            or final_seccomp_size != seccomp_size
            or final_seccomp_hash != seccomp_hash
        ):
            _fail("validator builder seccomp profile changed during use")
        _extract_output_archive(archive_path, output, policy=policy)
    _require_unchanged(
        source_archive,
        source_identity,
        label="container-mounted source archive",
    )
    final_source_hash, final_source_size, _ = _hash_direct_file(
        source_archive,
        label="container-mounted source archive",
        maximum=min(limits["max_total_bytes"], MAX_SOURCE_ARCHIVE_BYTES),
    )
    if (
        final_source_hash != source_archive_sha256
        or final_source_size != observed_source_size
    ):
        _fail("container-mounted source archive changed during the build")
    _require_unchanged(docker, docker_identity, label="pinned Docker executable")


def _hash_direct_file(
    path: Path,
    *,
    label: str,
    maximum: int,
    allow_empty: bool = False,
    secret_scan: bool = False,
    require_static_linux_amd64_elf: bool = False,
) -> tuple[str, int, bool]:
    try:
        before = path.lstat()
    except OSError:
        _fail(f"{label} is unavailable")
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or (before.st_size == 0 and not allow_empty)
        or before.st_size > maximum
        or before.st_mode & (stat.S_ISUID | stat.S_ISGID | stat.S_ISVTX)
    ):
        _fail(f"{label} is not one safe bounded direct file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    digest = hashlib.sha256()
    observed = 0
    overlap = b""
    scan_budget = (
        common._SecretScanBudget(
            max_variants=65_536,
            max_decoded_bytes=min(
                2 * 1024**3,
                maximum * 4 + 64 * 1024**2,
            ),
            max_decoded_tokens=65_536,
        )
        if secret_scan
        else None
    )
    try:
        descriptor = os.open(path, flags)
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns) != (
            before.st_dev,
            before.st_ino,
            before.st_size,
            before.st_ctime_ns,
        ):
            _fail(f"{label} changed while opening")
        remaining = opened.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail(f"{label} changed while hashing")
            observed += len(chunk)
            digest.update(chunk)
            if secret_scan:
                combined = overlap + chunk
                _reject_concrete_material(
                    combined,
                    label=label,
                    inspect_json_keys=False,
                    budget=scan_budget,
                )
                overlap = combined[-STREAM_SECRET_SCAN_OVERLAP_BYTES:]
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail(f"{label} changed while hashing")
        if require_static_linux_amd64_elf:
            _validate_static_linux_amd64_elf_descriptor(
                descriptor,
                size=opened.st_size,
                label=label,
            )
        after = os.fstat(descriptor)
    except OSError:
        _fail(f"{label} could not be read safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)
    if observed != before.st_size or (
        after.st_dev,
        after.st_ino,
        after.st_size,
        after.st_ctime_ns,
    ) != (opened.st_dev, opened.st_ino, opened.st_size, opened.st_ctime_ns):
        _fail(f"{label} changed while hashing")
    return digest.hexdigest(), observed, bool(before.st_mode & stat.S_IXUSR)


def _pread_exact(descriptor: int, size: int, offset: int, *, label: str) -> bytes:
    payload = bytearray()
    while len(payload) < size:
        chunk = os.pread(descriptor, size - len(payload), offset + len(payload))
        if not chunk:
            _fail(f"{label} has a truncated ELF structure")
        payload.extend(chunk)
    return bytes(payload)


def _validate_static_linux_amd64_elf_descriptor(
    descriptor: int,
    *,
    size: int,
    label: str,
) -> None:
    """Require an amd64 ELF with no interpreter or shared-library dependency."""

    if size < 64:
        _fail(f"{label} is not a complete Linux/amd64 ELF executable")
    try:
        header = struct.unpack(
            "<16sHHIQQQIHHHHHH",
            _pread_exact(descriptor, 64, 0, label=label),
        )
    except struct.error:
        _fail(f"{label} has a malformed ELF header")
    (
        identity,
        executable_type,
        machine,
        version,
        _entry,
        program_offset,
        _section_offset,
        _flags,
        header_size,
        program_entry_size,
        program_count,
        _section_entry_size,
        _section_count,
        _section_names,
    ) = header
    if (
        identity[:7] != b"\x7fELF\x02\x01\x01"
        or any(identity[9:])
        or executable_type not in (2, 3)
        or machine != 62
        or version != 1
        or header_size != 64
        or program_entry_size != 56
        or not 1 <= program_count <= 4096
        or program_offset < 64
        or program_offset > size
        or program_count > (size - program_offset) // program_entry_size
    ):
        _fail(f"{label} is not a canonical Linux/amd64 ELF executable")
    table_size = program_count * program_entry_size
    table = _pread_exact(descriptor, table_size, program_offset, label=label)
    saw_load = False
    for index in range(program_count):
        entry = struct.unpack_from("<IIQQQQQQ", table, index * program_entry_size)
        segment_type, segment_flags, segment_offset, _, _, file_size, _, _ = entry
        if segment_type == 1:
            saw_load = True
        if segment_type == 3:
            _fail(f"{label} depends on an ambient ELF interpreter")
        if segment_type == 0x6474E551 and segment_flags & 1:
            _fail(f"{label} requests an executable process stack")
        if segment_type != 2:
            continue
        if (
            file_size == 0
            or file_size > 16 * 1024 * 1024
            or file_size % 16
            or segment_offset > size
            or file_size > size - segment_offset
        ):
            _fail(f"{label} has a malformed ELF dynamic table")
        dynamic = _pread_exact(descriptor, file_size, segment_offset, label=label)
        terminated = False
        for offset in range(0, len(dynamic), 16):
            tag, _ = struct.unpack_from("<qQ", dynamic, offset)
            if tag == 0:
                terminated = True
                break
            if tag in (1, 15, 29):
                _fail(f"{label} depends on ambient shared-library runtime state")
        if not terminated:
            _fail(f"{label} has an unterminated ELF dynamic table")
    if not saw_load:
        _fail(f"{label} has no loadable ELF segment")


def _validate_inventory(
    value: Any,
    *,
    label: str,
    prefix: str,
    maximum_entries: int,
) -> list[dict[str, Any]]:
    if type(value) is not list or not value or len(value) > maximum_entries:
        _fail(f"{label} has an invalid final-V1 cardinality")
    normalized: list[dict[str, Any]] = []
    previous = ""
    for index, candidate in enumerate(value):
        entry = _object(
            candidate,
            label=f"{label}[{index}]",
            keys=("path", "sha256", "size_bytes", "executable"),
        )
        path = _string(entry["path"], label=f"{label}[{index}].path", maximum=1024)
        pure = PurePosixPath(path)
        if (
            pure.is_absolute()
            or str(pure) != path
            or not path.startswith(prefix + "/")
            or any(part in ("", ".", "..") for part in pure.parts)
            or path <= previous
        ):
            _fail(f"{label} paths must be contained, unique, and strictly sorted")
        previous = path
        digest = _hex32(entry["sha256"], label=f"{label}[{index}].sha256")
        size = entry["size_bytes"]
        if type(size) is not int or not 0 <= size <= 2 * 1024**3:
            _fail(f"{label}[{index}].size_bytes is outside its bound")
        if type(entry["executable"]) is not bool:
            _fail(f"{label}[{index}].executable must be boolean")
        normalized.append(
            {
                "path": path,
                "sha256": digest,
                "size_bytes": size,
                "executable": entry["executable"],
            }
        )
    return normalized


def _require_metadata_path(value: Any, *, roots: tuple[str, ...]) -> None:
    """Require one normalized Cargo path to live in an inventoried tree."""

    if type(value) is not str:
        _fail("Cargo metadata contains a malformed build-input path")
    for root in roots:
        prefix = root + "/"
        if not value.startswith(prefix):
            continue
        relative = value[len(prefix) :]
        pure = PurePosixPath(relative)
        if (
            relative
            and not pure.is_absolute()
            and str(pure) == relative
            and all(part not in ("", ".", "..") for part in pure.parts)
        ):
            return
    _fail("Cargo metadata references a build input outside inventoried closure trees")


def _validate_metadata_input_boundaries(metadata: Any) -> None:
    """Independently close Cargo paths over tracked source and vendor inputs."""

    if (
        type(metadata) is not dict
        or metadata.get("workspace_root") != "${SOURCE}"
        or metadata.get("target_directory") != "${TARGET}"
        or type(metadata.get("packages")) is not list
    ):
        _fail("Cargo metadata does not bind the isolated workspace and target roots")
    for package in metadata["packages"]:
        if type(package) is not dict:
            _fail("Cargo metadata contains a malformed package")
        source = package.get("source")
        if source is not None and type(source) is not str:
            _fail("Cargo metadata contains a malformed package source")
        package_roots = ("${SOURCE}",) if source is None else ("${VENDOR}",)
        _require_metadata_path(package.get("manifest_path"), roots=package_roots)
        for optional in ("license_file", "readme"):
            path = package.get(optional)
            if path is not None:
                _require_metadata_path(path, roots=package_roots)
        targets = package.get("targets")
        dependencies = package.get("dependencies")
        if type(targets) is not list or not targets or type(dependencies) is not list:
            _fail("Cargo metadata omits package target or dependency paths")
        for target in targets:
            if type(target) is not dict:
                _fail("Cargo metadata contains a malformed package target")
            _require_metadata_path(target.get("src_path"), roots=package_roots)
        for dependency in dependencies:
            if type(dependency) is not dict:
                _fail("Cargo metadata contains a malformed package dependency")
            path = dependency.get("path")
            if path is not None:
                _require_metadata_path(path, roots=("${SOURCE}", "${VENDOR}"))


def _derive_sbom(metadata: Any) -> dict[str, Any]:
    _validate_metadata_input_boundaries(metadata)
    if type(metadata) is not dict:
        _fail("Cargo metadata cannot derive the validator SBOM")
    packages = metadata.get("packages")
    resolve = metadata.get("resolve")
    if (
        type(packages) is not list
        or type(resolve) is not dict
        or type(resolve.get("nodes")) is not list
    ):
        _fail("Cargo metadata omits the resolved validator package graph")
    dependency_map: dict[str, list[str]] = {}
    for node in resolve["nodes"]:
        if (
            type(node) is not dict
            or type(node.get("id")) is not str
            or type(node.get("dependencies")) is not list
            or any(type(item) is not str for item in node.get("dependencies", ()))
            or node["id"] in dependency_map
        ):
            _fail("Cargo metadata contains a malformed resolved dependency node")
        dependency_map[node["id"]] = sorted(node["dependencies"])
    entries: list[dict[str, Any]] = []
    roots: list[str] = []
    for package in packages:
        if type(package) is not dict or package.get("id") not in dependency_map:
            continue
        if any(
            type(package.get(field)) is not str or not package[field]
            for field in ("id", "name", "version", "manifest_path")
        ):
            _fail("Cargo metadata package has an incomplete SBOM identity")
        if any(
            package.get(field) is not None and type(package.get(field)) is not str
            for field in ("source", "license", "license_file")
        ):
            _fail("Cargo metadata package has a malformed SBOM field")
        entries.append(
            {
                "id": package["id"],
                "name": package["name"],
                "version": package["version"],
                "source": package.get("source"),
                "license": package.get("license"),
                "license_file": package.get("license_file"),
                "manifest_path": package["manifest_path"],
                "dependency_ids": dependency_map[package["id"]],
            }
        )
        if package["name"] == CRATE and package.get("source") is None:
            roots.append(package["id"])
    entries.sort(key=lambda entry: entry["id"])
    if not entries or len(roots) != 1:
        _fail("Cargo metadata does not identify one exact validator root package")
    return {
        "schema": SBOM_SCHEMA,
        "target_triple": TARGET,
        "root_package_id": roots[0],
        "binary": BINARY,
        "enabled_features": list(FEATURES),
        "packages": entries,
    }


def _validate_driver_output(
    output: Path,
    *,
    policy: Mapping[str, Any],
    policy_sha256: str,
    source_archive_sha256: str,
    source_archive_size: int,
    candidate: bool = False,
    release: bool = False,
) -> tuple[dict[str, Any], bytes, dict[str, bytes]]:
    try:
        top_level = sorted(entry.name for entry in os.scandir(output))
    except OSError:
        _fail("validator builder output could not be enumerated")
    if candidate and release:
        _fail("validator builder output cannot be both a candidate and a release")
    expected_top_level = ["builder-report.json", "closure", "validator"]
    if release:
        expected_top_level.extend(
            [
                "rebuilds",
                "source.tar",
                "validator-build-receipt.json",
                "validator-builder-policy.json",
                "validator-output-lock.json",
            ]
        )
        expected_top_level.sort()
    elif candidate:
        expected_top_level.extend(
            [
                "rebuild-signing-payload.bin",
                "source.tar",
                "unsigned-rebuild-attestation.json",
            ]
        )
        expected_top_level.sort()
    if top_level != expected_top_level:
        _fail("validator builder output has an unexpected top-level shape")
    closure = output / "closure"
    validator = output / "validator"
    if (
        closure.is_symlink()
        or validator.is_symlink()
        or not closure.is_dir()
        or not validator.is_dir()
    ):
        _fail("validator builder output contains an unsafe directory")
    if sorted(entry.name for entry in os.scandir(closure)) != list(
        EXPECTED_CLOSURE_FILES
    ):
        _fail("validator builder closure has an inexact file inventory")
    if sorted(entry.name for entry in os.scandir(validator)) != [BINARY]:
        _fail("validator builder emitted an inexact executable inventory")

    report, report_bytes = _load_json(
        output / "builder-report.json",
        label="validator builder report",
        maximum=MAX_REPORT_BYTES,
    )
    report = _object(
        report,
        label="validator builder report",
        keys=(
            "schema",
            "policy_sha256",
            "source_commit",
            "source_archive_sha256",
            "source_archive_size_bytes",
            "builder_image",
            "platform",
            "target_triple",
            "crate",
            "binary",
            "build_profile",
            "enabled_features",
            "build_jobs",
            "default_features",
            "cargo_locked",
            "cargo_frozen",
            "cargo_offline",
            "network_disabled",
            "dependency_inventory_sha256",
            "dependency_inventory_size_bytes",
            "cargo_metadata_closure_sha256",
            "cargo_metadata_closure_size_bytes",
            "sbom_sha256",
            "sbom_size_bytes",
            "toolchain_inventory_sha256",
            "toolchain_inventory_size_bytes",
            "sysroot_inventory_sha256",
            "sysroot_inventory_size_bytes",
            "linker_sha256",
            "build_recipe_sha256",
            "build_recipe_size_bytes",
            "build_environment_sha256",
            "build_environment_size_bytes",
            "executable_path",
            "executable_sha256",
            "executable_size_bytes",
        ),
    )
    if (
        report["schema"] != DRIVER_REPORT_SCHEMA
        or report["policy_sha256"] != policy_sha256
        or report["source_commit"] != policy["source"]["commit"]
        or report["source_archive_sha256"] != source_archive_sha256
        or report["source_archive_size_bytes"] != source_archive_size
        or report["builder_image"] != policy["builder"]["image"]
        or report["platform"] != PLATFORM
        or report["target_triple"] != TARGET
        or report["crate"] != CRATE
        or report["binary"] != BINARY
        or report["build_profile"] != "release"
        or report["enabled_features"] != list(FEATURES)
        or report["build_jobs"] != 1
        or report["default_features"] is not False
        or report["cargo_locked"] is not True
        or report["cargo_frozen"] is not True
        or report["cargo_offline"] is not True
        or report["network_disabled"] is not True
        or report["executable_path"] != f"validator/{BINARY}"
    ):
        _fail("validator builder report does not prove the exact production recipe")

    expectation = policy["builder"]["closure_expectations"]
    closure_payloads: dict[str, bytes] = {}
    closure_fields = {
        "dependency-inventory.json": (
            "dependency_inventory_sha256",
            "dependency_inventory_size_bytes",
        ),
        "cargo-metadata-closure.json": (
            "cargo_metadata_closure_sha256",
            "cargo_metadata_closure_size_bytes",
        ),
        "sbom.json": ("sbom_sha256", "sbom_size_bytes"),
        "toolchain-inventory.json": (
            "toolchain_inventory_sha256",
            "toolchain_inventory_size_bytes",
        ),
        "sysroot-inventory.json": (
            "sysroot_inventory_sha256",
            "sysroot_inventory_size_bytes",
        ),
        "build-recipe.json": ("build_recipe_sha256", "build_recipe_size_bytes"),
        "build-environment.json": (
            "build_environment_sha256",
            "build_environment_size_bytes",
        ),
    }
    for name, (hash_field, size_field) in closure_fields.items():
        path = closure / name
        payload = common.read_direct_file(
            path, label=f"validator closure {name}", maximum=MAX_CLOSURE_DOCUMENT_BYTES
        )
        _reject_concrete_material(
            payload,
            label=f"validator closure {name}",
            inspect_json_keys=True,
        )
        digest = hashlib.sha256(payload).hexdigest()
        if digest != report[hash_field] or len(payload) != report[size_field]:
            _fail("validator closure document does not match its builder report")
        if digest != expectation[hash_field]:
            _fail(
                "validator closure document differs from the approved immutable policy"
            )
        closure_payloads[name] = payload

    cargo_config = common.read_direct_file(
        closure / "cargo-config.toml",
        label="validator closure cargo-config.toml",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    _reject_concrete_material(
        cargo_config,
        label="validator closure cargo-config.toml",
        inspect_json_keys=False,
    )
    closure_payloads["cargo-config.toml"] = cargo_config

    dependency_value = common.parse_json_bytes(
        closure_payloads["dependency-inventory.json"],
        label="dependency inventory",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["dependency-inventory.json"],
        dependency_value,
        label="dependency inventory",
    )
    _validate_inventory(
        dependency_value,
        label="dependency inventory",
        prefix="vendor",
        maximum_entries=policy["limits"]["max_inventory_files"],
    )
    sysroot_value = common.parse_json_bytes(
        closure_payloads["sysroot-inventory.json"],
        label="sysroot inventory",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["sysroot-inventory.json"],
        sysroot_value,
        label="sysroot inventory",
    )
    _validate_inventory(
        sysroot_value,
        label="sysroot inventory",
        prefix="sysroot",
        maximum_entries=policy["limits"]["max_inventory_files"],
    )
    metadata_value = common.parse_json_bytes(
        closure_payloads["cargo-metadata-closure.json"],
        label="Cargo metadata closure",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["cargo-metadata-closure.json"],
        metadata_value,
        label="Cargo metadata closure",
    )
    if (
        type(metadata_value) is not dict
        or type(metadata_value.get("packages")) is not list
    ):
        _fail("Cargo metadata closure omits its package inventory")

    sbom_value = common.parse_json_bytes(
        closure_payloads["sbom.json"],
        label="validator SBOM",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["sbom.json"], sbom_value, label="validator SBOM"
    )
    sbom = _object(
        sbom_value,
        label="validator SBOM",
        keys=(
            "schema",
            "target_triple",
            "root_package_id",
            "binary",
            "enabled_features",
            "packages",
        ),
    )
    if (
        sbom["schema"] != SBOM_SCHEMA
        or sbom["target_triple"] != TARGET
        or sbom["binary"] != BINARY
        or sbom["enabled_features"] != list(FEATURES)
        or type(sbom["root_package_id"]) is not str
        or not sbom["root_package_id"]
        or type(sbom["packages"]) is not list
        or not sbom["packages"]
    ):
        _fail("validator SBOM does not bind the exact final-V1 package closure")
    if sbom != _derive_sbom(metadata_value):
        _fail("validator SBOM differs from the independently derived Cargo graph")

    toolchain_value = common.parse_json_bytes(
        closure_payloads["toolchain-inventory.json"],
        label="toolchain inventory",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["toolchain-inventory.json"],
        toolchain_value,
        label="toolchain inventory",
    )
    toolchain = _object(
        toolchain_value,
        label="toolchain inventory",
        keys=(
            "python_version",
            "cargo_version",
            "rustc_version",
            "linker_version",
            "sysroot",
            "tools",
        ),
    )
    builder = policy["builder"]
    if (
        toolchain["python_version"] != builder["python_reported_version"]
        or toolchain["cargo_version"] != builder["cargo_reported_version"]
        or toolchain["rustc_version"] != builder["rustc_reported_version"]
        or toolchain["linker_version"] != builder["linker_reported_version"]
        or toolchain["sysroot"] != "${SYSROOT}"
        or type(toolchain["tools"]) is not list
        or len(toolchain["tools"]) != 5
    ):
        _fail("validator toolchain inventory differs from the approved exact toolchain")
    expected_paths = {
        "builder-driver": DRIVER_MOUNT,
        "container-python": builder["python_path"],
        "cargo": builder["cargo_path"],
        "rustc": builder["rustc_path"],
        "linker": builder["linker_path"],
    }
    seen_roles: set[str] = set()
    for tool_candidate in toolchain["tools"]:
        entry = _object(
            tool_candidate,
            label="toolchain entry",
            keys=("role", "path", "sha256", "size_bytes", "executable"),
        )
        role = entry["role"]
        if (
            role not in expected_paths
            or role in seen_roles
            or entry["path"] != expected_paths[role]
        ):
            _fail("validator toolchain inventory has a substituted role or path")
        seen_roles.add(role)
        digest = _hex32(entry["sha256"], label=f"toolchain {role} hash")
        if role == "builder-driver" and digest != builder["driver_sha256"]:
            _fail("validator toolchain inventory uses a different builder driver")
        if role == "linker" and digest != expectation["linker_sha256"]:
            _fail("validator toolchain inventory uses a different linker")
        if (
            type(entry["size_bytes"]) is not int
            or entry["size_bytes"] <= 0
            or entry["executable"] is not True
        ):
            _fail("validator toolchain entry is not one bounded executable")
    if seen_roles != set(expected_paths):
        _fail("validator toolchain inventory omits a required role")
    if report["linker_sha256"] != expectation["linker_sha256"]:
        _fail("validator builder report does not bind the approved linker")

    recipe = common.parse_json_bytes(
        closure_payloads["build-recipe.json"],
        label="validator build recipe",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["build-recipe.json"],
        recipe,
        label="validator build recipe",
    )
    recipe = _object(
        recipe,
        label="validator build recipe",
        keys=(
            "program",
            "arguments",
            "working_directory",
            "cargo_vendor_arguments",
            "cargo_metadata_arguments",
            "cargo_config_sha256",
            "source_cargo_config_sha256",
            "driver_sha256",
        ),
    )
    exact_build_arguments = [
        "build",
        "--release",
        "--locked",
        "--frozen",
        "--offline",
        "--no-default-features",
        "--features",
        "dev-tools",
        "-p",
        CRATE,
        "--bin",
        BINARY,
        "--jobs",
        "1",
        "--target",
        TARGET,
    ]
    if (
        recipe["program"] != builder["cargo_path"]
        or recipe["arguments"] != exact_build_arguments
        or recipe["working_directory"] != "${SOURCE}"
        or recipe["cargo_vendor_arguments"]
        != ["vendor", "--locked", "--offline", "--versioned-dirs", "${VENDOR}"]
        or recipe["driver_sha256"] != builder["driver_sha256"]
        or not HEX32_RE.fullmatch(str(recipe["cargo_config_sha256"]))
        or recipe["cargo_config_sha256"]
        != hashlib.sha256(closure_payloads["cargo-config.toml"]).hexdigest()
        or recipe["source_cargo_config_sha256"] != APPROVED_SOURCE_CARGO_CONFIG_SHA256
    ):
        _fail("validator build recipe is not the sole final-V1 Cargo invocation")
    metadata_arguments = recipe["cargo_metadata_arguments"]
    if type(metadata_arguments) is not list or metadata_arguments != [
        "metadata",
        "--locked",
        "--offline",
        "--format-version=1",
        "--filter-platform",
        TARGET,
        "--no-default-features",
        "--features",
        f"{CRATE}/dev-tools",
    ]:
        _fail(
            "validator metadata closure was not selected with exact production features"
        )

    environment = common.parse_json_bytes(
        closure_payloads["build-environment.json"],
        label="validator build environment",
        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
    )
    common.require_canonical_json_file(
        closure_payloads["build-environment.json"],
        environment,
        label="validator build environment",
    )
    expected_environment = {
        "HOME": "/work/build/home",
        "CARGO_HOME": "/work/build/cargo-home",
        "CARGO_TARGET_DIR": "/work/build/target",
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "CARGO_TERM_COLOR": "never",
        "CARGO_BUILD_JOBS": "1",
        "TMPDIR": "/work/build/target/tmp",
        "LANG": "C",
        "LC_ALL": "C",
        "TZ": "UTC",
        "SOURCE_DATE_EPOCH": str(policy["source"]["source_date_epoch"]),
        "RUST_BACKTRACE": "0",
        "RUSTC": builder["rustc_path"],
        "RUSTFLAGS": (
            "--remap-path-prefix=/work/source=. "
            "--remap-path-prefix=/work/vendor=vendor "
            "--remap-path-prefix=/work/build/target=target "
            "-C target-feature=+crt-static "
            "-C strip=symbols"
        ),
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": builder["linker_path"],
        "PATH": "/usr/local/bin:/usr/bin:/bin",
    }
    if environment != expected_environment:
        _fail("validator build environment is not the fixed final-V1 environment")

    executable_path = output / "validator" / BINARY
    executable_hash, executable_size, executable = _hash_direct_file(
        executable_path,
        label="SCCP release validator executable",
        maximum=policy["limits"]["max_file_bytes"],
        secret_scan=True,
        require_static_linux_amd64_elf=True,
    )
    if (
        not executable
        or executable_hash != report["executable_sha256"]
        or executable_size != report["executable_size_bytes"]
    ):
        _fail("SCCP validator executable does not match its builder report")
    report_hash = hashlib.sha256(report_bytes).hexdigest()
    return (
        report,
        report_bytes,
        {
            **closure_payloads,
            "builder-report.json": report_bytes,
            "builder-report.sha256": report_hash.encode("ascii"),
        },
    )


def _unsigned_rebuild(
    *,
    role: str,
    nonce_hex: str,
    built_at_unix_ms: int,
    policy: Mapping[str, Any],
    policy_sha256: str,
    report: Mapping[str, Any],
    report_sha256: str,
) -> dict[str, Any]:
    if role not in ROLES or not HEX32_RE.fullmatch(nonce_hex) or nonce_hex == "00" * 32:
        _fail("validator rebuild role or nonce is invalid")
    if (
        type(built_at_unix_ms) is not int
        or not 1 <= built_at_unix_ms <= 4_102_444_800_000
    ):
        _fail("validator rebuild completion time is invalid")
    return {
        "schema": REBUILD_SCHEMA,
        "role": role,
        "signer_id": policy["approvers"][ROLES.index(role)]["signer_id"],
        "rebuild_nonce_hex": nonce_hex,
        "built_at_unix_ms": built_at_unix_ms,
        "builder_policy_sha256": policy_sha256,
        "source_commit": report["source_commit"],
        "source_archive_sha256": report["source_archive_sha256"],
        "source_archive_size_bytes": report["source_archive_size_bytes"],
        "builder_image": report["builder_image"],
        "builder_report_sha256": report_sha256,
        "dependency_inventory_sha256": report["dependency_inventory_sha256"],
        "cargo_metadata_closure_sha256": report["cargo_metadata_closure_sha256"],
        "sbom_sha256": report["sbom_sha256"],
        "toolchain_inventory_sha256": report["toolchain_inventory_sha256"],
        "sysroot_inventory_sha256": report["sysroot_inventory_sha256"],
        "linker_sha256": report["linker_sha256"],
        "build_recipe_sha256": report["build_recipe_sha256"],
        "build_environment_sha256": report["build_environment_sha256"],
        "executable_sha256": report["executable_sha256"],
        "executable_size_bytes": report["executable_size_bytes"],
    }


def _validate_unsigned_rebuild(
    value: Any,
    *,
    role: str,
    policy: Mapping[str, Any],
    policy_sha256: str,
    report: Mapping[str, Any],
    report_sha256: str,
) -> dict[str, Any]:
    expected_shape = _unsigned_rebuild(
        role=role,
        nonce_hex="01" * 32,
        built_at_unix_ms=1,
        policy=policy,
        policy_sha256=policy_sha256,
        report=report,
        report_sha256=report_sha256,
    )
    candidate = _object(
        value,
        label="unsigned validator rebuild",
        keys=tuple(expected_shape),
    )
    nonce = _hex32(candidate["rebuild_nonce_hex"], label="rebuild nonce")
    built_at_unix_ms = candidate["built_at_unix_ms"]
    expected = _unsigned_rebuild(
        role=role,
        nonce_hex=nonce,
        built_at_unix_ms=built_at_unix_ms,
        policy=policy,
        policy_sha256=policy_sha256,
        report=report,
        report_sha256=report_sha256,
    )
    if candidate != expected:
        _fail(
            "validator rebuild attestation does not bind its exact reproduced closure"
        )
    return expected


def _ensure_outside_repository(path: Path, *, label: str) -> None:
    try:
        absolute = path.parent.resolve(strict=True) / path.name
    except OSError:
        _fail(f"{label} parent is unavailable")
    try:
        absolute.relative_to(ROOT.resolve())
    except ValueError:
        return
    _fail(f"{label} must be outside the repository source tree")


def _open_parent_directory(path: Path, *, label: str) -> int:
    _ensure_outside_repository(path, label=label)
    if not SEGMENT_RE.fullmatch(path.name):
        _fail(f"{label} name is not canonical")
    absolute_parent = path.parent.absolute()
    if not absolute_parent.is_absolute():
        _fail(f"{label} parent is not absolute")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(os.sep, flags)
        for component in absolute_parent.parts[1:]:
            child = os.open(component, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
        metadata = os.fstat(descriptor)
    except OSError:
        if "descriptor" in locals():
            os.close(descriptor)
        _fail(f"{label} parent must be a direct directory tree")
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o022
    ):
        os.close(descriptor)
        _fail(f"{label} parent must be owner-controlled")
    return descriptor


def _new_private_output(path: Path, *, label: str) -> int:
    parent = _open_parent_directory(path, label=label)
    try:
        os.mkdir(path.name, mode=0o700, dir_fd=parent)
        descriptor = os.open(
            path.name,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent,
        )
        os.fchmod(descriptor, 0o700)
        os.fsync(parent)
    except FileExistsError:
        os.close(parent)
        _fail(f"{label} already exists; publication never overwrites")
    except OSError:
        os.close(parent)
        _fail(f"{label} could not be created safely")
    os.close(parent)
    metadata = os.fstat(descriptor)
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
    ):
        os.close(descriptor)
        _fail(f"{label} is not one owner-only direct directory")
    return descriptor


def _verify_published_output(path: Path, descriptor: int, *, label: str) -> None:
    """Bind the final advertised pathname to the directory built via ``descriptor``."""

    parent = _open_parent_directory(path, label=label)
    try:
        opened = os.fstat(descriptor)
        named = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
        if (
            not stat.S_ISDIR(opened.st_mode)
            or not stat.S_ISDIR(named.st_mode)
            or (opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino)
            or opened.st_uid != os.geteuid()
            or opened.st_mode & 0o077
        ):
            _fail(f"{label} advertised pathname changed during publication")
        os.fsync(descriptor)
        os.fsync(parent)
    except OSError:
        _fail(f"{label} advertised pathname could not be authenticated")
    finally:
        os.close(parent)


def _mkdir_at(parent: int, name: str) -> int:
    if not SEGMENT_RE.fullmatch(name):
        _fail("validator publication contains an invalid directory name")
    try:
        os.mkdir(name, mode=0o700, dir_fd=parent)
        descriptor = os.open(
            name,
            os.O_RDONLY
            | getattr(os, "O_DIRECTORY", 0)
            | getattr(os, "O_CLOEXEC", 0)
            | getattr(os, "O_NOFOLLOW", 0),
            dir_fd=parent,
        )
        os.fchmod(descriptor, 0o700)
        os.fsync(parent)
        metadata = os.fstat(descriptor)
        named = os.stat(name, dir_fd=parent, follow_symlinks=False)
    except OSError:
        if "descriptor" in locals():
            os.close(descriptor)
        _fail("validator publication could not create a direct output directory")
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
        or (metadata.st_dev, metadata.st_ino) != (named.st_dev, named.st_ino)
    ):
        os.close(descriptor)
        _fail("validator publication output directory inode changed")
    return descriptor


def _write_at(
    parent: int, name: str, payload: bytes, *, executable: bool = False
) -> None:
    if not SEGMENT_RE.fullmatch(name) or not payload:
        _fail("validator publication contains an invalid or empty file")
    mode = 0o700 if executable else 0o600
    flags = (
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(name, flags, mode, dir_fd=parent)
        opened = os.fstat(descriptor)
        identity = (opened.st_dev, opened.st_ino)
        view = memoryview(payload)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail("validator publication write made no progress")
            view = view[written:]
        os.fsync(descriptor)
        os.fchmod(descriptor, mode)
        named = os.stat(name, dir_fd=parent, follow_symlinks=False)
        current = os.fstat(descriptor)
        if (
            (named.st_dev, named.st_ino) != identity
            or (current.st_dev, current.st_ino) != identity
            or current.st_nlink != 1
            or current.st_size != len(payload)
            or current.st_mode & 0o077
        ):
            _fail("validator publication output inode changed")
        os.lseek(descriptor, 0, os.SEEK_SET)
        digest = hashlib.sha256()
        observed = 0
        remaining = len(payload)
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("validator publication failed inode readback")
            observed += len(chunk)
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            _fail("validator publication failed inode readback")
        if (
            observed != len(payload)
            or digest.digest() != hashlib.sha256(payload).digest()
        ):
            _fail("validator publication failed inode readback")
    except OSError:
        _fail("validator publication could not create an output file safely")
    finally:
        if "descriptor" in locals():
            os.close(descriptor)


def _copy_at(
    parent: int,
    name: str,
    source: Path,
    *,
    expected_sha256: str,
    maximum: int,
    executable: bool,
) -> None:
    digest, size, source_executable = _hash_direct_file(
        source,
        label="validator publication source",
        maximum=maximum,
    )
    if digest != expected_sha256 or (executable and not source_executable):
        _fail("validator publication source differs from its authenticated inventory")
    mode = 0o700 if executable else 0o600
    flags = (
        os.O_RDWR
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        source_descriptor = os.open(
            source,
            os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
        )
        source_opened = os.fstat(source_descriptor)
        source_identity = (
            source_opened.st_dev,
            source_opened.st_ino,
            source_opened.st_size,
            source_opened.st_ctime_ns,
        )
        if (
            not stat.S_ISREG(source_opened.st_mode)
            or source_opened.st_nlink != 1
            or source_opened.st_size != size
        ):
            _fail("validator publication source changed while opening")
        destination_descriptor = os.open(name, flags, mode, dir_fd=parent)
        copied = hashlib.sha256()
        observed = 0
        remaining = size
        while remaining:
            chunk = os.read(source_descriptor, min(1024 * 1024, remaining + 1))
            if not chunk or len(chunk) > remaining:
                _fail("validator publication source grew during copy")
            observed += len(chunk)
            copied.update(chunk)
            view = memoryview(chunk)
            while view:
                written = os.write(destination_descriptor, view)
                if written <= 0:
                    _fail("validator publication copy made no progress")
                view = view[written:]
            remaining -= len(chunk)
        if os.read(source_descriptor, 1):
            _fail("validator publication source grew during copy")
        os.fsync(destination_descriptor)
        os.fchmod(destination_descriptor, mode)
        source_after = os.fstat(source_descriptor)
        opened = os.fstat(destination_descriptor)
        named = os.stat(name, dir_fd=parent, follow_symlinks=False)
        if (
            observed != size
            or copied.hexdigest() != expected_sha256
            or source_identity
            != (
                source_after.st_dev,
                source_after.st_ino,
                source_after.st_size,
                source_after.st_ctime_ns,
            )
            or not stat.S_ISREG(opened.st_mode)
            or opened.st_size != size
            or opened.st_nlink != 1
            or (opened.st_dev, opened.st_ino) != (named.st_dev, named.st_ino)
            or opened.st_mode & 0o077
        ):
            _fail("validator publication copy failed authenticated readback")
    except OSError:
        _fail("validator publication could not copy an authenticated file")
    finally:
        if "source_descriptor" in locals():
            os.close(source_descriptor)
        if "destination_descriptor" in locals():
            os.close(destination_descriptor)


def _publish_candidate(
    destination: Path,
    *,
    build_output: Path,
    source_archive: Path,
    report: Mapping[str, Any],
    report_bytes: bytes,
    unsigned: Mapping[str, Any],
    policy: Mapping[str, Any],
) -> None:
    root = _new_private_output(destination, label="validator rebuild candidate")
    try:
        closure = _mkdir_at(root, "closure")
        try:
            for name in EXPECTED_CLOSURE_FILES:
                expected = hashlib.sha256(
                    common.read_direct_file(
                        build_output / "closure" / name,
                        label=f"validator closure {name}",
                        maximum=MAX_CLOSURE_DOCUMENT_BYTES,
                    )
                ).hexdigest()
                _copy_at(
                    closure,
                    name,
                    build_output / "closure" / name,
                    expected_sha256=expected,
                    maximum=MAX_CLOSURE_DOCUMENT_BYTES,
                    executable=False,
                )
        finally:
            os.fsync(closure)
            os.close(closure)
        validator = _mkdir_at(root, "validator")
        try:
            _copy_at(
                validator,
                BINARY,
                build_output / "validator" / BINARY,
                expected_sha256=report["executable_sha256"],
                maximum=policy["limits"]["max_file_bytes"],
                executable=True,
            )
        finally:
            os.fsync(validator)
            os.close(validator)
        _write_at(root, "builder-report.json", report_bytes)
        _copy_at(
            root,
            "source.tar",
            source_archive,
            expected_sha256=report["source_archive_sha256"],
            maximum=MAX_SOURCE_ARCHIVE_BYTES,
            executable=False,
        )
        unsigned_bytes = common.canonical_json_file_bytes(unsigned)
        _write_at(root, "unsigned-rebuild-attestation.json", unsigned_bytes)
        _write_at(
            root, "rebuild-signing-payload.bin", rebuild_signing_payload(unsigned)
        )
        os.fsync(root)
        _verify_published_output(
            destination,
            root,
            label="validator rebuild candidate",
        )
    finally:
        os.close(root)


def _load_policy(
    path: Path, *, trusted_policy_sha256: str
) -> tuple[dict[str, Any], bytes, str]:
    _ensure_outside_repository(path, label="validator builder policy")
    value, payload = _load_json(
        path, label="validator builder policy", maximum=MAX_POLICY_BYTES
    )
    policy = validate_policy(value)
    if common.canonical_json_file_bytes(policy) != payload:
        _fail("validator builder policy is not normalized final-V1 JSON")
    digest = hashlib.sha256(payload).hexdigest()
    if (
        _hex32(trusted_policy_sha256, label="trusted validator policy SHA-256")
        != digest
    ):
        _fail("validator builder policy does not match the externally trusted digest")
    return policy, payload, digest


def _require_private_directory(path: Path, *, label: str) -> None:
    _ensure_outside_repository(path, label=label)
    try:
        metadata = path.lstat()
    except OSError:
        _fail(f"{label} is unavailable")
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or metadata.st_mode & 0o077
    ):
        _fail(f"{label} must be one owner-only direct directory")


def _files_byte_identical(
    first: Path,
    second: Path,
    *,
    expected_sha256: str,
    maximum: int,
    label: str,
) -> None:
    """Require actual byte identity, not only matching reported digests."""

    identities: list[tuple[int, int, int, int, int]] = []
    descriptors: list[int] = []
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        for path in (first, second):
            before = path.lstat()
            if (
                stat.S_ISLNK(before.st_mode)
                or not stat.S_ISREG(before.st_mode)
                or before.st_nlink != 1
                or not 0 < before.st_size <= maximum
            ):
                _fail(f"{label} is not one bounded direct file in both rebuilds")
            descriptor = os.open(path, flags)
            opened = os.fstat(descriptor)
            identity = (
                before.st_dev,
                before.st_ino,
                before.st_size,
                before.st_mtime_ns,
                before.st_ctime_ns,
            )
            if identity != (
                opened.st_dev,
                opened.st_ino,
                opened.st_size,
                opened.st_mtime_ns,
                opened.st_ctime_ns,
            ):
                _fail(f"{label} changed while opening")
            descriptors.append(descriptor)
            identities.append(identity)
        if identities[0][2] != identities[1][2]:
            _fail(f"{label} differs between independent rebuilds")
        digest = hashlib.sha256()
        remaining = identities[0][2]
        while remaining:
            amount = min(1024 * 1024, remaining + 1)
            left = os.read(descriptors[0], amount)
            right = os.read(descriptors[1], amount)
            if left != right:
                _fail(f"{label} differs between independent rebuilds")
            if not left or len(left) > remaining:
                _fail(f"{label} changed during byte comparison")
            digest.update(left)
            remaining -= len(left)
        if os.read(descriptors[0], 1) or os.read(descriptors[1], 1):
            _fail(f"{label} changed during byte comparison")
        if digest.hexdigest() != expected_sha256:
            _fail(f"{label} differs from its authenticated digest")
        for descriptor, identity in zip(descriptors, identities):
            after = os.fstat(descriptor)
            if identity != (
                after.st_dev,
                after.st_ino,
                after.st_size,
                after.st_mtime_ns,
                after.st_ctime_ns,
            ):
                _fail(f"{label} changed during byte comparison")
    except OSError:
        _fail(f"{label} could not be compared safely")
    finally:
        for descriptor in descriptors:
            os.close(descriptor)


def _candidate_report_seed(candidate: Path) -> dict[str, Any]:
    value, _ = _load_json(
        candidate / "builder-report.json",
        label="validator candidate builder report",
        maximum=MAX_REPORT_BYTES,
    )
    for field in ("source_archive_sha256", "policy_sha256"):
        _hex32(value.get(field), label=f"builder report {field}")
    size = value.get("source_archive_size_bytes")
    if type(size) is not int or not 0 < size <= MAX_SOURCE_ARCHIVE_BYTES:
        _fail("validator builder report has an invalid source archive size")
    return value


def _load_candidate(
    candidate: Path,
    signed_rebuild_path: Path,
    *,
    role: str,
    policy: Mapping[str, Any],
    policy_sha256: str,
) -> dict[str, Any]:
    _require_private_directory(candidate, label=f"{role} validator rebuild candidate")
    seed = _candidate_report_seed(candidate)
    if seed["policy_sha256"] != policy_sha256:
        _fail("validator rebuild candidate binds a different builder policy")
    report, report_bytes, closure_payloads = _validate_driver_output(
        candidate,
        policy=policy,
        policy_sha256=policy_sha256,
        source_archive_sha256=seed["source_archive_sha256"],
        source_archive_size=seed["source_archive_size_bytes"],
        candidate=True,
    )
    report_sha256 = hashlib.sha256(report_bytes).hexdigest()
    archive_sha256, archive_size, archive_executable = _hash_direct_file(
        candidate / "source.tar",
        label=f"{role} tracked source archive",
        maximum=MAX_SOURCE_ARCHIVE_BYTES,
    )
    if (
        archive_executable
        or archive_sha256 != report["source_archive_sha256"]
        or archive_size != report["source_archive_size_bytes"]
    ):
        _fail("validator rebuild source archive differs from its builder report")

    unsigned_value, unsigned_bytes = _load_json(
        candidate / "unsigned-rebuild-attestation.json",
        label=f"{role} unsigned validator rebuild",
        maximum=MAX_SIGNED_REBUILD_BYTES,
    )
    unsigned = _validate_unsigned_rebuild(
        unsigned_value,
        role=role,
        policy=policy,
        policy_sha256=policy_sha256,
        report=report,
        report_sha256=report_sha256,
    )
    if common.canonical_json_file_bytes(unsigned) != unsigned_bytes:
        _fail("unsigned validator rebuild is not normalized final-V1 JSON")
    signing_payload = common.read_direct_file(
        candidate / "rebuild-signing-payload.bin",
        label=f"{role} validator rebuild signing payload",
        maximum=MAX_SIGNED_REBUILD_BYTES,
    )
    if signing_payload != rebuild_signing_payload(unsigned):
        _fail("validator rebuild signing payload differs from its attestation")

    _ensure_outside_repository(
        signed_rebuild_path, label=f"{role} signed validator rebuild"
    )
    signed_value, signed_bytes = _load_json(
        signed_rebuild_path,
        label=f"{role} signed validator rebuild",
        maximum=MAX_SIGNED_REBUILD_BYTES,
    )
    signed = _validate_signature(
        signed_value,
        unsigned=unsigned,
        policy=policy,
        role_index=ROLES.index(role),
    )
    if common.canonical_json_file_bytes(signed) != signed_bytes:
        _fail("signed validator rebuild is not normalized final-V1 JSON")
    return {
        "path": candidate,
        "report": report,
        "report_bytes": report_bytes,
        "report_sha256": report_sha256,
        "closure_payloads": closure_payloads,
        "unsigned": unsigned,
        "signed": signed,
        "signed_bytes": signed_bytes,
        "signed_sha256": hashlib.sha256(signed_bytes).hexdigest(),
    }


def _closure_identity(report: Mapping[str, Any]) -> dict[str, Any]:
    return {
        "source_commit": report["source_commit"],
        "builder_image": report["builder_image"],
        "platform": report["platform"],
        "target_triple": report["target_triple"],
        "crate": report["crate"],
        "binary": report["binary"],
        "source_archive_sha256": report["source_archive_sha256"],
        "source_archive_size_bytes": report["source_archive_size_bytes"],
        "dependency_inventory_sha256": report["dependency_inventory_sha256"],
        "cargo_metadata_closure_sha256": report["cargo_metadata_closure_sha256"],
        "sbom_sha256": report["sbom_sha256"],
        "toolchain_inventory_sha256": report["toolchain_inventory_sha256"],
        "sysroot_inventory_sha256": report["sysroot_inventory_sha256"],
        "linker_sha256": report["linker_sha256"],
        "build_recipe_sha256": report["build_recipe_sha256"],
        "build_environment_sha256": report["build_environment_sha256"],
        "executable_sha256": report["executable_sha256"],
        "executable_size_bytes": report["executable_size_bytes"],
    }


def _output_lock(
    *,
    policy_sha256: str,
    report: Mapping[str, Any],
    report_sha256: str,
    rebuilds: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    closure = _closure_identity(report)
    complete_closure_sha256 = hashlib.sha256(
        OUTPUT_LOCK_HASH_DOMAIN + common.canonical_json_bytes(closure)
    ).hexdigest()
    rebuild_entries = [
        {
            "role": role,
            "signer_id": rebuild["signed"]["signer_id"],
            "rebuild_nonce_hex": rebuild["signed"]["rebuild_nonce_hex"],
            "built_at_unix_ms": rebuild["signed"]["built_at_unix_ms"],
            "signed_rebuild_sha256": rebuild["signed_sha256"],
        }
        for role, rebuild in zip(ROLES, rebuilds)
    ]
    return {
        "schema": LOCK_SCHEMA,
        "builder_policy_sha256": policy_sha256,
        "source_commit": report["source_commit"],
        "source_archive_sha256": report["source_archive_sha256"],
        "source_archive_size_bytes": report["source_archive_size_bytes"],
        "builder_image": report["builder_image"],
        "container_manifest_sha256": _container_manifest_sha256(
            report["builder_image"]
        ),
        "builder_report_sha256": report_sha256,
        **{key: report[key] for key in EXPECTATION_FIELDS},
        "executable_sha256": report["executable_sha256"],
        "executable_size_bytes": report["executable_size_bytes"],
        "complete_build_closure_sha256": complete_closure_sha256,
        "rebuilds": rebuild_entries,
    }


def _receipt(
    *,
    lock: Mapping[str, Any],
    output_lock_sha256: str,
) -> dict[str, Any]:
    return {
        "schema": RECEIPT_SCHEMA,
        "source_commit": lock["source_commit"],
        "validator_built_at_unix_ms": min(
            rebuild["built_at_unix_ms"] for rebuild in lock["rebuilds"]
        ),
        "validator_builder_policy_sha256": lock["builder_policy_sha256"],
        "validator_source_archive_sha256": lock["source_archive_sha256"],
        "validator_source_archive_size_bytes": lock["source_archive_size_bytes"],
        "validator_dependency_inventory_sha256": lock["dependency_inventory_sha256"],
        "validator_cargo_metadata_closure_sha256": lock[
            "cargo_metadata_closure_sha256"
        ],
        "validator_sbom_sha256": lock["sbom_sha256"],
        "validator_toolchain_inventory_sha256": lock["toolchain_inventory_sha256"],
        "validator_sysroot_inventory_sha256": lock["sysroot_inventory_sha256"],
        "validator_linker_sha256": lock["linker_sha256"],
        "validator_build_recipe_sha256": lock["build_recipe_sha256"],
        "validator_build_environment_sha256": lock["build_environment_sha256"],
        "validator_container_manifest_sha256": lock["container_manifest_sha256"],
        "validator_builder_report_sha256": lock["builder_report_sha256"],
        "validator_executable_sha256": lock["executable_sha256"],
        "validator_executable_size_bytes": lock["executable_size_bytes"],
        "validator_complete_build_closure_sha256": lock[
            "complete_build_closure_sha256"
        ],
        "validator_output_lock_sha256": output_lock_sha256,
        "rebuilds": lock["rebuilds"],
    }


def _require_fresh_rebuilds(
    rebuilds: Sequence[Mapping[str, Any]],
    *,
    trusted_now_unix_ms: int,
) -> None:
    """Require both independently signed build completions inside seven days."""

    for rebuild in rebuilds:
        built_at = rebuild["signed"].get("built_at_unix_ms")
        if type(built_at) is not int or not 1 <= built_at <= 4_102_444_800_000:
            _fail("validator rebuild completion time is invalid")
        if built_at > trusted_now_unix_ms + MAX_FUTURE_SKEW_MS:
            _fail("validator rebuild completion time is future-dated")
        if trusted_now_unix_ms - built_at > MAX_VALIDATOR_BUILD_AGE_MS:
            _fail("validator rebuild signature is older than seven days")


def _validate_receipt_hash_roles(receipt: Mapping[str, Any]) -> None:
    values = [receipt.get(field) for field in RECEIPT_HASH_FIELDS]
    if (
        len(values) != 15
        or any(
            type(value) is not str
            or not HEX32_RE.fullmatch(value)
            or value == "00" * 32
            for value in values
        )
        or len(set(values)) != len(values)
    ):
        _fail(
            "validator build receipt hash roles are not complete and pairwise distinct"
        )


def _publish_release(
    destination: Path,
    *,
    candidate: Mapping[str, Any],
    policy_bytes: bytes,
    rebuilds: Sequence[Mapping[str, Any]],
    lock_bytes: bytes,
    receipt: Mapping[str, Any],
    policy: Mapping[str, Any],
) -> None:
    root = _new_private_output(destination, label="validator release output")
    candidate_path = candidate["path"]
    report = candidate["report"]
    try:
        closure = _mkdir_at(root, "closure")
        try:
            closure_hash_fields = {
                "build-environment.json": "build_environment_sha256",
                "build-recipe.json": "build_recipe_sha256",
                "cargo-metadata-closure.json": "cargo_metadata_closure_sha256",
                "dependency-inventory.json": "dependency_inventory_sha256",
                "sbom.json": "sbom_sha256",
                "sysroot-inventory.json": "sysroot_inventory_sha256",
                "toolchain-inventory.json": "toolchain_inventory_sha256",
            }
            for name in EXPECTED_CLOSURE_FILES:
                expected_sha256 = (
                    hashlib.sha256(candidate["closure_payloads"][name]).hexdigest()
                    if name == "cargo-config.toml"
                    else report[closure_hash_fields[name]]
                )
                _copy_at(
                    closure,
                    name,
                    candidate_path / "closure" / name,
                    expected_sha256=expected_sha256,
                    maximum=MAX_CLOSURE_DOCUMENT_BYTES,
                    executable=False,
                )
        finally:
            os.fsync(closure)
            os.close(closure)
        validator = _mkdir_at(root, "validator")
        try:
            _copy_at(
                validator,
                BINARY,
                candidate_path / "validator" / BINARY,
                expected_sha256=report["executable_sha256"],
                maximum=policy["limits"]["max_file_bytes"],
                executable=True,
            )
        finally:
            os.fsync(validator)
            os.close(validator)
        rebuild_directory = _mkdir_at(root, "rebuilds")
        try:
            for role, rebuild in zip(ROLES, rebuilds):
                _write_at(
                    rebuild_directory,
                    f"{role}.json",
                    rebuild["signed_bytes"],
                )
        finally:
            os.fsync(rebuild_directory)
            os.close(rebuild_directory)
        _copy_at(
            root,
            "source.tar",
            candidate_path / "source.tar",
            expected_sha256=report["source_archive_sha256"],
            maximum=MAX_SOURCE_ARCHIVE_BYTES,
            executable=False,
        )
        _write_at(root, "validator-builder-policy.json", policy_bytes)
        _write_at(root, "builder-report.json", candidate["report_bytes"])
        _write_at(root, "validator-output-lock.json", lock_bytes)
        receipt_bytes = common.canonical_json_file_bytes(receipt)
        if len(receipt_bytes) > MAX_RECEIPT_BYTES:
            _fail("validator build receipt exceeds its final-V1 bound")
        common.reject_secret_material(receipt_bytes, label="validator build receipt")
        # The receipt is the release manifest and is deliberately published last.
        _write_at(root, "validator-build-receipt.json", receipt_bytes)
        os.fsync(root)
        _verify_published_output(
            destination,
            root,
            label="validator release output",
        )
    finally:
        os.close(root)


def verify_release_directory(
    release_directory: Path,
    *,
    trusted_policy_sha256: str,
) -> dict[str, Any]:
    """Authenticate one published final-V1 validator release without mutating it."""

    release_directory = Path(release_directory)
    _require_private_directory(
        release_directory,
        label="published validator release",
    )
    policy, _, policy_sha256 = _load_policy(
        release_directory / "validator-builder-policy.json",
        trusted_policy_sha256=trusted_policy_sha256,
    )
    seed = _candidate_report_seed(release_directory)
    if seed["policy_sha256"] != policy_sha256:
        _fail("published validator release binds a different builder policy")
    report, report_bytes, _ = _validate_driver_output(
        release_directory,
        policy=policy,
        policy_sha256=policy_sha256,
        source_archive_sha256=seed["source_archive_sha256"],
        source_archive_size=seed["source_archive_size_bytes"],
        release=True,
    )
    report_sha256 = hashlib.sha256(report_bytes).hexdigest()
    archive_sha256, archive_size, archive_executable = _hash_direct_file(
        release_directory / "source.tar",
        label="published validator source archive",
        maximum=MAX_SOURCE_ARCHIVE_BYTES,
    )
    if (
        archive_executable
        or archive_sha256 != report["source_archive_sha256"]
        or archive_size != report["source_archive_size_bytes"]
    ):
        _fail("published validator source archive differs from its report")

    rebuild_directory = release_directory / "rebuilds"
    if (
        rebuild_directory.is_symlink()
        or not rebuild_directory.is_dir()
        or sorted(entry.name for entry in os.scandir(rebuild_directory))
        != [f"{role}.json" for role in ROLES]
    ):
        _fail("published validator release has an inexact rebuild inventory")
    rebuilds: list[dict[str, Any]] = []
    for index, role in enumerate(ROLES):
        value, payload = _load_json(
            rebuild_directory / f"{role}.json",
            label=f"published {role} validator rebuild",
            maximum=MAX_SIGNED_REBUILD_BYTES,
        )
        nonce = _hex32(value.get("rebuild_nonce_hex"), label="rebuild nonce")
        unsigned = _unsigned_rebuild(
            role=role,
            nonce_hex=nonce,
            built_at_unix_ms=value.get("built_at_unix_ms"),
            policy=policy,
            policy_sha256=policy_sha256,
            report=report,
            report_sha256=report_sha256,
        )
        signed = _validate_signature(
            value,
            unsigned=unsigned,
            policy=policy,
            role_index=index,
        )
        if common.canonical_json_file_bytes(signed) != payload:
            _fail("published validator rebuild is not normalized final-V1 JSON")
        rebuilds.append(
            {
                "signed": signed,
                "signed_bytes": payload,
                "signed_sha256": hashlib.sha256(payload).hexdigest(),
            }
        )
    if (
        rebuilds[0]["signed"]["rebuild_nonce_hex"]
        == rebuilds[1]["signed"]["rebuild_nonce_hex"]
        or rebuilds[0]["signed_sha256"] == rebuilds[1]["signed_sha256"]
        or rebuilds[0]["signed"]["provenance"]["signature_b64"]
        == rebuilds[1]["signed"]["provenance"]["signature_b64"]
    ):
        _fail("published validator rebuild evidence is not independent")

    lock_value, lock_bytes = _load_json(
        release_directory / "validator-output-lock.json",
        label="published validator output lock",
        maximum=MAX_LOCK_BYTES,
    )
    expected_lock = _output_lock(
        policy_sha256=policy_sha256,
        report=report,
        report_sha256=report_sha256,
        rebuilds=rebuilds,
    )
    if lock_value != expected_lock or lock_bytes != common.canonical_json_file_bytes(
        expected_lock
    ):
        _fail("published validator output lock does not authenticate the release")
    lock_sha256 = hashlib.sha256(lock_bytes).hexdigest()

    receipt_value, receipt_bytes = _load_json(
        release_directory / "validator-build-receipt.json",
        label="published validator build receipt",
        maximum=MAX_RECEIPT_BYTES,
    )
    expected_receipt = _receipt(
        lock=expected_lock,
        output_lock_sha256=lock_sha256,
    )
    if (
        receipt_value != expected_receipt
        or receipt_bytes != common.canonical_json_file_bytes(expected_receipt)
    ):
        _fail("published validator build receipt does not authenticate the output lock")
    _validate_receipt_hash_roles(expected_receipt)

    absolute_release = release_directory.resolve(strict=True)
    return {
        "schema": VERIFICATION_SCHEMA,
        "source_commit": report["source_commit"],
        "validator_built_at_unix_ms": expected_receipt["validator_built_at_unix_ms"],
        "validator_build_receipt_sha256": hashlib.sha256(receipt_bytes).hexdigest(),
        "validator_executable_path": os.fspath(absolute_release / "validator" / BINARY),
        "validator_executable_size_bytes": report["executable_size_bytes"],
        "hashes": {field: expected_receipt[field] for field in RECEIPT_HASH_FIELDS},
    }


def prepare_rebuild(arguments: argparse.Namespace) -> None:
    """Run one role-bound independent build and emit external-signing bytes."""

    policy, _, policy_sha256 = _load_policy(
        arguments.policy, trusted_policy_sha256=arguments.trusted_policy_sha256
    )
    role = arguments.role
    if role not in ROLES:
        _fail("validator rebuild role is not a final-V1 independent role")
    builder = policy["builder"]
    python, python_identity = _open_stable_executable(
        os.fspath(Path(sys.executable).resolve()),
        label="pinned host Python executable",
        expected_sha256=builder["host_python_sha256"],
    )
    git, git_identity = _open_stable_executable(
        arguments.git,
        label="pinned Git executable",
        expected_sha256=builder["host_git_sha256"],
    )
    docker, docker_identity = _open_stable_executable(
        arguments.docker,
        label="pinned Docker executable",
        expected_sha256=builder["host_docker_sha256"],
    )
    commit_verifier, commit_verifier_identity = _open_stable_executable(
        arguments.commit_verifier,
        label="pinned OpenPGP commit signature verifier",
        expected_sha256=builder["host_commit_verifier_sha256"],
    )
    with tempfile.TemporaryDirectory(
        prefix=f"iroha-sccp-validator-{role}-",
        dir=HOST_TEMP_ROOT,
    ) as temporary:
        temporary_root = Path(temporary)
        os.chmod(temporary_root, 0o700)
        staged_git, staged_git_identity = _stage_host_executable(
            git,
            git_identity,
            temporary_root / "git-tool" / "git",
            expected_sha256=builder["host_git_sha256"],
            label="pinned Git executable",
        )
        staged_docker, staged_docker_identity = _stage_host_executable(
            docker,
            docker_identity,
            temporary_root / "docker-tool" / "docker",
            expected_sha256=builder["host_docker_sha256"],
            label="pinned Docker executable",
        )
        staged_verifier, staged_verifier_identity = _stage_host_executable(
            commit_verifier,
            commit_verifier_identity,
            temporary_root / "commit-verifier-tool" / "verifier",
            expected_sha256=builder["host_commit_verifier_sha256"],
            label="pinned OpenPGP commit signature verifier",
        )
        docker_config = temporary_root / "docker-config"
        docker_config.mkdir(mode=0o700)
        docker_environment = _closed_environment(
            source_date_epoch=policy["source"]["source_date_epoch"],
            docker_config=docker_config,
        )
        _inspect_docker_daemon(
            staged_docker,
            staged_docker_identity,
            policy,
            docker_environment,
        )
        _inspect_image(
            staged_docker,
            staged_docker_identity,
            policy,
            docker_environment,
        )
        archive = temporary_root / "source.tar"
        source_sha256, source_size = _verify_source_and_archive(
            staged_git,
            staged_verifier,
            policy,
            archive,
        )
        if source_size > min(
            policy["limits"]["max_total_bytes"], MAX_SOURCE_ARCHIVE_BYTES
        ):
            _fail("tracked source archive exceeds the signed build limits")
        output = temporary_root / "build-output"
        _run_container_build(
            staged_docker,
            staged_docker_identity,
            policy,
            policy_sha256,
            archive,
            source_sha256,
            output,
            docker_environment,
        )
        report, report_bytes, _ = _validate_driver_output(
            output,
            policy=policy,
            policy_sha256=policy_sha256,
            source_archive_sha256=source_sha256,
            source_archive_size=source_size,
        )
        report_sha256 = hashlib.sha256(report_bytes).hexdigest()
        unsigned = _unsigned_rebuild(
            role=role,
            nonce_hex=secrets.token_hex(32),
            built_at_unix_ms=time.time_ns() // 1_000_000,
            policy=policy,
            policy_sha256=policy_sha256,
            report=report,
            report_sha256=report_sha256,
        )
        _publish_candidate(
            arguments.output_dir,
            build_output=output,
            source_archive=archive,
            report=report,
            report_bytes=report_bytes,
            unsigned=unsigned,
            policy=policy,
        )
        _require_unchanged(
            staged_git,
            staged_git_identity,
            label="staged pinned Git executable",
        )
        _require_unchanged(
            staged_docker,
            staged_docker_identity,
            label="staged pinned Docker executable",
        )
        _require_unchanged(
            staged_verifier,
            staged_verifier_identity,
            label="staged pinned OpenPGP commit signature verifier",
        )
    _require_unchanged(python, python_identity, label="pinned host Python executable")
    _require_unchanged(git, git_identity, label="pinned Git executable")
    _require_unchanged(docker, docker_identity, label="pinned Docker executable")
    _require_unchanged(
        commit_verifier,
        commit_verifier_identity,
        label="pinned OpenPGP commit signature verifier",
    )
    sys.stdout.buffer.write(common.canonical_json_file_bytes(unsigned))


def finalize_release(arguments: argparse.Namespace) -> None:
    """Authenticate two byte-identical independent rebuilds and publish final-V1."""

    policy, policy_bytes, policy_sha256 = _load_policy(
        arguments.policy, trusted_policy_sha256=arguments.trusted_policy_sha256
    )
    candidates = [
        _load_candidate(
            arguments.engineering_candidate,
            arguments.engineering_signed_rebuild,
            role=ROLES[0],
            policy=policy,
            policy_sha256=policy_sha256,
        ),
        _load_candidate(
            arguments.security_candidate,
            arguments.security_signed_rebuild,
            role=ROLES[1],
            policy=policy,
            policy_sha256=policy_sha256,
        ),
    ]
    first, second = candidates
    report = first["report"]
    if first["report_bytes"] != second["report_bytes"]:
        _fail("independent validator builder reports are not byte-identical")
    if (
        first["unsigned"]["rebuild_nonce_hex"]
        == second["unsigned"]["rebuild_nonce_hex"]
        or first["signed_sha256"] == second["signed_sha256"]
        or first["signed"]["provenance"]["signature_b64"]
        == second["signed"]["provenance"]["signature_b64"]
    ):
        _fail("validator rebuild evidence is not independent")
    _require_fresh_rebuilds(
        candidates,
        trusted_now_unix_ms=time.time_ns() // 1_000_000,
    )
    _files_byte_identical(
        first["path"] / "source.tar",
        second["path"] / "source.tar",
        expected_sha256=report["source_archive_sha256"],
        maximum=MAX_SOURCE_ARCHIVE_BYTES,
        label="tracked source archive",
    )
    closure_hash_fields = {
        "build-environment.json": "build_environment_sha256",
        "build-recipe.json": "build_recipe_sha256",
        "cargo-metadata-closure.json": "cargo_metadata_closure_sha256",
        "dependency-inventory.json": "dependency_inventory_sha256",
        "sbom.json": "sbom_sha256",
        "sysroot-inventory.json": "sysroot_inventory_sha256",
        "toolchain-inventory.json": "toolchain_inventory_sha256",
    }
    for name in EXPECTED_CLOSURE_FILES:
        expected_sha256 = (
            hashlib.sha256(first["closure_payloads"][name]).hexdigest()
            if name == "cargo-config.toml"
            else report[closure_hash_fields[name]]
        )
        _files_byte_identical(
            first["path"] / "closure" / name,
            second["path"] / "closure" / name,
            expected_sha256=expected_sha256,
            maximum=MAX_CLOSURE_DOCUMENT_BYTES,
            label=f"validator closure {name}",
        )
    _files_byte_identical(
        first["path"] / "validator" / BINARY,
        second["path"] / "validator" / BINARY,
        expected_sha256=report["executable_sha256"],
        maximum=policy["limits"]["max_file_bytes"],
        label="SCCP release validator executable",
    )
    lock = _output_lock(
        policy_sha256=policy_sha256,
        report=report,
        report_sha256=first["report_sha256"],
        rebuilds=candidates,
    )
    lock_bytes = common.canonical_json_file_bytes(lock)
    if len(lock_bytes) > MAX_LOCK_BYTES:
        _fail("validator output lock exceeds its final-V1 bound")
    common.reject_secret_material(lock_bytes, label="validator output lock")
    output_lock_sha256 = hashlib.sha256(lock_bytes).hexdigest()
    receipt = _receipt(lock=lock, output_lock_sha256=output_lock_sha256)
    _validate_receipt_hash_roles(receipt)
    _publish_release(
        arguments.output_dir,
        candidate=first,
        policy_bytes=policy_bytes,
        rebuilds=candidates,
        lock_bytes=lock_bytes,
        receipt=receipt,
        policy=policy,
    )
    sys.stdout.buffer.write(common.canonical_json_file_bytes(receipt))


def verify_release(arguments: argparse.Namespace) -> None:
    """Verify a published final-V1 bundle and emit its normalized identities."""

    verification = verify_release_directory(
        arguments.release_dir,
        trusted_policy_sha256=arguments.trusted_policy_sha256,
    )
    sys.stdout.buffer.write(common.canonical_json_file_bytes(verification))


def _parser() -> argparse.ArgumentParser:
    parser = _SafeArgumentParser(description=__doc__, allow_abbrev=False)
    modes = parser.add_subparsers(dest="mode", required=True)
    prepare = modes.add_parser("prepare", allow_abbrev=False)
    prepare.add_argument("--role", required=True, choices=ROLES)
    prepare.add_argument("--policy", required=True, type=Path)
    prepare.add_argument("--trusted-policy-sha256", required=True)
    prepare.add_argument("--git", required=True, help="absolute pinned Git executable")
    prepare.add_argument(
        "--docker", required=True, help="absolute pinned Docker executable"
    )
    prepare.add_argument(
        "--commit-verifier",
        required=True,
        help="absolute pinned network-inert OpenPGP verifier executable",
    )
    prepare.add_argument("--output-dir", required=True, type=Path)
    prepare.set_defaults(handler=prepare_rebuild)

    finalize = modes.add_parser("finalize", allow_abbrev=False)
    finalize.add_argument("--policy", required=True, type=Path)
    finalize.add_argument("--trusted-policy-sha256", required=True)
    finalize.add_argument("--engineering-candidate", required=True, type=Path)
    finalize.add_argument("--engineering-signed-rebuild", required=True, type=Path)
    finalize.add_argument("--security-candidate", required=True, type=Path)
    finalize.add_argument("--security-signed-rebuild", required=True, type=Path)
    finalize.add_argument("--output-dir", required=True, type=Path)
    finalize.set_defaults(handler=finalize_release)

    verify = modes.add_parser("verify", allow_abbrev=False)
    verify.add_argument("--release-dir", required=True, type=Path)
    verify.add_argument("--trusted-policy-sha256", required=True)
    verify.set_defaults(handler=verify_release)
    return parser


def main(arguments: Sequence[str] | None = None) -> int:
    """CLI entry point with bounded, secret-free diagnostics."""

    try:
        parsed = _parser().parse_args(arguments)
        parsed.handler(parsed)
        return 0
    except (ValidatorBuilderError, common.SccpReleaseError) as error:
        print(f"SCCP validator builder failed: {error}", file=sys.stderr)
        return 2
    except (OSError, UnicodeError, ValueError, TypeError):
        print("SCCP validator builder failed safely", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
