#!/usr/bin/env python3
"""Package and re-verify one native-qualified Vega Figure 9 release bundle.

This controller never generates keys and does not decide governance.  A
release operator supplies an already-built, digest-pinned native validator and
owner-only canonical key files. The native boundary must also generate and
replay the four canonical Vega release stages. The result is a
content-addressed, owner-private candidate package whose manifest keeps
activation explicitly unauthorized until a separate reviewed governance
transaction is finalized.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, BinaryIO


PACKAGE_SCHEMA = "iroha.vega.figure9.governed-artifact-package"
PACKAGE_SCHEMA_VERSION = 2
NATIVE_SCHEMA = "iroha.vega.figure9.native-artifact-validation"
NATIVE_SCHEMA_VERSION = 2
ARTIFACT_MANIFEST_SCHEMA = "iroha.vega.figure9.microsoft-mc.artifacts"
ARTIFACT_MANIFEST_SCHEMA_VERSION = 1
ARTIFACT_MANIFEST_DOMAIN = b"iroha.vega.figure9.microsoft-mc.artifact-manifest.v1\0"
MAX_KEY_BYTES = 512 * 1024 * 1024
MAX_VALIDATOR_BYTES = 256 * 1024 * 1024
MAX_NATIVE_REPORT_BYTES = 64 * 1024
MAX_EVIDENCE_ARCHIVE_BYTES = 2 * 1024 * 1024
MAX_VENDOR_SOURCE_FILE_BYTES = 16 * 1024 * 1024
MAX_VENDOR_SOURCE_TOTAL_BYTES = 64 * 1024 * 1024
MAX_VENDOR_SOURCE_FILES = 512
NATIVE_TIMEOUT_SECONDS = 7_200
EVIDENCE_SET_DOMAIN = b"iroha.vega.figure9.release-evidence-set.v1\0"
VEGA_PROTOCOL_ID = "vega-existing-credential-zk-v0"
VEGA_PROOF_BYTES_CEILING = 524_288
VEGA_PRIMARY_UNITS = 2_359_296
VEGA_SECONDARY_UNITS = 1_048_576
VEGA_RELATION_DEPTH = 21
CARGO_LOCK_SHA256 = "179f589da420c024725efd9a65adb9c1e34085fa022cc01a8c67bb2262e93bf7"

COMPILED_PROFILE_DIGEST = "e754aebc68f64401b5891983fbaeff81bc4b4d59921a72c3a62aa99a19260a2a"
CANONICAL_RELATION_DIGEST = "8bf6a311206ef6789b2b3d613b4e98b9fdc58acd02373a9dbc2b7b64cb7edfbc"
LOGICAL_GOVERNED_VERIFIER_DIGEST = (
    "86d0ce5b22f463785d07936034ddefc487461634f2fe2cfc470bf30bde3d6827"
)
UPSTREAM_SOURCE_COMMIT = "c0ee259053cd12eaf43ed71b5cde375452b3ee4d"
UPSTREAM_SOURCE_TREE = "7226b6cbfbfe8613dd2d5ee831096b7578a5c115"
VENDOR_MANIFEST_SHA256 = "539c54251c8853fa99673e71d777966a3e3e238e64028d47b3e683329023236f"
VENDOR_SOURCE_ROOT = Path(__file__).resolve().parents[1] / "vendor" / "vega-prover"
VENDOR_PROVENANCE_FILE = "IROHA_PROVENANCE.md"

NATIVE_VALIDATOR_FILE = "vega-figure9-artifact-tool"
PROVING_KEY_FILE = "proving-key.bin"
VERIFIER_KEY_FILE = "verifier-key.bin"
NATIVE_REPORT_FILE = "native-validation.json"
MANIFEST_FILE = "manifest.json"
EVIDENCE_FILES = (
    (
        "positive-canonical-end-to-end",
        "vega-evidence-16-positive-canonical-end-to-end.norito",
        "not-applicable",
    ),
    (
        "public-statement-binding-mutation",
        "vega-evidence-17-public-statement-binding-mutation.norito",
        "public-statement-binding-rejected",
    ),
    (
        "proof-corruption-and-truncation",
        "vega-evidence-18-proof-corruption-and-truncation.norito",
        "canonical-wire-corruption-and-truncation-rejected",
    ),
    (
        "maximum-shape-resource",
        "vega-evidence-19-maximum-shape-resource.norito",
        "not-applicable",
    ),
)
EVIDENCE_FILE_NAMES = frozenset(file_name for _, file_name, _ in EVIDENCE_FILES)
PACKAGE_FILES = frozenset(
    {
        NATIVE_VALIDATOR_FILE,
        PROVING_KEY_FILE,
        VERIFIER_KEY_FILE,
        NATIVE_REPORT_FILE,
        MANIFEST_FILE,
        *EVIDENCE_FILE_NAMES,
    }
)
LOWER_HEX_64 = re.compile(r"[0-9a-f]{64}\Z")
LOWER_HEX_40 = re.compile(r"[0-9a-f]{40}\Z")
PLATFORM_TOKEN = re.compile(r"[a-z0-9_+.-]{1,64}\Z")

NATIVE_KEYS = frozenset(
    {
        "artifact_manifest_schema",
        "artifact_manifest_schema_version",
        "artifact_manifest_sha256",
        "canonical_relation_digest",
        "cargo_lock_sha256",
        "compiled_profile_digest",
        "evidence",
        "evidence_set_sha256",
        "iroha_signed_source_commit",
        "logical_governed_verifier_digest",
        "proving_key",
        "release_qualification",
        "schema",
        "schema_version",
        "source_allowed_signers_sha256",
        "source_revocation_sha256",
        "upstream_source_commit",
        "upstream_source_tree",
        "validator_arch",
        "validator_os",
        "validator_role",
        "vendor_manifest_sha256",
        "verifier_key",
        "workspace_source_manifest_sha256",
    }
)
ARTIFACT_KEYS = frozenset({"exact_byte_len", "raw_canonical_sha256", "role"})
EVIDENCE_KEYS = frozenset(
    {
        "archive_sha256",
        "case_kind",
        "exact_byte_len",
        "file_name",
        "failure_class",
        "proof_artifacts",
        "protocol_id",
        "public_statement_sha256",
        "resources",
        "stage_ordinal",
    }
)
EVIDENCE_PROOF_KEYS = frozenset(
    {
        "artifact_ordinal",
        "canonical_proof_exact_byte_len",
        "proof_bytes_ceiling",
        "proof_sha256",
    }
)
EVIDENCE_RESOURCE_KEYS = frozenset(
    {
        "primary_ceiling",
        "primary_units",
        "relation_depth",
        "relation_depth_ceiling",
        "secondary_ceiling",
        "secondary_units",
    }
)
PACKAGE_KEYS = frozenset(
    {
        "availability",
        "files",
        "native_validation",
        "network_activation_authorized",
        "native_release_qualification",
        "release_boundary",
        "schema",
        "schema_version",
    }
)
FILE_KEYS = frozenset({"mode", "path", "sha256", "size"})


class Refusal(RuntimeError):
    """Fail-closed release input or package validation error."""


@dataclass(frozen=True)
class FileIdentity:
    path: Path
    size: int
    sha256: str
    mode: int

    def manifest_value(self, package_name: str, packaged_mode: int) -> dict[str, Any]:
        return {
            "mode": f"{packaged_mode:04o}",
            "path": package_name,
            "sha256": self.sha256,
            "size": self.size,
        }


def _canonical_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True) + "\n"
    ).encode("ascii")


def _strict_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise Refusal(f"JSON contains duplicate key {key!r}")
        result[key] = value
    return result


def _strict_json(data: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(data, object_pairs_hook=_strict_object)
    except (UnicodeDecodeError, json.JSONDecodeError, Refusal) as error:
        raise Refusal(f"{label} is not strict JSON: {error}") from error
    if not isinstance(value, dict):
        raise Refusal(f"{label} must be one JSON object")
    if data != _canonical_json(value):
        raise Refusal(f"{label} is not canonical sorted compact JSON")
    return value


def _require_keys(value: dict[str, Any], expected: frozenset[str], label: str) -> None:
    actual = frozenset(value)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise Refusal(f"{label} schema mismatch; missing={missing}, extra={extra}")


def _digest(value: Any, label: str) -> str:
    if not isinstance(value, str) or not LOWER_HEX_64.fullmatch(value) or value == "0" * 64:
        raise Refusal(f"{label} must be one nonzero lowercase SHA-256")
    return value


def _commit(value: Any, label: str) -> str:
    if not isinstance(value, str) or not LOWER_HEX_40.fullmatch(value) or value == "0" * 40:
        raise Refusal(f"{label} must be one nonzero lowercase commit identity")
    return value


def _u64(value: Any, label: str, maximum: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0 or value > maximum:
        raise Refusal(f"{label} is outside 1..={maximum}")
    return value


def _exact_integer(value: Any, expected: int, label: str) -> int:
    """Require one exact JSON integer, excluding booleans and numeric lookalikes."""
    if isinstance(value, bool) or not isinstance(value, int) or value != expected:
        raise Refusal(f"{label} must be the exact integer {expected}")
    return value


def _manifest_digest(report: dict[str, Any]) -> str:
    schema = report["artifact_manifest_schema"].encode("ascii")
    data = bytearray(ARTIFACT_MANIFEST_DOMAIN)
    data.extend(len(schema).to_bytes(8, "big"))
    data.extend(schema)
    data.extend(report["artifact_manifest_schema_version"].to_bytes(2, "big"))
    data.extend(bytes.fromhex(report["compiled_profile_digest"]))
    data.extend(bytes.fromhex(report["canonical_relation_digest"]))
    data.extend(report["upstream_source_commit"].encode("ascii"))
    data.extend(report["upstream_source_tree"].encode("ascii"))
    data.extend(bytes.fromhex(report["vendor_manifest_sha256"]))
    data.extend(bytes.fromhex(report["logical_governed_verifier_digest"]))
    for role_number, field in ((1, "proving_key"), (2, "verifier_key")):
        artifact = report[field]
        data.append(role_number)
        data.extend(artifact["exact_byte_len"].to_bytes(8, "big"))
        data.extend(bytes.fromhex(artifact["raw_canonical_sha256"]))
    return hashlib.sha256(data).hexdigest()


def _evidence_set_digest(evidence: list[dict[str, Any]]) -> str:
    digest = hashlib.sha256()
    digest.update(EVIDENCE_SET_DOMAIN)
    for archive in evidence:
        file_name = archive["file_name"].encode("ascii")
        digest.update(archive["stage_ordinal"].to_bytes(2, "big"))
        digest.update(len(file_name).to_bytes(8, "big"))
        digest.update(file_name)
        digest.update(archive["exact_byte_len"].to_bytes(8, "big"))
        digest.update(bytes.fromhex(archive["archive_sha256"]))
    return digest.hexdigest()


def _validate_evidence_summary(evidence: Any) -> list[dict[str, Any]]:
    if not isinstance(evidence, list) or len(evidence) != len(EVIDENCE_FILES):
        raise Refusal("native validation must contain the exact four Vega evidence stages")
    validated: list[dict[str, Any]] = []
    for index, (archive, expected) in enumerate(zip(evidence, EVIDENCE_FILES, strict=True)):
        case_kind, file_name, failure_class = expected
        label = f"native validation evidence[{index}]"
        if not isinstance(archive, dict):
            raise Refusal(f"{label} must be an object")
        _require_keys(archive, EVIDENCE_KEYS, label)
        _exact_integer(archive["stage_ordinal"], 16 + index, f"{label} stage ordinal")
        if (
            archive["case_kind"] != case_kind
            or archive["file_name"] != file_name
            or archive["failure_class"] != failure_class
            or archive["protocol_id"] != VEGA_PROTOCOL_ID
        ):
            raise Refusal(f"{label} differs from the closed Vega coordinate")
        _u64(archive["exact_byte_len"], f"{label} archive length", MAX_EVIDENCE_ARCHIVE_BYTES)
        _digest(archive["archive_sha256"], f"{label} archive SHA-256")
        _digest(archive["public_statement_sha256"], f"{label} statement SHA-256")

        proofs = archive["proof_artifacts"]
        if not isinstance(proofs, list) or len(proofs) != 1:
            raise Refusal(f"{label} must contain exactly one canonical proof artifact")
        proof = proofs[0]
        if not isinstance(proof, dict):
            raise Refusal(f"{label} proof artifact must be an object")
        _require_keys(proof, EVIDENCE_PROOF_KEYS, f"{label} proof artifact")
        _exact_integer(proof["artifact_ordinal"], 0, f"{label} proof artifact ordinal")
        _exact_integer(
            proof["proof_bytes_ceiling"],
            VEGA_PROOF_BYTES_CEILING,
            f"{label} proof decoder ceiling",
        )
        _u64(
            proof["canonical_proof_exact_byte_len"],
            f"{label} proof length",
            VEGA_PROOF_BYTES_CEILING,
        )
        _digest(proof["proof_sha256"], f"{label} proof SHA-256")

        resources = archive["resources"]
        if not isinstance(resources, dict):
            raise Refusal(f"{label} resources must be an object")
        _require_keys(resources, EVIDENCE_RESOURCE_KEYS, f"{label} resources")
        expected_resources = {
            "primary_ceiling": VEGA_PRIMARY_UNITS,
            "primary_units": VEGA_PRIMARY_UNITS,
            "relation_depth": VEGA_RELATION_DEPTH,
            "relation_depth_ceiling": VEGA_RELATION_DEPTH,
            "secondary_ceiling": VEGA_SECONDARY_UNITS,
            "secondary_units": VEGA_SECONDARY_UNITS,
        }
        for field, expected in expected_resources.items():
            _exact_integer(resources[field], expected, f"{label} resources {field}")
        validated.append(archive)
    return validated


def _validate_native_report(report: dict[str, Any]) -> None:
    _require_keys(report, NATIVE_KEYS, "native validation report")
    _exact_integer(report["schema_version"], NATIVE_SCHEMA_VERSION, "native validation schema version")
    if report["schema"] != NATIVE_SCHEMA:
        raise Refusal("native validation report schema/version mismatch")
    _exact_integer(
        report["artifact_manifest_schema_version"],
        ARTIFACT_MANIFEST_SCHEMA_VERSION,
        "native artifact-manifest schema version",
    )
    if (
        report["artifact_manifest_schema"] != ARTIFACT_MANIFEST_SCHEMA
    ):
        raise Refusal("native artifact-manifest schema/version mismatch")
    expected_constants = {
        "canonical_relation_digest": CANONICAL_RELATION_DIGEST,
        "cargo_lock_sha256": CARGO_LOCK_SHA256,
        "compiled_profile_digest": COMPILED_PROFILE_DIGEST,
        "logical_governed_verifier_digest": LOGICAL_GOVERNED_VERIFIER_DIGEST,
        "upstream_source_commit": UPSTREAM_SOURCE_COMMIT,
        "upstream_source_tree": UPSTREAM_SOURCE_TREE,
        "vendor_manifest_sha256": VENDOR_MANIFEST_SHA256,
        "release_qualification": "passed-native-four-case",
        "validator_role": "prover-pair-and-four-case-release-evidence",
    }
    for field, expected in expected_constants.items():
        if report[field] != expected:
            raise Refusal(f"native validation report {field} differs from the released profile")
    evidence = _validate_evidence_summary(report["evidence"])
    _digest(report["evidence_set_sha256"], "native validation evidence-set SHA-256")
    if _evidence_set_digest(evidence) != report["evidence_set_sha256"]:
        raise Refusal("native validation evidence-set SHA-256 is not reproducible")
    for field in (
        "artifact_manifest_sha256",
        "cargo_lock_sha256",
        "source_allowed_signers_sha256",
        "source_revocation_sha256",
        "workspace_source_manifest_sha256",
    ):
        _digest(report[field], f"native validation {field}")
    _commit(report["iroha_signed_source_commit"], "native validation signed source commit")
    for field in ("validator_arch", "validator_os"):
        if not isinstance(report[field], str) or not PLATFORM_TOKEN.fullmatch(report[field]):
            raise Refusal(f"native validation {field} is not a bounded platform token")
    for field, role in (("proving_key", "proving-key"), ("verifier_key", "verifier-key")):
        artifact = report[field]
        if not isinstance(artifact, dict):
            raise Refusal(f"native validation {field} must be an object")
        _require_keys(artifact, ARTIFACT_KEYS, f"native validation {field}")
        if artifact["role"] != role:
            raise Refusal(f"native validation {field} role mismatch")
        _u64(artifact["exact_byte_len"], f"native validation {field} length", MAX_KEY_BYTES)
        _digest(artifact["raw_canonical_sha256"], f"native validation {field} SHA-256")
    if report["proving_key"]["raw_canonical_sha256"] == report["verifier_key"]["raw_canonical_sha256"]:
        raise Refusal("native validation report aliases the PK and VK identities")
    if _manifest_digest(report) != report["artifact_manifest_sha256"]:
        raise Refusal("native artifact manifest SHA-256 is not reproducible")


def _canonical_existing_path(path: Path, label: str, *, directory: bool = False) -> Path:
    if not path.is_absolute():
        raise Refusal(f"{label} path must be absolute")
    try:
        if path.is_symlink():
            raise Refusal(f"{label} must not be a symbolic link")
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise Refusal(f"cannot resolve {label}: {error}") from error
    if resolved != path:
        raise Refusal(f"{label} path must already be canonical")
    metadata = path.stat(follow_symlinks=False)
    if directory != stat.S_ISDIR(metadata.st_mode):
        expected = "directory" if directory else "regular file"
        raise Refusal(f"{label} must be one {expected}")
    if not directory and (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1):
        raise Refusal(f"{label} must be a singly linked regular file")
    if stat.S_IMODE(metadata.st_mode) & 0o077:
        raise Refusal(f"{label} must be owner-only")
    return resolved


def _open_nofollow(path: Path) -> BinaryIO:
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    return os.fdopen(descriptor, "rb", closefd=True)


def _vendor_source_file_sha256(path: Path, label: str) -> tuple[str, int]:
    before = path.stat(follow_symlinks=False)
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
        raise Refusal(f"{label} must be a singly linked regular file")
    if before.st_size > MAX_VENDOR_SOURCE_FILE_BYTES:
        raise Refusal(
            f"{label} exceeds the {MAX_VENDOR_SOURCE_FILE_BYTES}-byte source-file bound"
        )
    digest = hashlib.sha256()
    observed = 0
    with _open_nofollow(path) as source:
        opened = os.fstat(source.fileno())
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise Refusal(f"{label} identity changed before authentication")
        while True:
            chunk = source.read(1024 * 1024)
            if not chunk:
                break
            observed += len(chunk)
            if observed > MAX_VENDOR_SOURCE_FILE_BYTES:
                raise Refusal(
                    f"{label} grew beyond the {MAX_VENDOR_SOURCE_FILE_BYTES}-byte source-file bound"
                )
            digest.update(chunk)
        after = os.fstat(source.fileno())
    if (
        observed != before.st_size
        or (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_mode)
        != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode)
    ):
        raise Refusal(f"{label} changed while it was being authenticated")
    return digest.hexdigest(), observed


def _vendor_source_manifest_sha256(root: Path) -> str:
    """Reproduce the reviewed manifest algorithm documented by the vendor tree."""
    try:
        if root.is_symlink():
            raise Refusal("vendored Vega source root must not be a symbolic link")
        resolved = root.resolve(strict=True)
    except OSError as error:
        raise Refusal(f"cannot resolve vendored Vega source root: {error}") from error
    if resolved != root or not resolved.is_dir():
        raise Refusal("vendored Vega source root must be one canonical directory")

    entries: list[tuple[bytes, str]] = []
    total_bytes = 0
    for candidate in resolved.rglob("*"):
        relative = candidate.relative_to(resolved)
        if relative == Path(VENDOR_PROVENANCE_FILE):
            continue
        metadata = candidate.stat(follow_symlinks=False)
        if stat.S_ISDIR(metadata.st_mode):
            continue
        if not stat.S_ISREG(metadata.st_mode):
            raise Refusal(f"vendored Vega source contains a non-regular path: {relative.as_posix()}")
        if len(entries) >= MAX_VENDOR_SOURCE_FILES:
            raise Refusal(
                f"vendored Vega source exceeds the {MAX_VENDOR_SOURCE_FILES}-file manifest bound"
            )
        relative_bytes = relative.as_posix().encode("utf-8")
        if b"\0" in relative_bytes or b"\n" in relative_bytes or b"\r" in relative_bytes:
            raise Refusal("vendored Vega source contains an unsafe manifest path")
        file_sha256, file_bytes = _vendor_source_file_sha256(
            candidate, f"vendored Vega source {relative.as_posix()}"
        )
        total_bytes += file_bytes
        if total_bytes > MAX_VENDOR_SOURCE_TOTAL_BYTES:
            raise Refusal(
                "vendored Vega source exceeds the bounded aggregate source-manifest size"
            )
        entries.append((relative_bytes, file_sha256))

    if not entries:
        raise Refusal("vendored Vega source manifest is empty")
    manifest = hashlib.sha256()
    for relative_bytes, file_sha256 in sorted(entries):
        manifest.update(file_sha256.encode("ascii"))
        manifest.update(b"  ")
        manifest.update(relative_bytes)
        manifest.update(b"\n")
    return manifest.hexdigest()


def _require_reviewed_vendor_source_manifest() -> None:
    first = _vendor_source_manifest_sha256(VENDOR_SOURCE_ROOT)
    second = _vendor_source_manifest_sha256(VENDOR_SOURCE_ROOT)
    if first != second:
        raise Refusal("vendored Vega source changed while reproducing its reviewed manifest")
    if first != VENDOR_MANIFEST_SHA256:
        raise Refusal(
            "vendored Vega source manifest does not reproduce the reviewed digest; "
            f"observed {first}"
        )


def _file_identity(
    path: Path,
    label: str,
    maximum: int,
    *,
    executable: bool = False,
) -> FileIdentity:
    path = _canonical_existing_path(path, label)
    before = path.stat(follow_symlinks=False)
    mode = stat.S_IMODE(before.st_mode)
    if executable and not mode & stat.S_IXUSR:
        raise Refusal(f"{label} must be owner-executable")
    if not executable and mode & 0o111:
        raise Refusal(f"{label} must not be executable")
    if before.st_size <= 0 or before.st_size > maximum:
        raise Refusal(f"{label} length is outside 1..={maximum}")
    digest = hashlib.sha256()
    observed = 0
    with _open_nofollow(path) as source:
        opened = os.fstat(source.fileno())
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise Refusal(f"{label} identity changed before authentication")
        while True:
            chunk = source.read(1024 * 1024)
            if not chunk:
                break
            observed += len(chunk)
            if observed > maximum:
                raise Refusal(f"{label} grew beyond its absolute byte bound")
            digest.update(chunk)
        after = os.fstat(source.fileno())
    if (
        observed != before.st_size
        or (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_mode)
        != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode)
    ):
        raise Refusal(f"{label} changed while it was being authenticated")
    return FileIdentity(path=path, size=observed, sha256=digest.hexdigest(), mode=mode)


def _run_native_validator(
    validator: FileIdentity,
    proving_key: FileIdentity,
    verifier_key: FileIdentity,
    evidence_output: Path,
) -> tuple[dict[str, Any], bytes, dict[str, FileIdentity]]:
    evidence_output = _canonical_existing_path(
        evidence_output, "native Vega evidence output", directory=True
    )
    if any(evidence_output.iterdir()):
        raise Refusal("native Vega evidence output must be empty")
    clean_environment = {
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
    }
    command = [
        str(validator.path),
        "qualify-prover-release",
        "--proving-key",
        str(proving_key.path),
        "--verifier-key",
        str(verifier_key.path),
        "--evidence-output",
        str(evidence_output),
    ]
    try:
        completed = subprocess.run(
            command,
            cwd="/",
            env=clean_environment,
            check=False,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=NATIVE_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise Refusal(f"native Vega artifact validation could not complete: {error}") from error
    if len(completed.stdout) > MAX_NATIVE_REPORT_BYTES or len(completed.stderr) > MAX_NATIVE_REPORT_BYTES:
        raise Refusal("native Vega artifact validator exceeded its output bound")
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", "replace").strip()
        raise Refusal(f"native Vega artifact validator rejected the pair: {diagnostic}")
    if completed.stderr:
        raise Refusal("successful native Vega artifact validation wrote diagnostics to stderr")
    report = _strict_json(completed.stdout, "native validation report")
    _validate_native_report(report)
    for field, identity in (("proving_key", proving_key), ("verifier_key", verifier_key)):
        artifact = report[field]
        if artifact["exact_byte_len"] != identity.size or artifact["raw_canonical_sha256"] != identity.sha256:
            raise Refusal(f"native validation {field} identity differs from the supplied file")
    inventory = {entry.name for entry in evidence_output.iterdir()}
    if inventory != EVIDENCE_FILE_NAMES or any(entry.is_symlink() for entry in evidence_output.iterdir()):
        raise Refusal("native Vega evidence output inventory is not exact")
    identities: dict[str, FileIdentity] = {}
    summaries = {archive["file_name"]: archive for archive in report["evidence"]}
    for file_name in EVIDENCE_FILE_NAMES:
        identity = _file_identity(
            (evidence_output / file_name).resolve(strict=True),
            f"native Vega evidence {file_name}",
            MAX_EVIDENCE_ARCHIVE_BYTES,
        )
        if identity.mode != 0o400:
            raise Refusal(f"native Vega evidence {file_name} must have exact mode 0400")
        summary = summaries[file_name]
        if identity.size != summary["exact_byte_len"] or identity.sha256 != summary["archive_sha256"]:
            raise Refusal(f"native Vega evidence {file_name} differs from its native summary")
        identities[file_name] = identity
    return report, completed.stdout, identities


def _write_bytes(path: Path, data: bytes, mode: int) -> FileIdentity:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, mode)
    try:
        with os.fdopen(descriptor, "wb", closefd=True) as destination:
            destination.write(data)
            destination.flush()
            os.fsync(destination.fileno())
    except BaseException:
        try:
            os.close(descriptor)
        except OSError:
            pass
        raise
    os.chmod(path, mode, follow_symlinks=False)
    return _file_identity(path.resolve(strict=True), path.name, len(data), executable=bool(mode & 0o100))


def _copy_file(source: FileIdentity, destination: Path, mode: int, label: str) -> FileIdentity:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(destination, flags, mode)
    digest = hashlib.sha256()
    observed = 0
    try:
        with _open_nofollow(source.path) as input_file, os.fdopen(
            descriptor, "wb", closefd=True
        ) as output_file:
            opened = os.fstat(input_file.fileno())
            while True:
                chunk = input_file.read(1024 * 1024)
                if not chunk:
                    break
                observed += len(chunk)
                digest.update(chunk)
                output_file.write(chunk)
            output_file.flush()
            os.fsync(output_file.fileno())
            closed = os.fstat(input_file.fileno())
        if (
            observed != source.size
            or digest.hexdigest() != source.sha256
            or (opened.st_dev, opened.st_ino, opened.st_size, opened.st_mtime_ns, opened.st_mode)
            != (closed.st_dev, closed.st_ino, closed.st_size, closed.st_mtime_ns, closed.st_mode)
        ):
            raise Refusal(f"{label} changed while it was being copied")
    except BaseException:
        try:
            os.close(descriptor)
        except OSError:
            pass
        raise
    os.chmod(destination, mode, follow_symlinks=False)
    copied = _file_identity(
        destination.resolve(strict=True),
        f"packaged {label}",
        max(source.size, 1),
        executable=bool(mode & 0o100),
    )
    if (copied.size, copied.sha256) != (source.size, source.sha256):
        raise Refusal(f"packaged {label} differs from the authenticated source")
    return copied


def _files_equal(left: FileIdentity, right: FileIdentity) -> bool:
    if (left.size, left.sha256) != (right.size, right.sha256):
        return False
    with _open_nofollow(left.path) as left_file, _open_nofollow(right.path) as right_file:
        while True:
            left_chunk = left_file.read(1024 * 1024)
            right_chunk = right_file.read(1024 * 1024)
            if left_chunk != right_chunk:
                return False
            if not left_chunk:
                return True


def _manifest_file(
    value: Any,
    label: str,
    expected_path: str,
    expected_mode: str,
    maximum: int,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise Refusal(f"package {label} file identity must be an object")
    _require_keys(value, FILE_KEYS, f"package {label} file identity")
    if value["path"] != expected_path or value["mode"] != expected_mode:
        raise Refusal(f"package {label} path/mode mismatch")
    _u64(value["size"], f"package {label} size", maximum)
    _digest(value["sha256"], f"package {label} SHA-256")
    return value


def _candidate_manifest(
    report: dict[str, Any],
    native_validator: FileIdentity,
    proving_key: FileIdentity,
    verifier_key: FileIdentity,
    native_report: FileIdentity,
    evidence: dict[str, FileIdentity],
) -> dict[str, Any]:
    """Construct the sole candidate-only governed package schema."""
    return {
        "availability": "unavailable-pending-reviewed-governance",
        "files": {
            "native_report": native_report.manifest_value(NATIVE_REPORT_FILE, 0o400),
            "native_validator": native_validator.manifest_value(NATIVE_VALIDATOR_FILE, 0o500),
            "proving_key": proving_key.manifest_value(PROVING_KEY_FILE, 0o400),
            "verifier_key": verifier_key.manifest_value(VERIFIER_KEY_FILE, 0o400),
            **{
                file_name: evidence[file_name].manifest_value(file_name, 0o400)
                for _, file_name, _ in EVIDENCE_FILES
            },
        },
        "native_validation": report,
        "network_activation_authorized": False,
        "native_release_qualification": "passed-native-four-case",
        "release_boundary": "candidate-only",
        "schema": PACKAGE_SCHEMA,
        "schema_version": PACKAGE_SCHEMA_VERSION,
    }


def _validate_package_manifest(
    manifest: dict[str, Any],
) -> tuple[dict[str, Any], dict[str, dict[str, Any]]]:
    """Validate the closed public schema before consulting packaged files."""
    _require_keys(manifest, PACKAGE_KEYS, "Vega package manifest")
    _exact_integer(manifest["schema_version"], PACKAGE_SCHEMA_VERSION, "Vega package schema version")
    if (
        manifest["schema"] != PACKAGE_SCHEMA
        or manifest["availability"] != "unavailable-pending-reviewed-governance"
        or manifest["network_activation_authorized"] is not False
        or manifest["native_release_qualification"] != "passed-native-four-case"
        or manifest["release_boundary"] != "candidate-only"
    ):
        raise Refusal("Vega package release boundary or schema is not fail closed")
    report = manifest["native_validation"]
    if not isinstance(report, dict):
        raise Refusal("Vega package native validation must be an object")
    _validate_native_report(report)
    files = manifest["files"]
    expected_files = {
        "native_report",
        "native_validator",
        "proving_key",
        "verifier_key",
        *EVIDENCE_FILE_NAMES,
    }
    if not isinstance(files, dict) or frozenset(files) != frozenset(expected_files):
        raise Refusal("Vega package file manifest is not closed")
    identities = {
        "native_validator": _manifest_file(
            files["native_validator"],
            "native validator",
            NATIVE_VALIDATOR_FILE,
            "0500",
            MAX_VALIDATOR_BYTES,
        ),
        "proving_key": _manifest_file(
            files["proving_key"], "proving key", PROVING_KEY_FILE, "0400", MAX_KEY_BYTES
        ),
        "verifier_key": _manifest_file(
            files["verifier_key"], "verifier key", VERIFIER_KEY_FILE, "0400", MAX_KEY_BYTES
        ),
        "native_report": _manifest_file(
            files["native_report"],
            "native report",
            NATIVE_REPORT_FILE,
            "0400",
            MAX_NATIVE_REPORT_BYTES,
        ),
        **{
            file_name: _manifest_file(
                files[file_name],
                f"evidence {file_name}",
                file_name,
                "0400",
                MAX_EVIDENCE_ARCHIVE_BYTES,
            )
            for _, file_name, _ in EVIDENCE_FILES
        },
    }
    return report, identities


def verify_package(package: Path, expected_package_sha256: str) -> dict[str, Any]:
    expected_package_sha256 = _digest(expected_package_sha256, "expected package SHA-256")
    package = _canonical_existing_path(package, "Vega package", directory=True)
    if package.name != expected_package_sha256:
        raise Refusal("Vega package directory is not named by the expected package digest")
    if stat.S_IMODE(package.stat(follow_symlinks=False).st_mode) != 0o500:
        raise Refusal("Vega package directory must have exact mode 0500")
    inventory = {entry.name for entry in package.iterdir()}
    if inventory != PACKAGE_FILES or any(entry.is_symlink() for entry in package.iterdir()):
        raise Refusal("Vega package file inventory is not exact or contains a symbolic link")
    manifest_path = package / MANIFEST_FILE
    manifest_identity = _file_identity(
        manifest_path.resolve(strict=True), "packaged manifest", MAX_NATIVE_REPORT_BYTES
    )
    if manifest_identity.mode != 0o400:
        raise Refusal("Vega package manifest must have exact mode 0400")
    with _open_nofollow(manifest_path) as manifest_file:
        manifest_bytes = manifest_file.read(MAX_NATIVE_REPORT_BYTES + 1)
    if hashlib.sha256(manifest_bytes).hexdigest() != expected_package_sha256:
        raise Refusal("Vega package manifest differs from the expected package digest")
    manifest = _strict_json(manifest_bytes, "Vega package manifest")
    report, declared_identities = _validate_package_manifest(manifest)
    identities = {
        "native_validator": _file_identity(
            (package / NATIVE_VALIDATOR_FILE).resolve(strict=True),
            "packaged native validator",
            MAX_VALIDATOR_BYTES,
            executable=True,
        ),
        "proving_key": _file_identity(
            (package / PROVING_KEY_FILE).resolve(strict=True),
            "packaged proving key",
            MAX_KEY_BYTES,
        ),
        "verifier_key": _file_identity(
            (package / VERIFIER_KEY_FILE).resolve(strict=True),
            "packaged verifier key",
            MAX_KEY_BYTES,
        ),
        "native_report": _file_identity(
            (package / NATIVE_REPORT_FILE).resolve(strict=True),
            "packaged native report",
            MAX_NATIVE_REPORT_BYTES,
        ),
        **{
            file_name: _file_identity(
                (package / file_name).resolve(strict=True),
                f"packaged evidence {file_name}",
                MAX_EVIDENCE_ARCHIVE_BYTES,
            )
            for _, file_name, _ in EVIDENCE_FILES
        },
    }
    for name, declared in declared_identities.items():
        identity = identities[name]
        if identity.size != declared["size"] or identity.sha256 != declared["sha256"]:
            raise Refusal(f"packaged {name} differs from its manifest identity")
    expected_modes = {
        "native_validator": 0o500,
        "proving_key": 0o400,
        "verifier_key": 0o400,
        "native_report": 0o400,
        **{file_name: 0o400 for _, file_name, _ in EVIDENCE_FILES},
    }
    for name, expected_mode in expected_modes.items():
        if identities[name].mode != expected_mode:
            raise Refusal(f"packaged {name} must have exact mode {expected_mode:04o}")
    with _open_nofollow(package / NATIVE_REPORT_FILE) as report_file:
        report_bytes = report_file.read(MAX_NATIVE_REPORT_BYTES + 1)
    if report_bytes != _canonical_json(report):
        raise Refusal("packaged native validation report differs from the manifest")
    with tempfile.TemporaryDirectory(prefix="iroha-vega-verify-") as temporary:
        evidence_output = Path(temporary).resolve(strict=True)
        os.chmod(evidence_output, 0o700)
        fresh_report, fresh_bytes, fresh_evidence = _run_native_validator(
            identities["native_validator"],
            identities["proving_key"],
            identities["verifier_key"],
            evidence_output,
        )
        if fresh_report != report or fresh_bytes != report_bytes:
            raise Refusal("packaged native validation replay differs from the sealed report")
        for _, file_name, _ in EVIDENCE_FILES:
            if not _files_equal(fresh_evidence[file_name], identities[file_name]):
                raise Refusal(f"packaged Vega evidence {file_name} differs from native replay")
    return manifest


def package(
    native_validator_path: Path,
    expected_native_validator_sha256: str,
    proving_key_path: Path,
    verifier_key_path: Path,
    output_root: Path,
) -> tuple[Path, str]:
    expected_native_validator_sha256 = _digest(
        expected_native_validator_sha256, "expected native validator SHA-256"
    )
    output_root = _canonical_existing_path(output_root, "Vega output root", directory=True)
    validator = _file_identity(
        native_validator_path, "native Vega artifact validator", MAX_VALIDATOR_BYTES, executable=True
    )
    if validator.sha256 != expected_native_validator_sha256:
        raise Refusal("native Vega artifact validator differs from its reviewed digest")
    # A compiled-in vendor digest is not source provenance by itself. Reproduce
    # the reviewed seal before looking up either operator-supplied key so stale
    # or undeclared vendor drift cannot enter a candidate package.
    _require_reviewed_vendor_source_manifest()
    proving_key = _file_identity(proving_key_path, "Vega proving key", MAX_KEY_BYTES)
    verifier_key = _file_identity(verifier_key_path, "Vega verifier key", MAX_KEY_BYTES)
    if proving_key.path == verifier_key.path or proving_key.sha256 == verifier_key.sha256:
        raise Refusal("Vega proving and verifier artifacts must have distinct identities")
    staging = Path(tempfile.mkdtemp(prefix=".vega-figure9-", dir=output_root))
    try:
        os.chmod(staging, 0o700)
        packaged_validator = _copy_file(
            validator, staging / NATIVE_VALIDATOR_FILE, 0o500, "native validator"
        )
        packaged_proving_key = _copy_file(
            proving_key, staging / PROVING_KEY_FILE, 0o400, "proving key"
        )
        packaged_verifier_key = _copy_file(
            verifier_key, staging / VERIFIER_KEY_FILE, 0o400, "verifier key"
        )
        # Never execute or decode through the caller-owned paths after their
        # identities have been checked.  Copying first makes the exact bytes
        # authenticated above the only bytes admitted to native qualification.
        first_output = Path(tempfile.mkdtemp(prefix=".native-evidence-first-", dir=staging)).resolve(
            strict=True
        )
        os.chmod(first_output, 0o700)
        report, report_bytes, first_evidence = _run_native_validator(
            packaged_validator,
            packaged_proving_key,
            packaged_verifier_key,
            first_output,
        )
        packaged_evidence = {
            file_name: _copy_file(
                first_evidence[file_name],
                staging / file_name,
                0o400,
                f"Vega evidence {file_name}",
            )
            for _, file_name, _ in EVIDENCE_FILES
        }
        packaged_report = _write_bytes(staging / NATIVE_REPORT_FILE, report_bytes, 0o400)
        second_output = Path(
            tempfile.mkdtemp(prefix=".native-evidence-second-", dir=staging)
        ).resolve(strict=True)
        os.chmod(second_output, 0o700)
        replay, replay_bytes, replay_evidence = _run_native_validator(
            packaged_validator,
            packaged_proving_key,
            packaged_verifier_key,
            second_output,
        )
        if replay != report or replay_bytes != report_bytes:
            raise Refusal("native validation changed after content-addressed staging")
        for _, file_name, _ in EVIDENCE_FILES:
            if not _files_equal(first_evidence[file_name], replay_evidence[file_name]):
                raise Refusal(f"native Vega evidence {file_name} changed across exact replay")
            if not _files_equal(packaged_evidence[file_name], replay_evidence[file_name]):
                raise Refusal(f"packaged Vega evidence {file_name} differs from exact replay")
        shutil.rmtree(first_output)
        shutil.rmtree(second_output)
        manifest = _candidate_manifest(
            report,
            packaged_validator,
            packaged_proving_key,
            packaged_verifier_key,
            packaged_report,
            packaged_evidence,
        )
        manifest_bytes = _canonical_json(manifest)
        package_sha256 = hashlib.sha256(manifest_bytes).hexdigest()
        _write_bytes(staging / MANIFEST_FILE, manifest_bytes, 0o400)
        final_path = output_root / package_sha256
        os.chmod(staging, 0o500)
        if final_path.exists():
            os.chmod(staging, 0o700)
            shutil.rmtree(staging)
            staging = final_path
        else:
            try:
                os.rename(staging, final_path)
            except OSError:
                if not final_path.exists():
                    raise
                os.chmod(staging, 0o700)
                shutil.rmtree(staging)
                staging = final_path
        verify_package(final_path, package_sha256)
        return final_path, package_sha256
    except BaseException:
        if staging.exists() and staging.name.startswith(".vega-figure9-"):
            try:
                os.chmod(staging, 0o700)
                shutil.rmtree(staging)
            except OSError:
                pass
        raise


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    create = commands.add_parser("package", help="create one candidate-only package")
    create.add_argument("--native-validator", type=Path, required=True)
    create.add_argument("--expected-native-validator-sha256", required=True)
    create.add_argument("--proving-key", type=Path, required=True)
    create.add_argument("--verifier-key", type=Path, required=True)
    create.add_argument("--output-root", type=Path, required=True)
    verify = commands.add_parser("verify-package", help="replay one pinned package")
    verify.add_argument("--package", type=Path, required=True)
    verify.add_argument("--expected-package-sha256", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "package":
            path, digest = package(
                args.native_validator,
                args.expected_native_validator_sha256,
                args.proving_key,
                args.verifier_key,
                args.output_root,
            )
            print(_canonical_json({"package": str(path), "package_sha256": digest}).decode(), end="")
        else:
            verify_package(args.package, args.expected_package_sha256)
            print(
                _canonical_json(
                    {"package": str(args.package), "package_sha256": args.expected_package_sha256}
                ).decode(),
                end="",
            )
    except Refusal as error:
        print(f"Vega Figure 9 artifact packaging refused: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
