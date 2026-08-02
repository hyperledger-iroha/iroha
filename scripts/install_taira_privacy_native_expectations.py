#!/usr/bin/env python3
"""Install the one-shot native privacy fixture set and exact X.509 source pins."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
from typing import Any, Callable


PROFILE_RELATIVE_PATH = Path("crates/iroha_core/src/privacy_engines/zk_x509/profile.rs")
READINESS_RELATIVE_PATH = Path(
    "crates/iroha_core/src/privacy_engines/zk_x509/profile/readiness_certificates.rs"
)
EXPECTATIONS_NORITO_RELATIVE_PATH = Path(
    "fixtures/privacy/native_release_expectations_v1.norito"
)
EXPECTATIONS_JSON_RELATIVE_PATH = Path(
    "fixtures/privacy/native_release_expectations_v1.json"
)
RESOURCE_NORITO_RELATIVE_PATH = Path(
    "fixtures/privacy/zk_x509_native_resource_v1.norito"
)
RESOURCE_JSON_RELATIVE_PATH = Path("fixtures/privacy/zk_x509_native_resource_v1.json")
EXACT12_RELATIVE_PATH = Path("fixtures/privacy/exact12_v1.tsv")

KAT_BYTES_PIN = "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1"
KAT_SHA256_PIN = "ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1"
EXPECTATIONS_NORITO_PIN = "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1"
EXPECTATIONS_JSON_PIN = "ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1"
RESOURCE_CERTIFICATE_PIN = "ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1"
OBSERVATION_PINS = {
    "positive_elapsed_millis": "ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1",
    "positive_peak_rss_bytes": "ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1",
    "positive_peak_address_space_bytes": (
        "ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1"
    ),
    "maximum_elapsed_millis": "ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1",
    "maximum_peak_rss_bytes": "ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1",
    "maximum_peak_address_space_bytes": (
        "ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1"
    ),
}

EXPECTED_SCHEMA_VERSION = 1
EXPECTED_STAGE_COUNT = 48
MAX_EXPECTATION_ARTIFACT_BYTES = 1024 * 1024 * 1024
MAX_RESOURCE_ARTIFACT_BYTES = 64 * 1024
MAX_EXACT12_BYTES = 64 * 1024
MAX_SOURCE_BYTES = 16 * 1024 * 1024
MAX_INSTALL_MANIFEST_BYTES = 1024 * 1024
MAX_NATIVE_VERIFIER_BYTES = 1024 * 1024 * 1024
MAX_CARGO_LOCK_BYTES = 128 * 1024 * 1024
MAX_KAT_PROOF_BYTES = 8_212_538
NATIVE_VALIDATION_MODE = "validate-captured-fixtures"
TRANSACTION_RELATIVE_PATH = Path(".taira-privacy-native-install-v1")
TRANSACTION_CLEANUP_RELATIVE_PATH = Path(
    ".taira-privacy-native-install-cleanup-v1"
)
TRANSACTION_STATE_NAME = "state-v1.json"
TRANSACTION_READY_NAME = "READY"
TRANSACTION_SOURCE_BLOBS = (
    "profile.original",
    "readiness.original",
)
TRANSACTION_ALLOWED_NAMES = frozenset(
    (TRANSACTION_STATE_NAME, TRANSACTION_READY_NAME, *TRANSACTION_SOURCE_BLOBS)
)
TRANSACTION_SCHEMA_VERSION = 1
MAX_TRANSACTION_STATE_BYTES = 64 * 1024
HASH_FRAME_DOMAIN = b"iroha.zk-x509.sha256.frame.v1"
RESOURCE_CERTIFICATE_DOMAIN = b"iroha.zk-x509.native-resource-certificate.payload.v1"
RESOURCE_CERTIFICATE_FIELD_COUNT = 60

EXPECTED_ENVIRONMENT: dict[str, object] = {
    "operating_system": "linux",
    "architecture": "aarch64",
    "endianness": "little",
    "kernel_minimum_major": 6,
    "kernel_minimum_minor": 3,
    "rustc_release": "1.93.1",
    "rustc_host": "aarch64-unknown-linux-gnu",
    "rustc_commit_hash": "01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf",
    "rustc_commit_date": "2026-02-11",
    "instance_type": "c7g.4xlarge",
    "cpu_model": "Neoverse-V1",
    "logical_cpu_count": 16,
    "online_cpu_count": 16,
    "affinity_cpu_count": 16,
}
EXPECTED_PROCESS_LIMITS: dict[str, object] = {
    "elapsed_ceiling_millis": 300_000,
    "peak_rss_ceiling_bytes": 12 * 1024 * 1024 * 1024,
    "address_space_ceiling_bytes": 32 * 1024 * 1024 * 1024,
    "main_thread_stack_bytes": 8 * 1024 * 1024,
    "rayon_worker_stack_bytes": 8 * 1024 * 1024,
    "watchdog_thread_stack_bytes": 8 * 1024 * 1024,
    "rayon_worker_count": 4,
    "max_stage_tasks": 6,
    "max_stage_open_files": 4,
    "core_dump_bytes": 0,
    "landlock_abi_minimum": 3,
    "minimum_effective_memory_bytes": 12 * 1024 * 1024 * 1024,
    "cgroup_v2": True,
    "cpu_quota_unlimited": True,
    "landlock_restrict_self": True,
    "anchored_openat2": True,
    "memfd_exec": True,
    "memfd_seal_exec": True,
    "static_elf_only": True,
    "seccomp_tsync": True,
}
EXPECTED_PROTOCOL = {
    "protocol": "iroha-zk-x509-stark-p256-v0",
    "value": None,
}
OBSERVATION_KEYS = {
    "case_kind",
    "elapsed_millis",
    "peak_rss_bytes",
    "peak_address_space_bytes",
    "primary_units",
    "primary_ceiling",
    "secondary_units",
    "secondary_ceiling",
    "relation_depth",
    "relation_depth_ceiling",
}
RESOURCE_KEYS = {
    "schema_version",
    "protocol_id",
    "compiled_profile_digest",
    "environment",
    "expectations_norito_sha256",
    "expectations_json_sha256",
    "kat_proof_bytes",
    "kat_proof_sha256",
    "process_limits",
    "positive",
    "maximum",
    "certificate_sha256",
}


class InstallError(RuntimeError):
    """The fixture set cannot be installed without weakening the release gate."""


def _canonical_existing_directory(raw: str, label: str) -> Path:
    path = Path(raw)
    if not path.is_absolute():
        raise InstallError(f"{label} must be an absolute path")
    canonical = path.resolve(strict=True)
    if canonical != path or not canonical.is_dir():
        raise InstallError(f"{label} must be one canonical physical directory")
    return canonical


def _outside_repository(path: Path, repository: Path, label: str) -> Path:
    if not path.is_absolute():
        raise InstallError(f"{label} must be an absolute path")
    canonical = path.resolve(strict=True)
    if canonical != path:
        raise InstallError(f"{label} must use its canonical physical path")
    try:
        canonical.relative_to(repository)
    except ValueError:
        return canonical
    raise InstallError(f"{label} must be outside the source checkout")


def _stable_regular_bytes(
    path: Path,
    label: str,
    maximum_bytes: int,
    *,
    expected_links: int = 1,
) -> bytes:
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != expected_links
        or before.st_size <= 0
        or before.st_size > maximum_bytes
    ):
        if expected_links == 1:
            raise InstallError(
                f"{label} must be one non-empty, bounded, singly linked regular file"
            )
        raise InstallError(
            f"{label} must be one non-empty, bounded regular file with exactly "
            f"{expected_links} links"
        )
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        identity_fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_nlink",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(
            getattr(opened, field) != getattr(before, field)
            for field in identity_fields
        ):
            raise InstallError(f"{label} changed before it was opened")
        chunks: list[bytes] = []
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                raise InstallError(f"{label} ended before its declared length")
            chunks.append(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise InstallError(f"{label} grew while it was read")
        after = os.fstat(descriptor)
        if any(
            getattr(after, field) != getattr(before, field) for field in identity_fields
        ):
            raise InstallError(f"{label} changed while it was read")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _stable_executable_sha256(path: Path, label: str, maximum_bytes: int) -> str:
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum_bytes
        or before.st_mode & 0o111 == 0
    ):
        raise InstallError(
            f"{label} must be one non-empty, bounded, singly linked executable file"
        )
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        identity_fields = (
            "st_dev",
            "st_ino",
            "st_mode",
            "st_nlink",
            "st_size",
            "st_mtime_ns",
            "st_ctime_ns",
        )
        if any(
            getattr(opened, field) != getattr(before, field)
            for field in identity_fields
        ):
            raise InstallError(f"{label} changed before it was opened")
        digest = hashlib.sha256()
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                raise InstallError(f"{label} ended before its declared length")
            digest.update(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise InstallError(f"{label} grew while it was hashed")
        after = os.fstat(descriptor)
        if any(
            getattr(after, field) != getattr(before, field)
            for field in identity_fields
        ):
            raise InstallError(f"{label} changed while it was hashed")
        return digest.hexdigest()
    finally:
        os.close(descriptor)


def _canonical_sha256(value: str, label: str) -> str:
    if (
        len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
        or value == "0" * 64
    ):
        raise InstallError(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _canonical_commit(value: str, label: str) -> str:
    if (
        len(value) != 40
        or any(character not in "0123456789abcdef" for character in value)
        or value == "0" * 40
    ):
        raise InstallError(f"{label} must be one nonzero lowercase full Git commit")
    return value


def _canonical_signer_principal(value: str, label: str) -> str:
    alphanumeric = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    allowed = alphanumeric + "._@+-"
    if (
        not 1 <= len(value) <= 128
        or value[0] not in alphanumeric
        or value[-1] not in alphanumeric
        or any(character not in allowed for character in value)
    ):
        raise InstallError(
            f"{label} must be one bounded canonical ASCII SSH signer principal"
        )
    return value


def _canonical_ssh_fingerprint(value: str, label: str) -> str:
    prefix = "SHA256:"
    if not value.startswith(prefix) or len(value) != len(prefix) + 43:
        raise InstallError(f"{label} must be one OpenSSH SHA256 fingerprint")
    encoded = value[len(prefix) :]
    if any(
        character
        not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/"
        for character in encoded
    ):
        raise InstallError(f"{label} must be one OpenSSH SHA256 fingerprint")
    try:
        decoded = base64.b64decode(encoded + "=", validate=True)
    except ValueError as error:
        raise InstallError(f"{label} must be one OpenSSH SHA256 fingerprint") from error
    if (
        len(decoded) != 32
        or decoded == b"\0" * 32
        or base64.b64encode(decoded).decode("ascii").rstrip("=") != encoded
    ):
        raise InstallError(
            f"{label} must be one nonzero canonical OpenSSH SHA256 fingerprint"
        )
    return value


def _authenticated_origins(
    *,
    iroha_source_commit: str,
    iroha_signer_principal: str,
    iroha_signer_fingerprint: str,
    iroha_allowed_signers_sha256: str,
    validator_source_commit: str,
    validator_signer_principal: str,
    validator_signer_fingerprint: str,
    validator_allowed_signers_sha256: str,
    validator_source_tree_sha256: str,
    bootstrap_source_tree_sha256: str,
    cargo_lock_sha256: str,
    rust_toolchain_tree_sha256: str,
) -> dict[str, object]:
    return {
        "iroha_source": {
            "commit": _canonical_commit(iroha_source_commit, "Iroha source commit"),
            "signature_format": "ssh",
            "signer_principal": _canonical_signer_principal(
                iroha_signer_principal, "Iroha signer principal"
            ),
            "signer_fingerprint": _canonical_ssh_fingerprint(
                iroha_signer_fingerprint, "Iroha signer fingerprint"
            ),
            "allowed_signers_sha256": _canonical_sha256(
                iroha_allowed_signers_sha256, "Iroha allowed-signers SHA-256"
            ),
        },
        "validator_source": {
            "commit": _canonical_commit(
                validator_source_commit, "validator source commit"
            ),
            "signature_format": "ssh",
            "signer_principal": _canonical_signer_principal(
                validator_signer_principal, "validator signer principal"
            ),
            "signer_fingerprint": _canonical_ssh_fingerprint(
                validator_signer_fingerprint, "validator signer fingerprint"
            ),
            "allowed_signers_sha256": _canonical_sha256(
                validator_allowed_signers_sha256,
                "validator allowed-signers SHA-256",
            ),
            "source_tree_sha256": _canonical_sha256(
                validator_source_tree_sha256, "validator source-tree SHA-256"
            ),
        },
        "build_inputs": {
            "bootstrap_source_tree_sha256": _canonical_sha256(
                bootstrap_source_tree_sha256, "bootstrap source-tree SHA-256"
            ),
            "cargo_lock_sha256": _canonical_sha256(
                cargo_lock_sha256, "Cargo.lock SHA-256"
            ),
        },
        "rust_toolchain": {
            "release": EXPECTED_ENVIRONMENT["rustc_release"],
            "host": EXPECTED_ENVIRONMENT["rustc_host"],
            "compiler_commit": EXPECTED_ENVIRONMENT["rustc_commit_hash"],
            "tree_sha256": _canonical_sha256(
                rust_toolchain_tree_sha256, "Rust toolchain tree SHA-256"
            ),
        },
    }


def _run_native_fixture_validation(
    *,
    native_verifier: Path,
    native_verifier_sha256: str,
    exact12_matrix: Path,
    captured_expectations_norito: Path,
    captured_expectations_json: Path,
    captured_resource_norito: Path,
    captured_resource_json: Path,
) -> None:
    expected_verifier_sha256 = _canonical_sha256(
        native_verifier_sha256, "native verifier SHA-256"
    )
    if (
        _stable_executable_sha256(
            native_verifier,
            "native capture verifier",
            MAX_NATIVE_VERIFIER_BYTES,
        )
        != expected_verifier_sha256
    ):
        raise InstallError("native capture verifier does not match its attested SHA-256")
    command = [
        str(native_verifier),
        NATIVE_VALIDATION_MODE,
        "--exact12-matrix",
        str(exact12_matrix),
        "--expectations-norito",
        str(captured_expectations_norito),
        "--expectations-json",
        str(captured_expectations_json),
        "--x509-resource-norito",
        str(captured_resource_norito),
        "--x509-resource-json",
        str(captured_resource_json),
    ]
    try:
        completed = subprocess.run(
            command,
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            env={},
        )
    except OSError as error:
        raise InstallError(f"cannot execute native capture verifier: {error}") from error
    if completed.returncode != 0:
        diagnostic = completed.stderr.strip()
        if len(diagnostic) > 4096:
            diagnostic = diagnostic[-4096:]
        suffix = f": {diagnostic}" if diagnostic else ""
        raise InstallError(
            f"native typed capture validation failed with code {completed.returncode}{suffix}"
        )
    if (
        _stable_executable_sha256(
            native_verifier,
            "native capture verifier after validation",
            MAX_NATIVE_VERIFIER_BYTES,
        )
        != expected_verifier_sha256
    ):
        raise InstallError("native capture verifier changed during validation")


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for name, item in pairs:
        if name in value:
            raise InstallError(f"JSON contains duplicate field {name!r}")
        value[name] = item
    return value


def _strict_json(encoded: bytes, label: str) -> Any:
    try:
        return json.loads(encoded, object_pairs_hook=_strict_object)
    except InstallError:
        raise
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise InstallError(f"{label} is invalid: {error}") from error


def _validate_expectations_json(encoded: bytes) -> None:
    payload = _strict_json(encoded, "captured expectations JSON")
    if not isinstance(payload, dict) or set(payload) != {
        "schema_version",
        "stage_count",
        "stages",
    }:
        raise InstallError("captured expectations JSON must have the exact fields")
    if (
        _uint(payload["schema_version"], 16, "expectations schema_version")
        != EXPECTED_SCHEMA_VERSION
    ):
        raise InstallError(
            "captured expectations JSON has an unexpected schema_version"
        )
    if (
        _uint(payload["stage_count"], 16, "expectations stage_count")
        != EXPECTED_STAGE_COUNT
    ):
        raise InstallError("captured expectations JSON must declare exactly 48 stages")
    stages = payload.get("stages")
    if not isinstance(stages, list) or len(stages) != EXPECTED_STAGE_COUNT:
        raise InstallError("captured expectations JSON must contain exactly 48 stages")


def _uint(value: Any, bits: int, label: str) -> int:
    if type(value) is not int or value < 0 or value >= 1 << bits:
        raise InstallError(f"{label} must be one unsigned {bits}-bit integer")
    return value


def _digest_bytes(value: Any, label: str, *, nonzero: bool = True) -> bytes:
    if (
        not isinstance(value, list)
        or len(value) != 32
        or any(type(byte) is not int or not 0 <= byte <= 255 for byte in value)
    ):
        raise InstallError(f"{label} must be one exact 32-byte array")
    encoded = bytes(value)
    if nonzero and encoded == bytes(32):
        raise InstallError(f"{label} must not be the all-zero sentinel")
    return encoded


def _require_exact_mapping(
    actual: Any, expected: dict[str, object], label: str
) -> dict[str, Any]:
    if not isinstance(actual, dict) or set(actual) != set(expected):
        raise InstallError(f"{label} must contain the exact canonical fields")
    for name, expected_value in expected.items():
        if (
            type(actual[name]) is not type(expected_value)
            or actual[name] != expected_value
        ):
            raise InstallError(f"{label}.{name} is not the canonical value")
    return actual


def _validate_observation(
    payload: Any,
    *,
    label: str,
    case_label: str,
    shape: tuple[int, int, int, int, int, int],
) -> dict[str, int]:
    if not isinstance(payload, dict) or set(payload) != OBSERVATION_KEYS:
        raise InstallError(f"{label} must contain the exact typed observation fields")
    _require_exact_mapping(
        payload["case_kind"],
        {"case": case_label, "value": None},
        f"{label}.case_kind",
    )
    values = {
        name: _uint(payload[name], 64, f"{label}.{name}")
        for name in OBSERVATION_KEYS
        if name != "case_kind"
    }
    shape_names = (
        "primary_units",
        "primary_ceiling",
        "secondary_units",
        "secondary_ceiling",
        "relation_depth",
        "relation_depth_ceiling",
    )
    if tuple(values[name] for name in shape_names) != shape:
        raise InstallError(f"{label} does not equal the canonical X.509 relation shape")
    for observed, ceiling in (
        ("elapsed_millis", "elapsed_ceiling_millis"),
        ("peak_rss_bytes", "peak_rss_ceiling_bytes"),
        ("peak_address_space_bytes", "address_space_ceiling_bytes"),
    ):
        if values[observed] == 0 or values[observed] > EXPECTED_PROCESS_LIMITS[ceiling]:
            raise InstallError(f"{label}.{observed} is outside its reviewed bound")
    return values


def _frame_digest(fields: list[bytes]) -> bytes:
    if len(fields) != RESOURCE_CERTIFICATE_FIELD_COUNT:
        raise AssertionError("resource-certificate field count drift")
    hasher = hashlib.sha256()
    hasher.update(HASH_FRAME_DOMAIN)
    hasher.update(len(RESOURCE_CERTIFICATE_DOMAIN).to_bytes(2, "big"))
    hasher.update(RESOURCE_CERTIFICATE_DOMAIN)
    hasher.update(RESOURCE_CERTIFICATE_FIELD_COUNT.to_bytes(2, "big"))
    for field in fields:
        hasher.update(len(field).to_bytes(8, "big"))
        hasher.update(field)
    return hasher.digest()


def _resource_certificate_digest(
    payload: dict[str, Any],
    *,
    compiled_profile_digest: bytes,
    expectations_norito_digest: bytes,
    expectations_json_digest: bytes,
    kat_digest: bytes,
    positive: dict[str, int],
    maximum: dict[str, int],
) -> bytes:
    environment = payload["environment"]
    limits = payload["process_limits"]
    fields = [
        _uint(payload["schema_version"], 16, "schema_version").to_bytes(2, "big"),
        compiled_profile_digest,
        environment["operating_system"].encode(),
        environment["architecture"].encode(),
        environment["endianness"].encode(),
        _uint(environment["kernel_minimum_major"], 16, "kernel major").to_bytes(
            2, "big"
        ),
        _uint(environment["kernel_minimum_minor"], 16, "kernel minor").to_bytes(
            2, "big"
        ),
        environment["rustc_release"].encode(),
        environment["rustc_host"].encode(),
        environment["rustc_commit_hash"].encode(),
        environment["rustc_commit_date"].encode(),
        environment["instance_type"].encode(),
        environment["cpu_model"].encode(),
        _uint(environment["logical_cpu_count"], 16, "logical CPUs").to_bytes(2, "big"),
        _uint(environment["online_cpu_count"], 16, "online CPUs").to_bytes(2, "big"),
        _uint(environment["affinity_cpu_count"], 16, "affinity CPUs").to_bytes(
            2, "big"
        ),
        expectations_norito_digest,
        expectations_json_digest,
        _uint(payload["kat_proof_bytes"], 32, "kat_proof_bytes").to_bytes(4, "big"),
        kat_digest,
    ]
    for name in (
        "elapsed_ceiling_millis",
        "peak_rss_ceiling_bytes",
        "address_space_ceiling_bytes",
        "main_thread_stack_bytes",
        "rayon_worker_stack_bytes",
        "watchdog_thread_stack_bytes",
    ):
        fields.append(
            _uint(limits[name], 64, f"process_limits.{name}").to_bytes(8, "big")
        )
    for name in ("rayon_worker_count", "max_stage_tasks", "max_stage_open_files"):
        fields.append(
            _uint(limits[name], 16, f"process_limits.{name}").to_bytes(2, "big")
        )
    fields.append(
        _uint(limits["core_dump_bytes"], 64, "process_limits.core_dump_bytes").to_bytes(
            8, "big"
        )
    )
    fields.append(
        _uint(
            limits["landlock_abi_minimum"],
            16,
            "process_limits.landlock_abi_minimum",
        ).to_bytes(2, "big")
    )
    fields.append(
        _uint(
            limits["minimum_effective_memory_bytes"],
            64,
            "process_limits.minimum_effective_memory_bytes",
        ).to_bytes(8, "big")
    )
    for name in (
        "cgroup_v2",
        "cpu_quota_unlimited",
        "landlock_restrict_self",
        "anchored_openat2",
        "memfd_exec",
        "memfd_seal_exec",
        "static_elf_only",
        "seccomp_tsync",
    ):
        if type(limits[name]) is not bool:
            raise InstallError(f"process_limits.{name} must be boolean")
        fields.append(bytes([limits[name]]))
    observation_order = (
        "elapsed_millis",
        "peak_rss_bytes",
        "peak_address_space_bytes",
        "primary_units",
        "primary_ceiling",
        "secondary_units",
        "secondary_ceiling",
        "relation_depth",
        "relation_depth_ceiling",
    )
    for case_ordinal, observation in ((0, positive), (3, maximum)):
        fields.append(bytes([case_ordinal]))
        fields.extend(
            observation[name].to_bytes(8, "big") for name in observation_order
        )
    return _frame_digest(fields)


def _validate_resource_json(
    encoded: bytes,
    expectations_norito_sha256: str,
    expectations_json_sha256: str,
) -> dict[str, Any]:
    payload = _strict_json(encoded, "captured X.509 resource JSON")
    if not isinstance(payload, dict) or set(payload) != RESOURCE_KEYS:
        raise InstallError(
            "captured X.509 resource JSON must contain the exact typed fields"
        )
    if (
        _uint(payload["schema_version"], 16, "schema_version")
        != EXPECTED_SCHEMA_VERSION
    ):
        raise InstallError("captured X.509 resource JSON has the wrong schema version")
    _require_exact_mapping(payload["protocol_id"], EXPECTED_PROTOCOL, "protocol_id")
    _require_exact_mapping(payload["environment"], EXPECTED_ENVIRONMENT, "environment")
    _require_exact_mapping(
        payload["process_limits"], EXPECTED_PROCESS_LIMITS, "process_limits"
    )

    compiled_digest = _digest_bytes(
        payload["compiled_profile_digest"], "compiled_profile_digest"
    )
    expectations_norito_digest = _digest_bytes(
        payload["expectations_norito_sha256"], "expectations_norito_sha256"
    )
    expectations_json_digest = _digest_bytes(
        payload["expectations_json_sha256"], "expectations_json_sha256"
    )
    if expectations_norito_digest.hex() != expectations_norito_sha256:
        raise InstallError("resource certificate binds the wrong expectations Norito")
    if expectations_json_digest.hex() != expectations_json_sha256:
        raise InstallError("resource certificate binds the wrong expectations JSON")
    if expectations_norito_digest == expectations_json_digest:
        raise InstallError("resource certificate expectation digests must differ")

    kat_proof_bytes = _uint(payload["kat_proof_bytes"], 32, "kat_proof_bytes")
    if not 0 < kat_proof_bytes <= MAX_KAT_PROOF_BYTES:
        raise InstallError("kat_proof_bytes is outside the canonical X5S1 bound")
    kat_digest = _digest_bytes(payload["kat_proof_sha256"], "kat_proof_sha256")
    positive = _validate_observation(
        payload["positive"],
        label="positive",
        case_label="positive-canonical-end-to-end",
        shape=(2, 3, 1, 4, 0, 64),
    )
    maximum = _validate_observation(
        payload["maximum"],
        label="maximum",
        case_label="maximum-shape-resource",
        shape=(3, 3, 4, 4, 64, 64),
    )
    claimed_digest = _digest_bytes(payload["certificate_sha256"], "certificate_sha256")
    calculated_digest = _resource_certificate_digest(
        payload,
        compiled_profile_digest=compiled_digest,
        expectations_norito_digest=expectations_norito_digest,
        expectations_json_digest=expectations_json_digest,
        kat_digest=kat_digest,
        positive=positive,
        maximum=maximum,
    )
    if claimed_digest != calculated_digest:
        raise InstallError("resource certificate payload digest does not match")
    return {
        "certificate_sha256": calculated_digest.hex(),
        "compiled_profile_sha256": compiled_digest.hex(),
        "kat_proof_bytes": kat_proof_bytes,
        "kat_proof_sha256": kat_digest.hex(),
        "positive": positive,
        "maximum": maximum,
    }


def _sha256(encoded: bytes) -> str:
    return hashlib.sha256(encoded).hexdigest()


def _render_digest(digest_hex: str) -> str:
    values = [f"0x{digest_hex[index : index + 2]}" for index in range(0, 64, 2)]
    lines = [
        "    " + ", ".join(values[index : index + 8]) + ","
        for index in range(0, len(values), 8)
    ]
    return "[\n" + "\n".join(lines) + "\n]"


def _replace_zero_digest(source: str, name: str, digest_hex: str) -> str:
    declaration = f"pub(crate) const {name}: [u8; 32] = [0; 32];"
    if source.count(declaration) != 1:
        raise InstallError(f"{name} must have exactly one all-zero declaration")
    return source.replace(
        declaration,
        f"pub(crate) const {name}: [u8; 32] = {_render_digest(digest_hex)};",
    )


def _replace_zero_integer(source: str, name: str, kind: str, value: int) -> str:
    declaration = f"pub(crate) const {name}: {kind} = 0;"
    if source.count(declaration) != 1:
        raise InstallError(f"{name} must have exactly one all-zero declaration")
    return source.replace(declaration, f"pub(crate) const {name}: {kind} = {value};")


def _create_new_file(path: Path, encoded: bytes, mode: int) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
        mode,
    )
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fchmod(stream.fileno(), mode)
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def _sync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


Failpoint = Callable[[str], None]


def _trip_failpoint(failpoint: Failpoint, phase: str) -> None:
    if failpoint is not None:
        failpoint(phase)


def _replacement_temporary_path(path: Path) -> Path:
    return path.with_name(f".{path.name}.taira-privacy-native-install-v1.tmp")


def _atomic_create_new(
    path: Path,
    encoded: bytes,
    mode: int,
    label: str,
    *,
    failpoint: Failpoint = None,
    temporary_phase: str | None = None,
    published_phase: str | None = None,
) -> None:
    temporary = _replacement_temporary_path(path)
    if os.path.lexists(path) or os.path.lexists(temporary):
        raise InstallError(f"{label} target or transaction temporary already exists")
    _create_new_file(temporary, encoded, mode)
    _sync_directory(path.parent)
    if temporary_phase is not None:
        _trip_failpoint(failpoint, temporary_phase)
    os.link(temporary, path, follow_symlinks=False)
    _sync_directory(path.parent)
    if published_phase is not None:
        _trip_failpoint(failpoint, published_phase)
    temporary.unlink()
    _sync_directory(path.parent)


def _atomic_replace(
    path: Path,
    encoded: bytes,
    mode: int,
    label: str,
    *,
    failpoint: Failpoint = None,
    temporary_phase: str | None = None,
) -> None:
    temporary = _replacement_temporary_path(path)
    if os.path.lexists(temporary):
        raise InstallError(f"{label} transaction temporary already exists")
    _create_new_file(temporary, encoded, mode)
    _sync_directory(path.parent)
    if temporary_phase is not None:
        _trip_failpoint(failpoint, temporary_phase)
    os.replace(temporary, path)
    _sync_directory(path.parent)


def _canonical_json_document(payload: dict[str, object]) -> bytes:
    return (
        json.dumps(payload, indent=2, sort_keys=True, separators=(",", ": "))
        + "\n"
    ).encode()


def _transaction_paths(repository: Path) -> tuple[Path, Path]:
    return (
        repository / TRANSACTION_RELATIVE_PATH,
        repository / TRANSACTION_CLEANUP_RELATIVE_PATH,
    )


def _validate_transaction_directory(path: Path, label: str) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or path.resolve(strict=True) != path
    ):
        raise InstallError(f"{label} must be one canonical mode-0700 directory")


def _discard_transaction_directory(path: Path, label: str) -> None:
    _validate_transaction_directory(path, label)
    entries = list(path.iterdir())
    unexpected = sorted(
        entry.name for entry in entries if entry.name not in TRANSACTION_ALLOWED_NAMES
    )
    if unexpected:
        raise InstallError(
            f"{label} contains unexpected entries: {', '.join(unexpected)}"
        )
    for entry in entries:
        metadata = entry.lstat()
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            raise InstallError(f"{label} entry {entry.name} is not a regular file")
    for entry in entries:
        entry.unlink()
    path.rmdir()
    _sync_directory(path.parent)


def _finalize_transaction_directory(repository: Path) -> None:
    transaction, cleanup = _transaction_paths(repository)
    if os.path.lexists(cleanup):
        raise InstallError("transaction cleanup tombstone already exists")
    os.rename(transaction, cleanup)
    _sync_directory(repository)
    _discard_transaction_directory(cleanup, "transaction cleanup tombstone")


def _transaction_source_entry(
    *,
    relative_path: Path,
    original_blob: str,
    original: bytes,
    installed: bytes,
    mode: int,
) -> dict[str, object]:
    return {
        "path": relative_path.as_posix(),
        "original_blob": original_blob,
        "original_bytes": len(original),
        "original_sha256": _sha256(original),
        "installed_bytes": len(installed),
        "installed_sha256": _sha256(installed),
        "mode": mode,
    }


def _build_transaction_state(
    *,
    repository: Path,
    profile_bytes: bytes,
    patched_profile: bytes,
    profile_mode: int,
    readiness_bytes: bytes,
    patched_readiness: bytes,
    readiness_mode: int,
    targets: tuple[Path, Path, Path, Path],
    captured: tuple[bytes, bytes, bytes, bytes],
    manifest_path: Path,
    manifest_encoded: bytes,
) -> dict[str, object]:
    return {
        "schema_version": TRANSACTION_SCHEMA_VERSION,
        "manifest_path": str(manifest_path),
        "sources": {
            "profile": _transaction_source_entry(
                relative_path=PROFILE_RELATIVE_PATH,
                original_blob=TRANSACTION_SOURCE_BLOBS[0],
                original=profile_bytes,
                installed=patched_profile,
                mode=profile_mode,
            ),
            "readiness": _transaction_source_entry(
                relative_path=READINESS_RELATIVE_PATH,
                original_blob=TRANSACTION_SOURCE_BLOBS[1],
                original=readiness_bytes,
                installed=patched_readiness,
                mode=readiness_mode,
            ),
        },
        "fixtures": [
            {
                "path": target.relative_to(repository).as_posix(),
                "bytes": len(encoded),
                "sha256": _sha256(encoded),
                "mode": 0o444,
            }
            for target, encoded in zip(targets, captured)
        ],
        "manifest": {
            "bytes": len(manifest_encoded),
            "sha256": _sha256(manifest_encoded),
            "mode": 0o600,
        },
    }


def _transaction_entry(
    value: Any,
    *,
    label: str,
    expected_path: str,
    expected_mode: int | None,
    expected_blob: str | None = None,
) -> dict[str, Any]:
    expected_keys = {"path", "bytes", "sha256", "mode"}
    if expected_blob is not None:
        expected_keys = {
            "path",
            "original_blob",
            "original_bytes",
            "original_sha256",
            "installed_bytes",
            "installed_sha256",
            "mode",
        }
    if not isinstance(value, dict) or set(value) != expected_keys:
        raise InstallError(f"{label} has noncanonical transaction fields")
    if value["path"] != expected_path or type(value["path"]) is not str:
        raise InstallError(f"{label}.path is not the fixed transaction path")
    mode = _uint(value["mode"], 16, f"{label}.mode")
    if mode > 0o777 or (expected_mode is not None and mode != expected_mode):
        raise InstallError(f"{label}.mode is not a valid fixed transaction mode")
    if expected_blob is None:
        if _uint(value["bytes"], 64, f"{label}.bytes") == 0:
            raise InstallError(f"{label}.bytes must be nonzero")
        _canonical_sha256(value["sha256"], f"{label}.sha256")
    else:
        if value["original_blob"] != expected_blob:
            raise InstallError(f"{label}.original_blob is not canonical")
        for field in ("original_bytes", "installed_bytes"):
            if _uint(value[field], 64, f"{label}.{field}") == 0:
                raise InstallError(f"{label}.{field} must be nonzero")
        for field in ("original_sha256", "installed_sha256"):
            _canonical_sha256(value[field], f"{label}.{field}")
    return value


def _load_transaction_state(
    *,
    repository: Path,
    transaction: Path,
    targets: tuple[Path, Path, Path, Path],
) -> tuple[dict[str, Any], tuple[bytes, bytes], Path]:
    state_bytes = _stable_regular_bytes(
        transaction / TRANSACTION_STATE_NAME,
        "transaction state",
        MAX_TRANSACTION_STATE_BYTES,
    )
    ready = _stable_regular_bytes(
        transaction / TRANSACTION_READY_NAME,
        "transaction ready marker",
        128,
    )
    if ready != f"{_sha256(state_bytes)}\n".encode():
        raise InstallError("transaction ready marker does not authenticate its state")
    payload = _strict_json(state_bytes, "transaction state")
    if not isinstance(payload, dict) or set(payload) != {
        "schema_version",
        "manifest_path",
        "sources",
        "fixtures",
        "manifest",
    }:
        raise InstallError("transaction state has noncanonical top-level fields")
    if _uint(payload["schema_version"], 16, "transaction schema_version") != 1:
        raise InstallError("transaction state schema is not exactly v1")
    raw_manifest_path = payload["manifest_path"]
    if type(raw_manifest_path) is not str:
        raise InstallError("transaction manifest path is not a string")
    transaction_manifest_path = Path(raw_manifest_path)
    if (
        not transaction_manifest_path.is_absolute()
        or Path(os.path.normpath(raw_manifest_path)) != transaction_manifest_path
    ):
        raise InstallError("transaction manifest path is not canonical and absolute")
    try:
        transaction_manifest_path.relative_to(repository)
    except ValueError:
        pass
    else:
        raise InstallError("transaction manifest path must remain outside the checkout")
    if transaction_manifest_path.parent.exists() and (
        transaction_manifest_path.parent.resolve(strict=True)
        != transaction_manifest_path.parent
    ):
        raise InstallError("transaction manifest parent is no longer canonical")
    if _canonical_json_document(payload) != state_bytes:
        raise InstallError("transaction state is not canonical JSON")

    sources = payload["sources"]
    if not isinstance(sources, dict) or set(sources) != {"profile", "readiness"}:
        raise InstallError("transaction sources are not the exact fixed pair")
    source_entries = (
        _transaction_entry(
            sources["profile"],
            label="transaction profile source",
            expected_path=PROFILE_RELATIVE_PATH.as_posix(),
            expected_mode=None,
            expected_blob=TRANSACTION_SOURCE_BLOBS[0],
        ),
        _transaction_entry(
            sources["readiness"],
            label="transaction readiness source",
            expected_path=READINESS_RELATIVE_PATH.as_posix(),
            expected_mode=None,
            expected_blob=TRANSACTION_SOURCE_BLOBS[1],
        ),
    )
    originals: list[bytes] = []
    for entry, blob_name, label in zip(
        source_entries,
        TRANSACTION_SOURCE_BLOBS,
        ("profile rollback source", "readiness rollback source"),
    ):
        original = _stable_regular_bytes(
            transaction / blob_name, label, MAX_SOURCE_BYTES
        )
        if (
            len(original) != entry["original_bytes"]
            or _sha256(original) != entry["original_sha256"]
        ):
            raise InstallError(f"{label} does not match transaction state")
        originals.append(original)

    fixtures = payload["fixtures"]
    if not isinstance(fixtures, list) or len(fixtures) != len(targets):
        raise InstallError("transaction fixtures are not the exact four-file closure")
    for index, (entry, target) in enumerate(zip(fixtures, targets)):
        _transaction_entry(
            entry,
            label=f"transaction fixture {index}",
            expected_path=target.relative_to(repository).as_posix(),
            expected_mode=0o444,
        )
    manifest = payload["manifest"]
    if not isinstance(manifest, dict) or set(manifest) != {"bytes", "sha256", "mode"}:
        raise InstallError("transaction manifest entry has noncanonical fields")
    if (
        _uint(manifest["bytes"], 64, "transaction manifest bytes") == 0
        or _uint(manifest["mode"], 16, "transaction manifest mode") != 0o600
    ):
        raise InstallError("transaction manifest size or mode is invalid")
    _canonical_sha256(manifest["sha256"], "transaction manifest SHA-256")
    return payload, (originals[0], originals[1]), transaction_manifest_path


def _prepare_transaction(
    *,
    repository: Path,
    state: dict[str, object],
    profile_bytes: bytes,
    readiness_bytes: bytes,
) -> None:
    transaction, cleanup = _transaction_paths(repository)
    if os.path.lexists(transaction) or os.path.lexists(cleanup):
        raise InstallError("transaction path was not recovered before preparation")
    os.mkdir(transaction, 0o700)
    os.chmod(transaction, 0o700)
    _sync_directory(repository)
    try:
        _create_new_file(
            transaction / TRANSACTION_SOURCE_BLOBS[0], profile_bytes, 0o600
        )
        _create_new_file(
            transaction / TRANSACTION_SOURCE_BLOBS[1], readiness_bytes, 0o600
        )
        state_bytes = _canonical_json_document(state)
        if len(state_bytes) > MAX_TRANSACTION_STATE_BYTES:
            raise InstallError("transaction state exceeds its fixed size bound")
        _create_new_file(transaction / TRANSACTION_STATE_NAME, state_bytes, 0o600)
        _sync_directory(transaction)
        _create_new_file(
            transaction / TRANSACTION_READY_NAME,
            f"{_sha256(state_bytes)}\n".encode(),
            0o600,
        )
        _sync_directory(transaction)
        _sync_directory(repository)
    except BaseException:
        if not os.path.lexists(transaction / TRANSACTION_READY_NAME):
            _discard_transaction_directory(
                transaction, "incomplete installation transaction"
            )
        raise


def _file_matches_transaction_entry(
    path: Path,
    entry: dict[str, Any],
    *,
    label: str,
    maximum_bytes: int,
    digest_field: str = "sha256",
    bytes_field: str = "bytes",
    expected_links: int = 1,
) -> bool:
    if not os.path.lexists(path):
        return False
    encoded = _stable_regular_bytes(
        path, label, maximum_bytes, expected_links=expected_links
    )
    mode = stat.S_IMODE(path.stat().st_mode)
    return (
        len(encoded) == entry[bytes_field]
        and _sha256(encoded) == entry[digest_field]
        and mode == entry["mode"]
    )


def _reconcile_transaction_temporary(
    path: Path,
    entry: dict[str, Any],
    *,
    label: str,
    maximum_bytes: int,
    digest_field: str = "sha256",
    bytes_field: str = "bytes",
) -> None:
    temporary = _replacement_temporary_path(path)
    if not os.path.lexists(temporary):
        return
    temporary_metadata = temporary.lstat()
    if not stat.S_ISREG(temporary_metadata.st_mode):
        raise InstallError(f"{label} temporary is not a regular file")

    if os.path.lexists(path):
        path_metadata = path.lstat()
        if (
            stat.S_ISREG(path_metadata.st_mode)
            and (path_metadata.st_dev, path_metadata.st_ino)
            == (temporary_metadata.st_dev, temporary_metadata.st_ino)
        ):
            if temporary_metadata.st_nlink != 2 or path_metadata.st_nlink != 2:
                raise InstallError(
                    f"{label} published temporary has an unexpected link count"
                )
            if not _file_matches_transaction_entry(
                path,
                entry,
                label=f"{label} published temporary",
                maximum_bytes=maximum_bytes,
                digest_field=digest_field,
                bytes_field=bytes_field,
                expected_links=2,
            ):
                raise InstallError(
                    f"{label} published temporary differs from transaction state"
                )
            temporary.unlink()
            _sync_directory(path.parent)
            return

    if not _file_matches_transaction_entry(
        temporary,
        entry,
        label=f"{label} unpublished temporary",
        maximum_bytes=maximum_bytes,
        digest_field=digest_field,
        bytes_field=bytes_field,
    ):
        raise InstallError(f"{label} temporary differs from transaction state")
    temporary.unlink()
    _sync_directory(path.parent)


def _remove_uncommitted_fixture(
    path: Path,
    entry: dict[str, Any],
    *,
    label: str,
    maximum_bytes: int,
) -> None:
    if not os.path.lexists(path):
        return
    if not _file_matches_transaction_entry(
        path, entry, label=label, maximum_bytes=maximum_bytes
    ):
        raise InstallError(f"{label} differs from the uncommitted transaction")
    path.unlink()
    _sync_directory(path.parent)


def _recover_transaction(
    *,
    repository: Path,
    manifest_path: Path,
    profile_path: Path,
    readiness_path: Path,
    targets: tuple[Path, Path, Path, Path],
    maximums: tuple[int, int, int, int],
    failpoint: Failpoint,
) -> str | None:
    transaction, cleanup = _transaction_paths(repository)
    if os.path.lexists(cleanup):
        _discard_transaction_directory(cleanup, "transaction cleanup tombstone")
    temporary_paths = tuple(
        _replacement_temporary_path(path)
        for path in (*targets, profile_path, readiness_path, manifest_path)
    )
    if not os.path.lexists(transaction):
        leftover = next((path for path in temporary_paths if os.path.lexists(path)), None)
        if leftover is not None:
            raise InstallError(f"orphaned installation temporary exists: {leftover}")
        return None
    _validate_transaction_directory(transaction, "installation transaction")
    ready_path = transaction / TRANSACTION_READY_NAME
    if not os.path.lexists(ready_path):
        if any(os.path.lexists(path) for path in (*targets, manifest_path, *temporary_paths)):
            raise InstallError(
                "incomplete transaction journal coexists with installation mutations"
            )
        _discard_transaction_directory(transaction, "incomplete installation transaction")
        return "discarded"

    state, originals, transaction_manifest_path = _load_transaction_state(
        repository=repository,
        transaction=transaction,
        targets=targets,
    )
    sources = state["sources"]
    fixtures = state["fixtures"]
    manifest = state["manifest"]
    if (
        not isinstance(sources, dict)
        or not isinstance(fixtures, list)
        or not isinstance(manifest, dict)
    ):
        raise InstallError("validated transaction state lost its typed structure")

    transaction_manifest_temporary = _replacement_temporary_path(
        transaction_manifest_path
    )
    invocation_manifest_temporary = _replacement_temporary_path(manifest_path)
    if (
        invocation_manifest_temporary != transaction_manifest_temporary
        and os.path.lexists(invocation_manifest_temporary)
    ):
        raise InstallError(
            "new invocation manifest temporary is not owned by the durable transaction"
        )
    for label, path, entry in (
        ("profile source", profile_path, sources["profile"]),
        ("readiness source", readiness_path, sources["readiness"]),
    ):
        _reconcile_transaction_temporary(
            path,
            entry,
            label=label,
            maximum_bytes=MAX_SOURCE_BYTES,
            digest_field="installed_sha256",
            bytes_field="installed_bytes",
        )
    for index, (path, entry, maximum) in enumerate(
        zip(targets, fixtures, maximums)
    ):
        _reconcile_transaction_temporary(
            path,
            entry,
            label=f"fixture {index}",
            maximum_bytes=maximum,
        )
    _reconcile_transaction_temporary(
        transaction_manifest_path,
        manifest,
        label="installation manifest",
        maximum_bytes=MAX_INSTALL_MANIFEST_BYTES,
    )
    if os.path.lexists(transaction_manifest_path):
        if not _file_matches_transaction_entry(
            transaction_manifest_path,
            manifest,
            label="committed installation manifest",
            maximum_bytes=MAX_INSTALL_MANIFEST_BYTES,
        ):
            raise InstallError("installation commit marker differs from transaction state")
        for label, path, entry in (
            ("committed profile source", profile_path, sources["profile"]),
            ("committed readiness source", readiness_path, sources["readiness"]),
        ):
            if not _file_matches_transaction_entry(
                path,
                entry,
                label=label,
                maximum_bytes=MAX_SOURCE_BYTES,
                digest_field="installed_sha256",
                bytes_field="installed_bytes",
            ):
                raise InstallError(f"{label} differs from committed transaction state")
        for index, (path, entry, maximum) in enumerate(
            zip(targets, fixtures, maximums)
        ):
            if not _file_matches_transaction_entry(
                path,
                entry,
                label=f"committed fixture {index}",
                maximum_bytes=maximum,
            ):
                raise InstallError(
                    f"committed fixture {index} differs from transaction state"
                )
        committed_temporaries = (
            *temporary_paths[:-1],
            transaction_manifest_temporary,
        )
        if any(os.path.lexists(path) for path in committed_temporaries):
            raise InstallError("committed transaction retains an installation temporary")
        _trip_failpoint(failpoint, "recovery_commit_validated")
        _finalize_transaction_directory(repository)
        return "committed"

    for label, path, entry, original in (
        ("profile source", profile_path, sources["profile"], originals[0]),
        ("readiness source", readiness_path, sources["readiness"], originals[1]),
    ):
        current = _stable_regular_bytes(path, label, MAX_SOURCE_BYTES)
        current_mode = stat.S_IMODE(path.stat().st_mode)
        current_is_original = (
            len(current) == entry["original_bytes"]
            and _sha256(current) == entry["original_sha256"]
        )
        current_is_installed = (
            len(current) == entry["installed_bytes"]
            and _sha256(current) == entry["installed_sha256"]
        )
        if not current_is_original and not current_is_installed:
            raise InstallError(f"{label} differs from both transaction states")
        if not current_is_original or current_mode != entry["mode"]:
            _atomic_replace(path, original, entry["mode"], f"rollback {label}")
        _trip_failpoint(failpoint, f"recovery_{label.replace(' ', '_')}_restored")

    for index, (path, entry, maximum) in enumerate(
        zip(targets, fixtures, maximums)
    ):
        _remove_uncommitted_fixture(
            path,
            entry,
            label=f"uncommitted fixture {index}",
            maximum_bytes=maximum,
        )
        _trip_failpoint(failpoint, f"recovery_fixture_{index}_removed")
    if any(
        os.path.lexists(_replacement_temporary_path(path))
        for path in (*targets, profile_path, readiness_path, transaction_manifest_path)
    ):
        raise InstallError("rollback retained an installation temporary")
    _trip_failpoint(failpoint, "recovery_rollback_complete")
    _finalize_transaction_directory(repository)
    return "rolled_back"


def install(
    *,
    repository: Path,
    native_verifier: Path,
    native_verifier_sha256: str,
    exact12_matrix: Path,
    captured_expectations_norito: Path,
    captured_expectations_json: Path,
    captured_resource_norito: Path,
    captured_resource_json: Path,
    manifest_path: Path,
    authenticated_iroha_source_commit: str,
    authenticated_iroha_signer_principal: str,
    authenticated_iroha_signer_fingerprint: str,
    authenticated_iroha_allowed_signers_sha256: str,
    authenticated_validator_source_commit: str,
    authenticated_validator_signer_principal: str,
    authenticated_validator_signer_fingerprint: str,
    authenticated_validator_allowed_signers_sha256: str,
    authenticated_validator_source_tree_sha256: str,
    authenticated_bootstrap_source_tree_sha256: str,
    authenticated_cargo_lock_sha256: str,
    authenticated_rust_toolchain_tree_sha256: str,
    _failpoint: Failpoint = None,
) -> dict[str, object]:
    """Validate and atomically install the first-release native fixture set."""

    profile_path = repository / PROFILE_RELATIVE_PATH
    readiness_path = repository / READINESS_RELATIVE_PATH
    canonical_exact12_path = repository / EXACT12_RELATIVE_PATH
    if exact12_matrix != canonical_exact12_path:
        raise InstallError(
            "exact12 matrix must be the canonical first-release repository fixture"
        )
    targets = (
        repository / EXPECTATIONS_NORITO_RELATIVE_PATH,
        repository / EXPECTATIONS_JSON_RELATIVE_PATH,
        repository / RESOURCE_NORITO_RELATIVE_PATH,
        repository / RESOURCE_JSON_RELATIVE_PATH,
    )
    maximums = (
        MAX_EXPECTATION_ARTIFACT_BYTES,
        MAX_EXPECTATION_ARTIFACT_BYTES,
        MAX_RESOURCE_ARTIFACT_BYTES,
        MAX_RESOURCE_ARTIFACT_BYTES,
    )
    labels = (
        "captured expectations Norito",
        "captured expectations JSON",
        "captured X.509 resource Norito",
        "captured X.509 resource JSON",
    )
    for parent, label in (
        (profile_path.parent, "ZK-X509 profile parent"),
        (readiness_path.parent, "readiness-certificate parent"),
        (targets[0].parent, "privacy fixture parent"),
        (manifest_path.parent, "installation manifest parent"),
    ):
        if parent.resolve(strict=True) != parent:
            raise InstallError(f"{label} must use its canonical physical path")
    _recover_transaction(
        repository=repository,
        manifest_path=manifest_path,
        profile_path=profile_path,
        readiness_path=readiness_path,
        targets=targets,
        maximums=maximums,
        failpoint=_failpoint,
    )
    for target in (*targets, manifest_path):
        if os.path.lexists(target):
            raise InstallError(f"one-shot installation target already exists: {target}")

    authenticated_origins = _authenticated_origins(
        iroha_source_commit=authenticated_iroha_source_commit,
        iroha_signer_principal=authenticated_iroha_signer_principal,
        iroha_signer_fingerprint=authenticated_iroha_signer_fingerprint,
        iroha_allowed_signers_sha256=authenticated_iroha_allowed_signers_sha256,
        validator_source_commit=authenticated_validator_source_commit,
        validator_signer_principal=authenticated_validator_signer_principal,
        validator_signer_fingerprint=authenticated_validator_signer_fingerprint,
        validator_allowed_signers_sha256=(
            authenticated_validator_allowed_signers_sha256
        ),
        validator_source_tree_sha256=authenticated_validator_source_tree_sha256,
        bootstrap_source_tree_sha256=authenticated_bootstrap_source_tree_sha256,
        cargo_lock_sha256=authenticated_cargo_lock_sha256,
        rust_toolchain_tree_sha256=authenticated_rust_toolchain_tree_sha256,
    )
    cargo_lock_path = repository / "Cargo.lock"
    cargo_lock_bytes = _stable_regular_bytes(
        cargo_lock_path, "authenticated Cargo.lock", MAX_CARGO_LOCK_BYTES
    )
    if _sha256(cargo_lock_bytes) != authenticated_cargo_lock_sha256:
        raise InstallError("Cargo.lock does not match its authenticated origin digest")

    capture_paths = (
        captured_expectations_norito,
        captured_expectations_json,
        captured_resource_norito,
        captured_resource_json,
    )
    validation_paths = (*capture_paths, exact12_matrix, native_verifier)
    identities = [(path.lstat().st_dev, path.lstat().st_ino) for path in validation_paths]
    if len(set(identities)) != len(identities):
        raise InstallError("native validation inputs must not alias any inode")
    captured = tuple(
        _stable_regular_bytes(path, label, maximum)
        for path, label, maximum in zip(capture_paths, labels, maximums)
    )
    exact12_bytes = _stable_regular_bytes(
        exact12_matrix, "exact12 matrix", MAX_EXACT12_BYTES
    )
    expectations_norito, expectations_json, resource_norito, resource_json = captured
    _validate_expectations_json(expectations_json)
    expectation_norito_sha256 = _sha256(expectations_norito)
    expectation_json_sha256 = _sha256(expectations_json)
    if expectation_norito_sha256 == expectation_json_sha256:
        raise InstallError("Norito and JSON expectation digests must differ")
    resource_values = _validate_resource_json(
        resource_json,
        expectation_norito_sha256,
        expectation_json_sha256,
    )
    if _sha256(resource_norito) == _sha256(resource_json):
        raise InstallError("Norito and JSON resource-certificate digests must differ")

    _run_native_fixture_validation(
        native_verifier=native_verifier,
        native_verifier_sha256=native_verifier_sha256,
        exact12_matrix=exact12_matrix,
        captured_expectations_norito=captured_expectations_norito,
        captured_expectations_json=captured_expectations_json,
        captured_resource_norito=captured_resource_norito,
        captured_resource_json=captured_resource_json,
    )
    captured_after_validation = tuple(
        _stable_regular_bytes(path, label, maximum)
        for path, label, maximum in zip(capture_paths, labels, maximums)
    )
    if captured_after_validation != captured:
        raise InstallError("captured fixture bytes changed during native validation")
    if (
        _stable_regular_bytes(
            exact12_matrix, "exact12 matrix after native validation", MAX_EXACT12_BYTES
        )
        != exact12_bytes
    ):
        raise InstallError("exact12 matrix changed during native validation")

    profile_bytes = _stable_regular_bytes(
        profile_path, "ZK-X509 profile source", MAX_SOURCE_BYTES
    )
    readiness_bytes = _stable_regular_bytes(
        readiness_path, "X.509 readiness-certificate source", MAX_SOURCE_BYTES
    )
    try:
        profile_source = profile_bytes.decode("utf-8")
        readiness_source = readiness_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise InstallError("X.509 source pin files must be UTF-8") from error

    patched_profile = _replace_zero_integer(
        profile_source,
        KAT_BYTES_PIN,
        "u32",
        resource_values["kat_proof_bytes"],
    )
    patched_profile = _replace_zero_digest(
        patched_profile, KAT_SHA256_PIN, resource_values["kat_proof_sha256"]
    )
    patched_profile = _replace_zero_digest(
        patched_profile, EXPECTATIONS_NORITO_PIN, expectation_norito_sha256
    )
    patched_profile = _replace_zero_digest(
        patched_profile, EXPECTATIONS_JSON_PIN, expectation_json_sha256
    )
    observations = {
        "positive_elapsed_millis": resource_values["positive"]["elapsed_millis"],
        "positive_peak_rss_bytes": resource_values["positive"]["peak_rss_bytes"],
        "positive_peak_address_space_bytes": resource_values["positive"][
            "peak_address_space_bytes"
        ],
        "maximum_elapsed_millis": resource_values["maximum"]["elapsed_millis"],
        "maximum_peak_rss_bytes": resource_values["maximum"]["peak_rss_bytes"],
        "maximum_peak_address_space_bytes": resource_values["maximum"][
            "peak_address_space_bytes"
        ],
    }
    patched_readiness = readiness_source
    for value_name, pin_name in OBSERVATION_PINS.items():
        patched_readiness = _replace_zero_integer(
            patched_readiness, pin_name, "u64", observations[value_name]
        )
    patched_readiness = _replace_zero_digest(
        patched_readiness,
        RESOURCE_CERTIFICATE_PIN,
        resource_values["certificate_sha256"],
    )

    patched_profile_bytes = patched_profile.encode()
    patched_readiness_bytes = patched_readiness.encode()
    profile_mode = stat.S_IMODE(profile_path.stat().st_mode)
    readiness_mode = stat.S_IMODE(readiness_path.stat().st_mode)
    manifest: dict[str, object] = {
        "schema_version": 1,
        "authenticated_origins": authenticated_origins,
        "installation_transaction": {
            "schema_version": TRANSACTION_SCHEMA_VERSION,
            "commit_marker": "manifest-created-last",
        },
        "native_capture_validation": {
            "mode": NATIVE_VALIDATION_MODE,
            "verifier_sha256": native_verifier_sha256,
            "exact12_path": EXACT12_RELATIVE_PATH.as_posix(),
            "exact12_sha256": _sha256(exact12_bytes),
        },
        "profile_source": PROFILE_RELATIVE_PATH.as_posix(),
        "readiness_certificate_source": READINESS_RELATIVE_PATH.as_posix(),
        "expectations_norito": {
            "path": EXPECTATIONS_NORITO_RELATIVE_PATH.as_posix(),
            "sha256": expectation_norito_sha256,
        },
        "expectations_json": {
            "path": EXPECTATIONS_JSON_RELATIVE_PATH.as_posix(),
            "sha256": expectation_json_sha256,
        },
        "x509_resource_norito": {
            "path": RESOURCE_NORITO_RELATIVE_PATH.as_posix(),
            "sha256": _sha256(resource_norito),
        },
        "x509_resource_json": {
            "path": RESOURCE_JSON_RELATIVE_PATH.as_posix(),
            "sha256": _sha256(resource_json),
        },
        "x509_resource_certificate": resource_values,
        "observation_pins": OBSERVATION_PINS,
        "pin_constants": {
            "kat_bytes": KAT_BYTES_PIN,
            "kat_sha256": KAT_SHA256_PIN,
            "expectations_norito": EXPECTATIONS_NORITO_PIN,
            "expectations_json": EXPECTATIONS_JSON_PIN,
            "resource_certificate_sha256": RESOURCE_CERTIFICATE_PIN,
        },
    }
    manifest_encoded = _canonical_json_document(manifest)
    if len(manifest_encoded) > MAX_INSTALL_MANIFEST_BYTES:
        raise InstallError("installation manifest exceeds its fixed size bound")
    if (
        _stable_regular_bytes(
            profile_path,
            "ZK-X509 profile source before transaction",
            MAX_SOURCE_BYTES,
        )
        != profile_bytes
        or _stable_regular_bytes(
            readiness_path,
            "X.509 readiness source before transaction",
            MAX_SOURCE_BYTES,
        )
        != readiness_bytes
    ):
        raise InstallError("X.509 source changed before transaction preparation")
    state = _build_transaction_state(
        repository=repository,
        profile_bytes=profile_bytes,
        patched_profile=patched_profile_bytes,
        profile_mode=profile_mode,
        readiness_bytes=readiness_bytes,
        patched_readiness=patched_readiness_bytes,
        readiness_mode=readiness_mode,
        targets=targets,
        captured=captured,
        manifest_path=manifest_path,
        manifest_encoded=manifest_encoded,
    )
    _prepare_transaction(
        repository=repository,
        state=state,
        profile_bytes=profile_bytes,
        readiness_bytes=readiness_bytes,
    )
    _trip_failpoint(_failpoint, "journal_ready")
    try:
        for index, (target, encoded, maximum, label) in enumerate(
            zip(targets, captured, maximums, labels)
        ):
            _atomic_create_new(
                target,
                encoded,
                0o444,
                f"installed {label}",
                failpoint=_failpoint,
                temporary_phase=f"fixture_{index}_temporary_durable",
                published_phase=f"fixture_{index}_published",
            )
            if _stable_regular_bytes(target, f"installed {label}", maximum) != encoded:
                raise InstallError(f"installed {label} differs from captured bytes")
            _trip_failpoint(_failpoint, f"fixture_{index}_installed")
        if (
            _stable_regular_bytes(
                profile_path,
                "ZK-X509 profile source before replacement",
                MAX_SOURCE_BYTES,
            )
            != profile_bytes
            or _stable_regular_bytes(
                readiness_path,
                "X.509 readiness source before replacement",
                MAX_SOURCE_BYTES,
            )
            != readiness_bytes
        ):
            raise InstallError(
                "X.509 source changed after the transaction became durable"
            )
        _atomic_replace(
            profile_path,
            patched_profile_bytes,
            profile_mode,
            "ZK-X509 profile source",
            failpoint=_failpoint,
            temporary_phase="profile_temporary_durable",
        )
        _trip_failpoint(_failpoint, "profile_installed")
        _atomic_replace(
            readiness_path,
            patched_readiness_bytes,
            readiness_mode,
            "X.509 readiness source",
            failpoint=_failpoint,
            temporary_phase="readiness_temporary_durable",
        )
        _trip_failpoint(_failpoint, "readiness_installed")
        _atomic_create_new(
            manifest_path,
            manifest_encoded,
            0o600,
            "installation commit manifest",
            failpoint=_failpoint,
            temporary_phase="manifest_temporary_durable",
            published_phase="manifest_published",
        )
        _trip_failpoint(_failpoint, "manifest_committed")
        _trip_failpoint(_failpoint, "before_transaction_cleanup")
        _finalize_transaction_directory(repository)
    except BaseException:
        recovery = _recover_transaction(
            repository=repository,
            manifest_path=manifest_path,
            profile_path=profile_path,
            readiness_path=readiness_path,
            targets=targets,
            maximums=maximums,
            failpoint=None,
        )
        if recovery == "committed":
            return manifest
        raise

    return manifest


def _parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "One-shot install of runner-captured native expectations, the typed "
            "X.509 resource certificate, and every exact compile-time pin."
        )
    )
    parser.add_argument("--repo", required=True)
    parser.add_argument("--native-verifier", required=True)
    parser.add_argument("--native-verifier-sha256", required=True)
    parser.add_argument("--exact12-matrix", required=True)
    parser.add_argument("--captured-norito", required=True)
    parser.add_argument("--captured-json", required=True)
    parser.add_argument("--captured-x509-resource-norito", required=True)
    parser.add_argument("--captured-x509-resource-json", required=True)
    parser.add_argument("--manifest-out", required=True)
    parser.add_argument("--authenticated-iroha-source-commit", required=True)
    parser.add_argument("--authenticated-iroha-signer-principal", required=True)
    parser.add_argument("--authenticated-iroha-signer-fingerprint", required=True)
    parser.add_argument(
        "--authenticated-iroha-allowed-signers-sha256", required=True
    )
    parser.add_argument("--authenticated-validator-source-commit", required=True)
    parser.add_argument(
        "--authenticated-validator-signer-principal", required=True
    )
    parser.add_argument(
        "--authenticated-validator-signer-fingerprint", required=True
    )
    parser.add_argument(
        "--authenticated-validator-allowed-signers-sha256", required=True
    )
    parser.add_argument(
        "--authenticated-validator-source-tree-sha256", required=True
    )
    parser.add_argument(
        "--authenticated-bootstrap-source-tree-sha256", required=True
    )
    parser.add_argument("--authenticated-cargo-lock-sha256", required=True)
    parser.add_argument(
        "--authenticated-rust-toolchain-tree-sha256", required=True
    )
    return parser.parse_args()


def main() -> int:
    """Run the strict one-shot installer."""

    arguments = _parse_arguments()
    try:
        repository = _canonical_existing_directory(arguments.repo, "repository")
        native_verifier = _outside_repository(
            Path(arguments.native_verifier), repository, "native capture verifier"
        )
        exact12_path = Path(arguments.exact12_matrix)
        if not exact12_path.is_absolute():
            raise InstallError("exact12 matrix must be an absolute path")
        canonical_exact12_path = exact12_path.resolve(strict=True)
        if (
            canonical_exact12_path != exact12_path
            or canonical_exact12_path != repository / EXACT12_RELATIVE_PATH
        ):
            raise InstallError(
                "exact12 matrix must be the canonical first-release repository fixture"
            )
        captures = [
            _outside_repository(Path(raw), repository, label)
            for raw, label in (
                (arguments.captured_norito, "captured expectations Norito"),
                (arguments.captured_json, "captured expectations JSON"),
                (
                    arguments.captured_x509_resource_norito,
                    "captured X.509 resource Norito",
                ),
                (
                    arguments.captured_x509_resource_json,
                    "captured X.509 resource JSON",
                ),
            )
        ]
        manifest_path = Path(arguments.manifest_out)
        if not manifest_path.is_absolute():
            raise InstallError("installation manifest path must be absolute")
        if manifest_path.parent.resolve(strict=True) != manifest_path.parent:
            raise InstallError(
                "installation manifest parent must use its canonical physical path"
            )
        try:
            manifest_path.relative_to(repository)
        except ValueError:
            pass
        else:
            raise InstallError("installation manifest must be outside the checkout")
        manifest = install(
            repository=repository,
            native_verifier=native_verifier,
            native_verifier_sha256=arguments.native_verifier_sha256,
            exact12_matrix=canonical_exact12_path,
            captured_expectations_norito=captures[0],
            captured_expectations_json=captures[1],
            captured_resource_norito=captures[2],
            captured_resource_json=captures[3],
            manifest_path=manifest_path,
            authenticated_iroha_source_commit=(
                arguments.authenticated_iroha_source_commit
            ),
            authenticated_iroha_signer_principal=(
                arguments.authenticated_iroha_signer_principal
            ),
            authenticated_iroha_signer_fingerprint=(
                arguments.authenticated_iroha_signer_fingerprint
            ),
            authenticated_iroha_allowed_signers_sha256=(
                arguments.authenticated_iroha_allowed_signers_sha256
            ),
            authenticated_validator_source_commit=(
                arguments.authenticated_validator_source_commit
            ),
            authenticated_validator_signer_principal=(
                arguments.authenticated_validator_signer_principal
            ),
            authenticated_validator_signer_fingerprint=(
                arguments.authenticated_validator_signer_fingerprint
            ),
            authenticated_validator_allowed_signers_sha256=(
                arguments.authenticated_validator_allowed_signers_sha256
            ),
            authenticated_validator_source_tree_sha256=(
                arguments.authenticated_validator_source_tree_sha256
            ),
            authenticated_bootstrap_source_tree_sha256=(
                arguments.authenticated_bootstrap_source_tree_sha256
            ),
            authenticated_cargo_lock_sha256=(
                arguments.authenticated_cargo_lock_sha256
            ),
            authenticated_rust_toolchain_tree_sha256=(
                arguments.authenticated_rust_toolchain_tree_sha256
            ),
        )
    except (InstallError, OSError) as error:
        print(f"Taira privacy fixture installation failed: {error}", file=os.sys.stderr)
        return 1
    print(json.dumps(manifest, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
