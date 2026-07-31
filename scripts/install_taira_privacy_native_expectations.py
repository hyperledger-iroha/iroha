#!/usr/bin/env python3
"""Install the one-shot native privacy fixture set and exact X.509 source pins."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
from typing import Any


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
MAX_SOURCE_BYTES = 16 * 1024 * 1024
MAX_KAT_PROOF_BYTES = 8_212_538
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


def _stable_regular_bytes(path: Path, label: str, maximum_bytes: int) -> bytes:
    before = path.lstat()
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum_bytes
    ):
        raise InstallError(
            f"{label} must be one non-empty, bounded, singly linked regular file"
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


def _temporary_file(path: Path, encoded: bytes, mode: int, label: str) -> Path:
    descriptor, raw_path = tempfile.mkstemp(
        prefix=f".{path.name}.{label}.", dir=path.parent
    )
    temporary = Path(raw_path)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fchmod(stream.fileno(), mode)
            os.fsync(stream.fileno())
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    return temporary


def _sync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def install(
    *,
    repository: Path,
    captured_expectations_norito: Path,
    captured_expectations_json: Path,
    captured_resource_norito: Path,
    captured_resource_json: Path,
    manifest_path: Path,
) -> dict[str, object]:
    """Validate and atomically install the first-release native fixture set."""

    profile_path = repository / PROFILE_RELATIVE_PATH
    readiness_path = repository / READINESS_RELATIVE_PATH
    targets = (
        repository / EXPECTATIONS_NORITO_RELATIVE_PATH,
        repository / EXPECTATIONS_JSON_RELATIVE_PATH,
        repository / RESOURCE_NORITO_RELATIVE_PATH,
        repository / RESOURCE_JSON_RELATIVE_PATH,
    )
    for parent, label in (
        (profile_path.parent, "ZK-X509 profile parent"),
        (readiness_path.parent, "readiness-certificate parent"),
        (targets[0].parent, "privacy fixture parent"),
        (manifest_path.parent, "installation manifest parent"),
    ):
        if parent.resolve(strict=True) != parent:
            raise InstallError(f"{label} must use its canonical physical path")
    for target in (*targets, manifest_path):
        if os.path.lexists(target):
            raise InstallError(f"one-shot installation target already exists: {target}")

    capture_paths = (
        captured_expectations_norito,
        captured_expectations_json,
        captured_resource_norito,
        captured_resource_json,
    )
    identities = [(path.lstat().st_dev, path.lstat().st_ino) for path in capture_paths]
    if len(set(identities)) != len(identities):
        raise InstallError("captured fixture paths must not alias any inode")
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
    captured = tuple(
        _stable_regular_bytes(path, label, maximum)
        for path, label, maximum in zip(capture_paths, labels, maximums)
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

    profile_mode = stat.S_IMODE(profile_path.stat().st_mode)
    readiness_mode = stat.S_IMODE(readiness_path.stat().st_mode)
    next_profile = _temporary_file(
        profile_path, patched_profile.encode(), profile_mode, "next"
    )
    next_readiness = _temporary_file(
        readiness_path, patched_readiness.encode(), readiness_mode, "next"
    )
    old_profile = _temporary_file(profile_path, profile_bytes, profile_mode, "rollback")
    old_readiness = _temporary_file(
        readiness_path, readiness_bytes, readiness_mode, "rollback"
    )
    installed: list[Path] = []
    profile_replaced = False
    readiness_replaced = False
    try:
        for target, encoded in zip(targets, captured):
            _create_new_file(target, encoded, 0o444)
            installed.append(target)
        for target, expected, maximum, label in zip(
            targets, captured, maximums, labels
        ):
            if _stable_regular_bytes(target, f"installed {label}", maximum) != expected:
                raise InstallError(f"installed {label} differs from captured bytes")
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
                "X.509 source changed after bootstrap pins were validated"
            )
        os.replace(next_profile, profile_path)
        profile_replaced = True
        os.replace(next_readiness, readiness_path)
        readiness_replaced = True
        manifest: dict[str, object] = {
            "schema_version": 1,
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
        manifest_encoded = (
            json.dumps(manifest, indent=2, sort_keys=True, separators=(",", ": "))
            + "\n"
        ).encode()
        _create_new_file(manifest_path, manifest_encoded, 0o600)
        for parent in {profile_path.parent, readiness_path.parent, targets[0].parent}:
            _sync_directory(parent)
    except BaseException:
        if profile_replaced:
            os.replace(old_profile, profile_path)
        if readiness_replaced:
            os.replace(old_readiness, readiness_path)
        for installed_path in reversed(installed):
            installed_path.unlink(missing_ok=True)
        manifest_path.unlink(missing_ok=True)
        raise
    finally:
        for temporary in (next_profile, next_readiness, old_profile, old_readiness):
            temporary.unlink(missing_ok=True)

    return manifest


def _parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "One-shot install of runner-captured native expectations, the typed "
            "X.509 resource certificate, and every exact compile-time pin."
        )
    )
    parser.add_argument("--repo", required=True)
    parser.add_argument("--captured-norito", required=True)
    parser.add_argument("--captured-json", required=True)
    parser.add_argument("--captured-x509-resource-norito", required=True)
    parser.add_argument("--captured-x509-resource-json", required=True)
    parser.add_argument("--manifest-out", required=True)
    return parser.parse_args()


def main() -> int:
    """Run the strict one-shot installer."""

    arguments = _parse_arguments()
    try:
        repository = _canonical_existing_directory(arguments.repo, "repository")
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
            captured_expectations_norito=captures[0],
            captured_expectations_json=captures[1],
            captured_resource_norito=captures[2],
            captured_resource_json=captures[3],
            manifest_path=manifest_path,
        )
    except (InstallError, OSError) as error:
        print(f"Taira privacy fixture installation failed: {error}", file=os.sys.stderr)
        return 1
    print(json.dumps(manifest, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
