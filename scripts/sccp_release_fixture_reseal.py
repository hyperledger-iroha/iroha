#!/usr/bin/env python3
"""Prepare and finalize the pinned SCCP test-fixture reseal.

This tool never accepts signing keys and contains no signing implementation.
``prepare`` builds the exact canonical evidence payload that two external
release-role signers must sign. ``finalize`` accepts only those public detached
signatures, validates the complete candidate with both Python and the pinned
Rust validator, and publishes a fully staged fixture generation with an atomic
directory exchange.
"""

from __future__ import annotations

import argparse
import base64
import copy
import ctypes
import errno
import fcntl
import hashlib
import os
import re
import shutil
import stat
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterable, Iterator, Mapping, Sequence

import sccp_release_common as common


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_ROOT = ROOT / "fixtures" / "sccp" / "release_evidence_v1"
RUST_VALIDATOR_SOURCE = (
    ROOT / "crates" / "iroha_sccp" / "src" / "bin" / "sccp_release_evidence.rs"
)

POLICY_NAME = "test-trust-policy.json"
EVIDENCE_NAME = "evidence.json"
SESSION_SCHEMA = "sccp-release-test-fixture-reseal-session-v1"
FIXTURE_RELATIVE_ROOT = "fixtures/sccp/release_evidence_v1"
FIXTURE_POLICY_ID = "sccp-v1-fixture-policy-20260711"
FIXTURE_RELEASE_ID = "sccp-v1-typed-fixture-20260711"
ROLE_SPECS = (
    ("release-engineering", "fixture-release-engineering"),
    ("release-security", "fixture-release-security"),
)
SESSION_FILES = {
    "manifest": "reseal-manifest.json",
    "policy": "candidate-policy.json",
    "evidence": "unsigned-evidence.json",
    "payload": "evidence-signing-payload.bin",
}
_RUST_FORBIDDEN_RE = re.compile(
    r"const FORBIDDEN_FIXTURE_PUBLIC_KEYS: \[&str; (?P<count>[0-9]+)\] = "
    r"\[(?P<body>.*?)\n\];",
    re.DOTALL,
)
_HEX_KEY_RE = re.compile(r'"([0-9a-f]{64})"')


def _fail(message: str) -> None:
    raise common.SccpReleaseError(message)


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _policy_path() -> Path:
    return FIXTURE_ROOT / POLICY_NAME


def _evidence_path() -> Path:
    return FIXTURE_ROOT / EVIDENCE_NAME


def _fsync_directory(path: Path) -> None:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_DIRECTORY", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError:
        _fail("reseal directory could not be opened durably")
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _require_direct_directory(path: Path, *, label: str) -> os.stat_result:
    return common._require_direct_directory(path, label=label)


def _directory_identity(path: Path, *, label: str) -> tuple[int, int]:
    metadata = _require_direct_directory(path, label=label)
    return metadata.st_dev, metadata.st_ino


def _require_directory_identity(
    path: Path, expected: tuple[int, int], *, label: str
) -> None:
    if _directory_identity(path, label=label) != expected:
        _fail(f"{label} was substituted during fixture publication")


def _open_direct_directory_fd(
    path: Path,
    *,
    label: str,
    expected_identity: tuple[int, int] | None = None,
) -> int:
    before = _require_direct_directory(path, label=label)
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags)
    except OSError:
        _fail(f"{label} could not be opened safely")
    try:
        opened = os.fstat(descriptor)
    except OSError:
        os.close(descriptor)
        _fail(f"{label} changed while opening")
    identity = opened.st_dev, opened.st_ino
    if (
        not stat.S_ISDIR(opened.st_mode)
        or identity != (before.st_dev, before.st_ino)
        or (expected_identity is not None and identity != expected_identity)
    ):
        os.close(descriptor)
        _fail(f"{label} changed while opening")
    return descriptor


def _open_child_directory_fd(
    parent_fd: int,
    name: str,
    *,
    label: str,
    expected_identity: tuple[int, int] | None = None,
) -> int:
    if not name or "/" in name or name in (".", ".."):
        _fail(f"{label} has an unsafe directory name")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(name, flags, dir_fd=parent_fd)
    except OSError:
        _fail(f"{label} could not be opened safely")
    try:
        opened = os.fstat(descriptor)
    except OSError:
        os.close(descriptor)
        _fail(f"{label} changed while opening")
    identity = opened.st_dev, opened.st_ino
    if not stat.S_ISDIR(opened.st_mode) or (
        expected_identity is not None and identity != expected_identity
    ):
        os.close(descriptor)
        _fail(f"{label} changed while opening")
    return descriptor


@contextmanager
def _fixture_publication_lock() -> Iterator[None]:
    """Serialize cooperating finalizers on the pinned fixture parent inode."""

    parent = FIXTURE_ROOT.parent
    before = _require_direct_directory(parent, label="pinned SCCP fixture parent")
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(parent, flags)
    except OSError:
        _fail("pinned SCCP fixture parent could not be opened for publication locking")
    locked = False
    try:
        opened = os.fstat(descriptor)
        if (
            not stat.S_ISDIR(opened.st_mode)
            or (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino)
        ):
            _fail("pinned SCCP fixture parent changed while acquiring its lock")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            locked = True
        except BlockingIOError:
            _fail("another SCCP fixture finalization is already in progress")
        except OSError:
            _fail("pinned SCCP fixture publication lock could not be acquired")
        yield
    finally:
        if locked:
            try:
                fcntl.flock(descriptor, fcntl.LOCK_UN)
            except OSError:
                pass
        os.close(descriptor)


def _absolute_output_path(path: Path, *, label: str) -> Path:
    if not path.is_absolute():
        _fail(f"{label} must be an absolute path")
    absolute = Path(os.path.abspath(path))
    current = Path(absolute.anchor)
    for component in absolute.parts[1:-1]:
        current /= component
        try:
            metadata = current.lstat()
        except OSError:
            _fail(f"{label} parent path is not accessible")
        if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
            _fail(f"{label} parent path must contain only direct directories")
    _require_direct_directory(absolute.parent, label=f"{label} parent")
    try:
        absolute.relative_to(FIXTURE_ROOT)
    except ValueError:
        pass
    else:
        _fail(f"{label} must stay outside the pinned fixture root")
    return absolute


def _write_exclusive_at(
    directory_fd: int,
    name: str,
    data: bytes,
    *,
    mode: int = 0o600,
) -> None:
    if not name or "/" in name or name in (".", ".."):
        _fail("reseal output has an unsafe file name")
    flags = (
        os.O_WRONLY
        | os.O_CREAT
        | os.O_EXCL
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(name, flags, mode, dir_fd=directory_fd)
    except OSError:
        _fail(f"reseal output {name} already exists or is unsafe")
    try:
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                _fail(f"reseal output {name} did not make write progress")
            view = view[written:]
        if hasattr(os, "fchmod"):
            os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or metadata.st_nlink != 1
            or metadata.st_size != len(data)
        ):
            _fail(f"reseal output {name} changed while writing")
    finally:
        os.close(descriptor)


def _read_json(path: Path, *, label: str, maximum: int) -> tuple[dict[str, Any], bytes]:
    data = common.read_direct_file(path, label=label, maximum=maximum)
    value = common.parse_json_bytes(data, label=label, maximum=maximum)
    common.require_canonical_json_file(data, value, label=label)
    if type(value) is not dict:
        _fail(f"{label} must be a JSON object")
    return value, data


def _rust_forbidden_keys() -> frozenset[str]:
    source = common.read_direct_file(
        RUST_VALIDATOR_SOURCE,
        label="Rust SCCP release validator source",
        maximum=2 * 1024 * 1024,
    )
    try:
        text = source.decode("utf-8", "strict")
    except UnicodeDecodeError:
        _fail("Rust SCCP release validator source is not UTF-8")
    match = _RUST_FORBIDDEN_RE.search(text)
    if match is None:
        _fail("Rust SCCP fixture-key registry is missing or malformed")
    keys = _HEX_KEY_RE.findall(match.group("body"))
    if int(match.group("count")) != len(keys) or len(keys) != len(set(keys)):
        _fail("Rust SCCP fixture-key registry count is not exact")
    return frozenset(keys)


def _require_forbidden_key_registration(keys: Iterable[str]) -> None:
    requested = frozenset(keys)
    rust_keys = _rust_forbidden_keys()
    if rust_keys != common.FORBIDDEN_FIXTURE_PUBLIC_KEYS:
        _fail("Python and Rust SCCP fixture-key registries do not match exactly")
    if not requested <= rust_keys:
        _fail("every reseal role key must be registered as fixture-only in Python and Rust")


def _fixture_inventory(evidence: Mapping[str, Any]) -> tuple[str, ...]:
    artifact_paths = tuple(entry["path"] for entry in evidence["artifacts"])
    expected = tuple(sorted((POLICY_NAME, EVIDENCE_NAME, *artifact_paths)))
    actual = common.enumerate_direct_files(FIXTURE_ROOT)
    if actual != expected:
        _fail("pinned SCCP fixture tree contains an unexpected or missing file")
    return expected


def _candidate_policy(
    policy: Mapping[str, Any], public_keys: Sequence[str]
) -> tuple[dict[str, Any], bytes]:
    if len(public_keys) != len(ROLE_SPECS):
        _fail("the reseal requires exactly two public role keys")
    candidate = copy.deepcopy(policy)
    for index, ((role, signer_id), public_key) in enumerate(zip(ROLE_SPECS, public_keys)):
        entry = candidate["roles"][index]
        if entry["role"] != role or entry["signer_id"] != signer_id:
            _fail("pinned SCCP fixture role identities have drifted")
        entry["public_key_hex"] = public_key
    data = common.canonical_json_file_bytes(candidate)
    validated, _ = common.validate_trust_policy_bytes(data, allow_test_policy=True)
    if validated["policy_id"] != FIXTURE_POLICY_ID:
        _fail("pinned SCCP fixture policy id has drifted")
    return validated, data


def _candidate_unsigned_evidence(
    evidence: Mapping[str, Any],
    policy: Mapping[str, Any],
    validator_identity: Mapping[str, Any],
) -> tuple[dict[str, Any], bytes, bytes]:
    if evidence.get("release_id") != FIXTURE_RELEASE_ID:
        _fail("pinned SCCP fixture release id has drifted")
    if evidence.get("trust_policy_id") != FIXTURE_POLICY_ID:
        _fail("pinned SCCP fixture evidence policy id has drifted")
    candidate = {
        key: copy.deepcopy(value)
        for key, value in evidence.items()
        if key != "provenance"
    }
    candidate["trust_policy_sha256_hex"] = _sha256(
        common.canonical_json_file_bytes(policy)
    )
    candidate["validator"] = copy.deepcopy(validator_identity)
    validated = common.validate_test_fixture_evidence_signing_candidate(
        candidate, policy
    )
    common.verify_evidence_artifacts(validated, FIXTURE_ROOT)
    data = common.canonical_json_file_bytes(validated)
    payload = common.evidence_signing_payload(validated)
    return validated, data, payload


def _manifest(
    *,
    base_policy: bytes,
    base_evidence: bytes,
    policy: Mapping[str, Any],
    policy_bytes: bytes,
    evidence: Mapping[str, Any],
    evidence_bytes: bytes,
    payload: bytes,
    validator_hash: str,
) -> dict[str, Any]:
    return {
        "schema": SESSION_SCHEMA,
        "fixture_root": FIXTURE_RELATIVE_ROOT,
        "base_policy_sha256_hex": _sha256(base_policy),
        "base_evidence_sha256_hex": _sha256(base_evidence),
        "candidate_policy_sha256_hex": _sha256(policy_bytes),
        "unsigned_evidence_sha256_hex": _sha256(evidence_bytes),
        "signing_payload_sha256_hex": _sha256(payload),
        "artifact_inventory_sha256_hex": _sha256(
            common.canonical_json_bytes(evidence["artifacts"])
        ),
        "validator_executable_sha256_hex": validator_hash,
        "roles": copy.deepcopy(policy["roles"]),
    }


def prepare(
    *,
    validator_path: Path,
    session_dir: Path,
    public_keys: Sequence[str],
) -> dict[str, Any]:
    """Create one immutable public signing session without overwriting files."""

    session_dir = _absolute_output_path(session_dir, label="reseal session directory")
    _require_direct_directory(FIXTURE_ROOT, label="pinned SCCP fixture root")
    policy, base_policy = common.load_trust_policy(
        _policy_path(), allow_test_policy=True
    )
    evidence, base_evidence = _read_json(
        _evidence_path(), label="existing SCCP fixture evidence", maximum=common.MAX_EVIDENCE_BYTES
    )
    _require_forbidden_key_registration(public_keys)
    candidate_policy, policy_bytes = _candidate_policy(policy, public_keys)
    identity, validator_hash = common.derive_validator_identity(validator_path)
    unsigned, evidence_bytes, payload = _candidate_unsigned_evidence(
        evidence, candidate_policy, identity
    )
    _fixture_inventory(unsigned)
    manifest = _manifest(
        base_policy=base_policy,
        base_evidence=base_evidence,
        policy=candidate_policy,
        policy_bytes=policy_bytes,
        evidence=unsigned,
        evidence_bytes=evidence_bytes,
        payload=payload,
        validator_hash=validator_hash,
    )

    parent_fd = _open_direct_directory_fd(
        session_dir.parent, label="reseal session directory parent"
    )
    try:
        try:
            os.stat(session_dir.name, dir_fd=parent_fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        except OSError:
            _fail("reseal session directory could not be inspected safely")
        else:
            _fail("reseal session directory must not already exist")
        try:
            os.mkdir(session_dir.name, 0o700, dir_fd=parent_fd)
        except OSError:
            _fail("reseal session directory could not be created exclusively")
        created = os.stat(session_dir.name, dir_fd=parent_fd, follow_symlinks=False)
        created_identity = created.st_dev, created.st_ino
        session_fd = _open_child_directory_fd(
            parent_fd,
            session_dir.name,
            label="reseal session directory",
            expected_identity=created_identity,
        )
        try:
            _write_exclusive_at(
                session_fd, SESSION_FILES["policy"], policy_bytes
            )
            _write_exclusive_at(
                session_fd, SESSION_FILES["evidence"], evidence_bytes
            )
            _write_exclusive_at(session_fd, SESSION_FILES["payload"], payload)
            _write_exclusive_at(
                session_fd,
                SESSION_FILES["manifest"],
                common.canonical_json_file_bytes(manifest),
            )
            observed = os.stat(
                session_dir.name, dir_fd=parent_fd, follow_symlinks=False
            )
            if (
                not stat.S_ISDIR(observed.st_mode)
                or (observed.st_dev, observed.st_ino) != created_identity
            ):
                _fail("reseal session directory was substituted while writing")
            os.fsync(session_fd)
            os.fsync(parent_fd)
        finally:
            os.close(session_fd)
    finally:
        # Never recursively remove the pathname on failure: another process
        # could have exchanged it after the exclusive mkdir. Exact inventory
        # checks make a partial session unusable and safe for inspection.
        os.close(parent_fd)
    return {
        "schema": SESSION_SCHEMA,
        "session_dir": str(session_dir),
        "signing_payload": str(session_dir / SESSION_FILES["payload"]),
        "signing_payload_sha256_hex": manifest["signing_payload_sha256_hex"],
        "roles": manifest["roles"],
    }


def _load_session(session_dir: Path) -> tuple[dict[str, Any], bytes, bytes, bytes]:
    session_dir = _absolute_output_path(session_dir, label="reseal session directory")
    _require_direct_directory(session_dir, label="reseal session directory")
    actual = common.enumerate_direct_files(session_dir)
    expected = tuple(sorted(SESSION_FILES.values()))
    if actual != expected:
        _fail("reseal session contains an unexpected or missing file")
    manifest, _ = _read_json(
        session_dir / SESSION_FILES["manifest"],
        label="reseal session manifest",
        maximum=64 * 1024,
    )
    manifest = common._require_object(
        manifest,
        label="reseal session manifest",
        keys=(
            "schema",
            "fixture_root",
            "base_policy_sha256_hex",
            "base_evidence_sha256_hex",
            "candidate_policy_sha256_hex",
            "unsigned_evidence_sha256_hex",
            "signing_payload_sha256_hex",
            "artifact_inventory_sha256_hex",
            "validator_executable_sha256_hex",
            "roles",
        ),
    )
    if manifest["schema"] != SESSION_SCHEMA or manifest["fixture_root"] != FIXTURE_RELATIVE_ROOT:
        _fail("reseal session is not pinned to the SCCP fixture")
    for field in (
        "base_policy_sha256_hex",
        "base_evidence_sha256_hex",
        "candidate_policy_sha256_hex",
        "unsigned_evidence_sha256_hex",
        "signing_payload_sha256_hex",
        "artifact_inventory_sha256_hex",
        "validator_executable_sha256_hex",
    ):
        common._require_hex(manifest[field], label=f"reseal manifest {field}", byte_length=32)
    policy_bytes = common.read_direct_file(
        session_dir / SESSION_FILES["policy"],
        label="reseal candidate policy",
        maximum=common.MAX_TRUST_POLICY_BYTES,
    )
    evidence_bytes = common.read_direct_file(
        session_dir / SESSION_FILES["evidence"],
        label="reseal unsigned evidence",
        maximum=common.MAX_EVIDENCE_BYTES,
    )
    payload = common.read_direct_file(
        session_dir / SESSION_FILES["payload"],
        label="reseal signing payload",
        maximum=common.MAX_EVIDENCE_BYTES,
    )
    if (
        _sha256(policy_bytes) != manifest["candidate_policy_sha256_hex"]
        or _sha256(evidence_bytes) != manifest["unsigned_evidence_sha256_hex"]
        or _sha256(payload) != manifest["signing_payload_sha256_hex"]
    ):
        _fail("reseal session files do not match their manifest")
    return manifest, policy_bytes, evidence_bytes, payload


def _validate_session_candidate(
    manifest: Mapping[str, Any],
    policy_bytes: bytes,
    evidence_bytes: bytes,
    payload: bytes,
    validator_path: Path,
) -> tuple[dict[str, Any], dict[str, Any]]:
    policy, _ = common.validate_trust_policy_bytes(
        policy_bytes, allow_test_policy=True
    )
    if policy["policy_id"] != FIXTURE_POLICY_ID or policy["roles"] != manifest["roles"]:
        _fail("reseal candidate policy does not match its pinned session")
    keys = [entry["public_key_hex"] for entry in policy["roles"]]
    _require_forbidden_key_registration(keys)
    value = common.parse_json_bytes(
        evidence_bytes,
        label="reseal unsigned evidence",
        maximum=common.MAX_EVIDENCE_BYTES,
    )
    common.require_canonical_json_file(
        evidence_bytes, value, label="reseal unsigned evidence"
    )
    evidence = common.validate_test_fixture_evidence_signing_candidate(value, policy)
    if evidence["release_id"] != FIXTURE_RELEASE_ID:
        _fail("reseal unsigned evidence release id has drifted")
    if _sha256(common.canonical_json_bytes(evidence["artifacts"])) != manifest[
        "artifact_inventory_sha256_hex"
    ]:
        _fail("reseal unsigned evidence artifact inventory has drifted")
    expected_payload = common.evidence_signing_payload(evidence)
    if payload != expected_payload:
        _fail("reseal signing payload is not the canonical evidence payload")
    identity, validator_hash = common.derive_validator_identity(validator_path)
    if (
        validator_hash != manifest["validator_executable_sha256_hex"]
        or evidence["validator"] != identity
    ):
        _fail("Rust validator identity changed after reseal preparation")
    return policy, evidence


def _provenance(
    policy: Mapping[str, Any], signatures: Sequence[str]
) -> list[dict[str, Any]]:
    if len(signatures) != len(ROLE_SPECS):
        _fail("the reseal requires exactly two detached signatures")
    output: list[dict[str, Any]] = []
    for index, ((role, signer_id), signature_text) in enumerate(zip(ROLE_SPECS, signatures)):
        signature = common._canonical_base64(
            signature_text,
            label=f"{role} detached signature",
            decoded_length=64,
        )
        trusted = policy["roles"][index]
        if trusted["role"] != role or trusted["signer_id"] != signer_id:
            _fail("reseal policy roles are not exact")
        output.append(
            {
                "role": role,
                "signer_id": signer_id,
                "algorithm": "ed25519",
                "public_key_hex": trusted["public_key_hex"],
                "signature_b64": base64.b64encode(signature).decode("ascii"),
            }
        )
    return output


def _copy_fixture_tree(
    target: Path,
    target_fd: int,
    expected_files: Sequence[str],
    policy_bytes: bytes,
    evidence_bytes: bytes,
) -> None:
    evidence_value = common.parse_json_bytes(
        evidence_bytes,
        label="staged evidence",
        maximum=common.MAX_EVIDENCE_BYTES,
    )
    artifact_kinds = {
        entry["path"]: entry["kind"] for entry in evidence_value["artifacts"]
    }
    source_root = _require_direct_directory(
        FIXTURE_ROOT, label="pinned SCCP fixture root"
    )
    target_identity = os.fstat(target_fd)
    if not stat.S_ISDIR(target_identity.st_mode):
        _fail("staged SCCP fixture descriptor is not a directory")
    os.fchmod(target_fd, stat.S_IMODE(source_root.st_mode))
    for relative in expected_files:
        parts = common._safe_relative_parts(relative, label="fixture copy path")
        source = FIXTURE_ROOT.joinpath(*parts)
        source_metadata = source.lstat()
        if (
            not stat.S_ISREG(source_metadata.st_mode)
            or stat.S_ISLNK(source_metadata.st_mode)
            or source_metadata.st_nlink != 1
        ):
            _fail(f"fixture copy {relative} is not a direct single-link file")
        current_fd = os.dup(target_fd)
        try:
            source_parent = FIXTURE_ROOT
            for component in parts[:-1]:
                source_parent /= component
                source_directory = _require_direct_directory(
                    source_parent, label=f"fixture copy {relative} source parent"
                )
                try:
                    os.mkdir(component, 0o700, dir_fd=current_fd)
                    os.fsync(current_fd)
                except FileExistsError:
                    pass
                except OSError:
                    _fail(f"fixture copy {relative} parent could not be created")
                child_fd = _open_child_directory_fd(
                    current_fd,
                    component,
                    label=f"fixture copy {relative} destination parent",
                )
                try:
                    os.fchmod(child_fd, stat.S_IMODE(source_directory.st_mode))
                    os.fsync(child_fd)
                except Exception:
                    os.close(child_fd)
                    raise
                os.close(current_fd)
                current_fd = child_fd
            if relative == POLICY_NAME:
                data = policy_bytes
            elif relative == EVIDENCE_NAME:
                data = evidence_bytes
            else:
                data = common.read_relative_file(
                    FIXTURE_ROOT,
                    relative,
                    label=f"fixture copy {relative}",
                    maximum=common.artifact_limit(artifact_kinds[relative]),
                )
            _write_exclusive_at(
                current_fd,
                parts[-1],
                data,
                mode=stat.S_IMODE(source_metadata.st_mode),
            )
            os.fsync(current_fd)
        finally:
            os.close(current_fd)
    os.fsync(target_fd)
    opened = os.fstat(target_fd)
    if (opened.st_dev, opened.st_ino) != (
        target_identity.st_dev,
        target_identity.st_ino,
    ):
        _fail("staged SCCP fixture descriptor changed while copying")
    _require_directory_identity(
        target,
        (target_identity.st_dev, target_identity.st_ino),
        label="staged SCCP fixture root",
    )


def _validate_staged_with_rust(stage: Path, validator_path: Path) -> None:
    policy_path = stage / POLICY_NAME
    evidence_path = stage / EVIDENCE_NAME
    policy, policy_bytes = common.load_trust_policy(
        policy_path, allow_test_policy=True
    )
    evidence, evidence_bytes = common.load_evidence_file(evidence_path, policy)
    common.verify_evidence_artifacts(evidence, stage)
    common.verify_rust_release_signatures(
        trust_policy_path=policy_path,
        trust_policy=policy,
        trust_policy_bytes=policy_bytes,
        evidence_path=evidence_path,
        evidence=evidence,
        evidence_bytes=evidence_bytes,
        validator_path=validator_path,
        environment="test-fixture",
    )
    common.verify_rust_lane_evidence(
        evidence,
        stage,
        validator_path,
        policy,
        trust_policy_path=policy_path,
        evidence_path=evidence_path,
        environment="test-fixture",
    )


def _atomic_exchange_directories(
    left: Path,
    right: Path,
    *,
    expected_left_identity: tuple[int, int],
    expected_right_identity: tuple[int, int],
) -> None:
    left_meta = _require_direct_directory(left, label="live SCCP fixture root")
    right_meta = _require_direct_directory(right, label="staged SCCP fixture root")
    if (left_meta.st_dev, left_meta.st_ino) != expected_left_identity:
        _fail("live SCCP fixture root was substituted before atomic exchange")
    if (right_meta.st_dev, right_meta.st_ino) != expected_right_identity:
        _fail("staged SCCP fixture root was substituted before atomic exchange")
    if left.parent != right.parent or left_meta.st_dev != right_meta.st_dev:
        _fail("live and staged SCCP fixture roots must share one direct filesystem parent")
    libc = ctypes.CDLL(None, use_errno=True)
    left_bytes = os.fsencode(left)
    right_bytes = os.fsencode(right)
    at_fdcwd = -2
    if sys.platform == "darwin" and hasattr(libc, "renameatx_np"):
        rename = libc.renameatx_np
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(at_fdcwd, left_bytes, at_fdcwd, right_bytes, 0x00000002)
    elif sys.platform.startswith("linux") and hasattr(libc, "renameat2"):
        rename = libc.renameat2
        rename.argtypes = [
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_int,
            ctypes.c_char_p,
            ctypes.c_uint,
        ]
        rename.restype = ctypes.c_int
        result = rename(at_fdcwd, left_bytes, at_fdcwd, right_bytes, 0x00000002)
    else:
        _fail("this platform has no supported atomic directory-exchange primitive")
    if result != 0:
        error = ctypes.get_errno()
        _fail(f"atomic SCCP fixture publication failed: {os.strerror(error or errno.EIO)}")


def _post_publish_validate(
    policy_bytes: bytes, evidence_bytes: bytes, expected_files: Sequence[str]
) -> None:
    policy, observed_policy = common.load_trust_policy(
        _policy_path(), allow_test_policy=True
    )
    evidence, observed_evidence = common.load_evidence_file(_evidence_path(), policy)
    if observed_policy != policy_bytes or observed_evidence != evidence_bytes:
        _fail("published SCCP fixture bytes differ from the validated generation")
    if common.enumerate_direct_files(FIXTURE_ROOT) != tuple(expected_files):
        _fail("published SCCP fixture inventory differs from the validated generation")
    common.verify_evidence_artifacts(evidence, FIXTURE_ROOT)


def _require_generation_continuity(
    root: Path,
    *,
    expected_files: Sequence[str],
    policy_bytes: bytes,
    evidence_bytes: bytes,
    artifact_evidence: Mapping[str, Any],
    label: str,
) -> None:
    if common.enumerate_direct_files(root) != tuple(expected_files):
        _fail(f"{label} inventory changed during finalization")
    if common.read_direct_file(
        root / POLICY_NAME,
        label=f"{label} policy",
        maximum=common.MAX_TRUST_POLICY_BYTES,
    ) != policy_bytes or common.read_direct_file(
        root / EVIDENCE_NAME,
        label=f"{label} evidence",
        maximum=common.MAX_EVIDENCE_BYTES,
    ) != evidence_bytes:
        _fail(f"{label} policy or evidence changed during finalization")
    common.verify_evidence_artifacts(artifact_evidence, root)


def _remove_old_generation(
    path: Path,
    *,
    expected_identity: tuple[int, int],
    expected_files: Sequence[str],
    policy_bytes: bytes,
    evidence_bytes: bytes,
    artifact_evidence: Mapping[str, Any],
) -> bool:
    try:
        _require_directory_identity(
            path, expected_identity, label="fixture generation selected for cleanup"
        )
        _require_generation_continuity(
            path,
            expected_files=expected_files,
            policy_bytes=policy_bytes,
            evidence_bytes=evidence_bytes,
            artifact_evidence=artifact_evidence,
            label="displaced SCCP fixture generation",
        )
        _require_directory_identity(
            path, expected_identity, label="fixture generation selected for cleanup"
        )
        shutil.rmtree(path)
        _fsync_directory(path.parent)
        return True
    except (OSError, common.SccpReleaseError):
        return False


def _finalize_locked(
    *,
    validator_path: Path,
    session_dir: Path,
    signatures: Sequence[str],
) -> dict[str, Any]:
    """Finalize while holding the fixture-parent publication lock."""

    session_dir = _absolute_output_path(session_dir, label="reseal session directory")
    manifest, policy_bytes, unsigned_bytes, payload = _load_session(session_dir)
    live_identity = _directory_identity(
        FIXTURE_ROOT, label="pinned SCCP fixture root"
    )
    current_policy = common.read_direct_file(
        _policy_path(), label="current SCCP fixture policy", maximum=common.MAX_TRUST_POLICY_BYTES
    )
    current_evidence = common.read_direct_file(
        _evidence_path(), label="current SCCP fixture evidence", maximum=common.MAX_EVIDENCE_BYTES
    )
    if (
        _sha256(current_policy) != manifest["base_policy_sha256_hex"]
        or _sha256(current_evidence) != manifest["base_evidence_sha256_hex"]
    ):
        _fail("pinned SCCP fixture changed after reseal preparation")
    policy, unsigned = _validate_session_candidate(
        manifest, policy_bytes, unsigned_bytes, payload, validator_path
    )
    expected_files = _fixture_inventory(unsigned)
    complete = copy.deepcopy(unsigned)
    complete["provenance"] = _provenance(policy, signatures)
    evidence = common.validate_evidence(complete, policy)
    common.verify_evidence_artifacts(evidence, FIXTURE_ROOT)
    evidence_bytes = common.canonical_json_file_bytes(evidence)

    parent = FIXTURE_ROOT.parent
    _require_direct_directory(parent, label="pinned SCCP fixture parent")
    stage = Path(tempfile.mkdtemp(prefix=".release_evidence_v1.reseal-", dir=parent))
    stage_identity = _directory_identity(stage, label="staged SCCP fixture root")
    stage_fd = _open_direct_directory_fd(
        stage,
        label="staged SCCP fixture root",
        expected_identity=stage_identity,
    )
    exchanged = False
    published = False
    cleanup_ok = True
    try:
        _copy_fixture_tree(
            stage, stage_fd, expected_files, policy_bytes, evidence_bytes
        )
        _validate_staged_with_rust(stage, validator_path)
        _require_generation_continuity(
            FIXTURE_ROOT,
            expected_files=expected_files,
            policy_bytes=current_policy,
            evidence_bytes=current_evidence,
            artifact_evidence=unsigned,
            label="live SCCP fixture generation",
        )
        _require_directory_identity(
            FIXTURE_ROOT, live_identity, label="live SCCP fixture root"
        )
        _require_directory_identity(stage, stage_identity, label="staged SCCP fixture root")
        # Mark the path state ambiguous before the syscall so an asynchronous
        # interruption can never make cleanup treat a completed exchange as a
        # pre-publication failure.
        exchanged = True
        _atomic_exchange_directories(
            FIXTURE_ROOT,
            stage,
            expected_left_identity=live_identity,
            expected_right_identity=stage_identity,
        )
        try:
            _require_directory_identity(
                FIXTURE_ROOT,
                stage_identity,
                label="published SCCP fixture generation",
            )
            _require_directory_identity(
                stage,
                live_identity,
                label="displaced SCCP fixture generation",
            )
            _fsync_directory(parent)
            _require_generation_continuity(
                stage,
                expected_files=expected_files,
                policy_bytes=current_policy,
                evidence_bytes=current_evidence,
                artifact_evidence=unsigned,
                label="displaced SCCP fixture generation",
            )
            _post_publish_validate(policy_bytes, evidence_bytes, expected_files)
        except Exception:
            rollback_exchanged = False
            try:
                _require_directory_identity(
                    FIXTURE_ROOT,
                    stage_identity,
                    label="published SCCP fixture generation",
                )
                _require_directory_identity(
                    stage,
                    live_identity,
                    label="displaced SCCP fixture generation",
                )
                _atomic_exchange_directories(
                    FIXTURE_ROOT,
                    stage,
                    expected_left_identity=stage_identity,
                    expected_right_identity=live_identity,
                )
                rollback_exchanged = True
                _require_directory_identity(
                    FIXTURE_ROOT,
                    live_identity,
                    label="restored SCCP fixture generation",
                )
                _require_directory_identity(
                    stage,
                    stage_identity,
                    label="rolled-back SCCP fixture candidate",
                )
                _fsync_directory(parent)
            except Exception as rollback_error:
                if rollback_exchanged:
                    # The second exchange happened but its resulting path
                    # identities are ambiguous, so neither path is deleted.
                    exchanged = True
                raise common.SccpReleaseError(
                    "published SCCP fixture failed validation and atomic rollback failed"
                ) from rollback_error
            exchanged = False
            raise
        published = True
        cleanup_ok = _remove_old_generation(
            stage,
            expected_identity=live_identity,
            expected_files=expected_files,
            policy_bytes=current_policy,
            evidence_bytes=current_evidence,
            artifact_evidence=unsigned,
        )
    finally:
        os.close(stage_fd)
        if not published and not exchanged:
            _remove_old_generation(
                stage,
                expected_identity=stage_identity,
                expected_files=expected_files,
                policy_bytes=policy_bytes,
                evidence_bytes=evidence_bytes,
                artifact_evidence=evidence,
            )
    result: dict[str, Any] = {
        "schema": SESSION_SCHEMA,
        "published": True,
        "fixture_root": str(FIXTURE_ROOT),
        "policy_sha256_hex": _sha256(policy_bytes),
        "evidence_sha256_hex": _sha256(evidence_bytes),
        "validator_executable_sha256_hex": manifest[
            "validator_executable_sha256_hex"
        ],
    }
    if not cleanup_ok:
        result["old_generation_cleanup_required"] = str(stage)
    return result


def finalize(
    *,
    validator_path: Path,
    session_dir: Path,
    signatures: Sequence[str],
) -> dict[str, Any]:
    """Verify external signatures and atomically publish the staged generation."""

    with _fixture_publication_lock():
        return _finalize_locked(
            validator_path=validator_path,
            session_dir=session_dir,
            signatures=signatures,
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Prepare or finalize the pinned SCCP test-fixture reseal."
    )
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser(
        "prepare", help="Emit the canonical public payload for two external signers."
    )
    prepare_parser.add_argument("--rust-validator", required=True, type=Path)
    prepare_parser.add_argument("--session-dir", required=True, type=Path)
    prepare_parser.add_argument(
        "--release-engineering-public-key-hex", required=True
    )
    prepare_parser.add_argument(
        "--release-security-public-key-hex", required=True
    )
    finalize_parser = commands.add_parser(
        "finalize", help="Verify detached signatures and atomically publish the fixture."
    )
    finalize_parser.add_argument("--rust-validator", required=True, type=Path)
    finalize_parser.add_argument("--session-dir", required=True, type=Path)
    finalize_parser.add_argument(
        "--release-engineering-signature-b64", required=True
    )
    finalize_parser.add_argument(
        "--release-security-signature-b64", required=True
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "prepare":
            result = prepare(
                validator_path=args.rust_validator,
                session_dir=args.session_dir,
                public_keys=(
                    args.release_engineering_public_key_hex,
                    args.release_security_public_key_hex,
                ),
            )
        else:
            result = finalize(
                validator_path=args.rust_validator,
                session_dir=args.session_dir,
                signatures=(
                    args.release_engineering_signature_b64,
                    args.release_security_signature_b64,
                ),
            )
        sys.stdout.buffer.write(common.canonical_json_file_bytes(result))
        return 0
    except (OSError, ValueError, common.SccpReleaseError) as error:
        print(f"SCCP fixture reseal failed: {common.public_error(error)}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
