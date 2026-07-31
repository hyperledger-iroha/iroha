#!/usr/bin/env python3
"""Deploy one authenticated four-validator Taira v21 fresh-reset cohort.

Without ``--apply`` this command is strictly read-only: it verifies one signed
archive-only rollout admission, binds that archive's receipt to the binary,
supervisor, reset manifest, and exact four configs, then authenticates the
current launchd cohort, disk headroom, and a read-only directory fsync barrier.
An explicitly authorized reset of an already-degraded testnet may use
``--allow-absent-old-child`` only when an exact loaded old supervisor has
neither a PID file nor any child process.  ``--apply`` additionally requires
root, re-verifies admission under the deployment lock, atomically consumes its
receipt in the canonical protected replay ledger, and installs
content-addressed root-owned code.  That exact binary fully qualifies the
catalog once and publishes its immutable root-owned seal, then validates all
four configs through the sealed fast path before mutating the old cohort.  The
receipt consumption is restored if deployment never reaches that first cohort
mutation.  The rollout replaces all four LaunchDaemons as one cohort, proves
mandatory offline readiness and advancing consensus, and proves one supervised
child can restart without replacing its supervisor.  Any failed rollout
removes only a seal whose returned identity it owns and restores the old
release/cohort.  If the writer publishes a seal but fails before returning its
identity, the controller preserves that unattributed seal and its installed
release for explicit operator recovery.

The controller never prints config contents, process command lines, HTTP
bodies, or other runtime signing material.
"""

from __future__ import annotations

import argparse
import contextlib
import dataclasses
import fcntl
import grp
import hashlib
import json
import os
import plistlib
import pwd
import re
import shlex
import shutil
import signal
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Callable, NoReturn, Optional, Sequence

try:
    from scripts import taira_rollout_admission as rollout_admission
except ModuleNotFoundError as error:
    if error.name != "scripts":
        raise
    import taira_rollout_admission as rollout_admission

PEER_COUNT = 4
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
CHAIN_DISCRIMINANT = 369
OFFLINE_ASSET_ID = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
OFFLINE_ASSET_SCALE = 2
OFFLINE_CAPABILITY = "cash_handoff_v1"
OFFLINE_BRIDGE_ABI = 21
NODE_STORAGE_BUDGET_BYTES = 64 * 1024 * 1024 * 1024
NODE_STORAGE_BUDGET_POLICY = "bounded-64-gib-per-validator"
NODE_STORAGE_WEIGHTS = {
    "kura_blocks_bps": 7_500,
    "wsv_snapshots_bps": 2_000,
    "sorafs_bps": 0,
    "soranet_spool_bps": 250,
    "soravpn_spool_bps": 250,
}
DEFAULT_MINIMUM_FREE_BYTES = 16 * 1024 * 1024 * 1024
DEFAULT_MAXIMUM_FSYNC_LATENCY_MS = 250
DEFAULT_HEALTH_TIMEOUT_SECONDS = 240
RESTART_PROOF_TIMEOUT_SECONDS = 45
CONFIG_CHECK_TIMEOUT_SECONDS = 180
# The one-time gate streams the full catalog twice (roughly 18.4 GB today);
# keep its APFS bound separate from the strict sealed fast-path checks.
CATALOG_QUALIFICATION_TIMEOUT_SECONDS = 3_600
MAX_BINARY_BYTES = 2 * 1024 * 1024 * 1024
MAX_CONFIG_BYTES = 2 * 1024 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_HTTP_BYTES = 4 * 1024 * 1024
MAX_QUALIFICATION_SEAL_BYTES = 8 * 1024 * 1024
MAX_TERMINAL_UNHEALTHY_BYTES = 1024
MAX_RELEASE_FILE_BYTES = 5 * 1024 * 1024 * 1024
MAX_BUNDLE_BYTES = 64 * 1024 * 1024 * 1024
EXPECTED_RELEASE_FILE_COUNT = 16
RELEASE_ATTESTATION_FILE_NAME = "release-attestation-v4.norito"
RELEASE_POLICY_FILE_NAME = "release-policy-v1.norito"
RELEASE_CATALOG_DIRECTORY_NAME = "catalog"
EXPECTED_RELEASE_FILE_NAMES = {
    "cryptographic-review.evidence",
    "manifest.json",
    "manifest.norito",
    "manifest.norito.sha256",
    "physical-device-benchmark.evidence",
    "promotion-record-v4.norito",
    RELEASE_ATTESTATION_FILE_NAME,
    "step-ep.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "topup-finality-roster-v4.norito",
}
MINIMUM_RELEASE_WITHDRAWAL_HEIGHT = 1_000_000
EMPTY_TREE_SHA256 = hashlib.sha256().hexdigest()
LABELS = tuple(
    f"io.soramitsu.taira.validator-{index}" for index in range(1, PEER_COUNT + 1)
)
SLUGS = tuple(f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1))
TORII_PORTS = tuple(29_080 + index for index in range(PEER_COUNT))
P2P_PORTS = tuple(33_337 + index for index in range(PEER_COUNT))
TOP_LEVEL_NAMES = {
    "base-config.toml",
    "genesis.json",
    "genesis.signed.nrt",
    "kagemusha",
    "operator-identity.json",
    "rendered",
    "reset-manifest.json",
    "validator-roster.toml",
    "validator-secrets.toml",
}
VALIDATOR_NAMES = {"codec", "config.toml", "configs", "manifests", "runtime", "storage"}
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
BLOCK_HASH_RE = re.compile(r"(?:hash:)?([0-9A-Fa-f]{64})")
TERMINAL_UNHEALTHY_SCHEMA = "taira-terminal-unhealthy-v1"
INSTALL_ROOT = Path("/Library/SORA/Taira")
LAUNCH_DAEMONS = Path("/Library/LaunchDaemons")
DEFAULT_SUPERVISOR_PYTHON = Path("/usr/bin/python3")
DEPLOYMENT_LOCK = INSTALL_ROOT / "deploy-v21.lock"
ADMISSION_REPLAY_LEDGER = INSTALL_ROOT / "rollout-admission-replay-v1.json"
ADMISSION_REPLAY_LEDGER_MODE = 0o644
MACOS_ACL_INSPECTOR = Path("/bin/ls")
MACOS_ACL_CLEARER = Path("/bin/chmod")
MACOS_ACL_COMMAND_TIMEOUT_SECONDS = 5
MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES = 64 * 1024


class DeploymentError(RuntimeError):
    """Raised when an identity, safety, rollout, or rollback gate fails."""


def fail(message: str) -> NoReturn:
    """Raise one redaction-safe deployment refusal."""

    raise DeploymentError(message)


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Decode JSON while rejecting ambiguous duplicate object members."""

    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member {key!r}")
        result[key] = value
    return result


def require_sha256(value: object, label: str) -> str:
    """Require a lowercase SHA-256 literal."""

    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        fail(f"{label} must be one lowercase SHA-256 digest")
    return value


def require_commit(value: object, label: str = "expected source commit") -> str:
    """Require one full nonzero lowercase Git object id."""

    if (
        not isinstance(value, str)
        or COMMIT_RE.fullmatch(value) is None
        or value == "0" * 40
    ):
        fail(f"{label} must be one full nonzero lowercase Git object id")
    return value


def qualification_seal_path(release_tree_sha256: str) -> Path:
    """Return the exact root-trusted seal path for one release-tree identity."""

    release_tree_sha256 = require_sha256(
        release_tree_sha256, "Kagemusha release tree SHA-256"
    )
    return INSTALL_ROOT / "seals" / f"kagemusha-v4-{release_tree_sha256}.norito"


def _run_bounded_macos_acl_command(
    program: Path, option: str, path: Path, label: str
) -> subprocess.CompletedProcess[bytes]:
    """Run one absolute macOS ACL tool with bounded time and retained output."""

    if not program.is_absolute() or program not in {
        MACOS_ACL_INSPECTOR,
        MACOS_ACL_CLEARER,
    }:
        fail(f"macOS ACL command is not one pinned absolute tool: {program}")
    try:
        result = subprocess.run(
            [str(program), option, str(path)],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            timeout=MACOS_ACL_COMMAND_TIMEOUT_SECONDS,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise DeploymentError(
            f"bounded macOS ACL command failed for {label}: {path}"
        ) from error
    if (
        len(result.stdout) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
        or len(result.stderr) > MACOS_ACL_COMMAND_MAX_OUTPUT_BYTES
    ):
        fail(f"macOS ACL command output exceeded its bound for {label}: {path}")
    return result


def require_acl_free_path(
    path: Path,
    label: str,
    *,
    descriptor: Optional[int] = None,
) -> os.stat_result:
    """Require a stable path, and on macOS prove it has no extended ACL."""

    before = path.lstat()
    if descriptor is not None and metadata_identity(
        os.fstat(descriptor)
    ) != metadata_identity(before):
        fail(f"{label} name does not identify its opened inode: {path}")
    if sys.platform == "darwin":
        result = _run_bounded_macos_acl_command(
            MACOS_ACL_INSPECTOR, "-ldeq", path, label
        )
        if (
            result.returncode != 0
            or result.stderr
            or not result.stdout.endswith(b"\n")
            or result.stdout.count(b"\n") != 1
        ):
            fail(f"{label} must not have an extended ACL: {path}")
    after = path.lstat()
    if metadata_identity(after) != metadata_identity(before):
        fail(f"{label} changed during ACL validation: {path}")
    if descriptor is not None and metadata_identity(
        os.fstat(descriptor)
    ) != metadata_identity(after):
        fail(f"{label} name changed from its opened inode: {path}")
    return after


def clear_owned_temporary_acl(path: Path, descriptor: int, label: str) -> None:
    """Clear inherited macOS ACLs only on one unpublished, owned temp inode."""

    before_path = path.lstat()
    before_opened = os.fstat(descriptor)
    if (
        metadata_identity(before_path) != metadata_identity(before_opened)
        or not stat.S_ISREG(before_opened.st_mode)
        or before_opened.st_nlink != 1
        or before_opened.st_uid != os.geteuid()
    ):
        fail(f"{label} temporary name does not identify its owned opened inode")
    if sys.platform == "darwin":
        result = _run_bounded_macos_acl_command(MACOS_ACL_CLEARER, "-N", path, label)
        if result.returncode != 0 or result.stdout or result.stderr:
            fail(f"failed to clear inherited ACL from {label}: {path}")
    after_path = path.lstat()
    after_opened = os.fstat(descriptor)
    stable_before = (
        before_opened.st_dev,
        before_opened.st_ino,
        before_opened.st_mode,
        before_opened.st_uid,
        before_opened.st_gid,
        before_opened.st_nlink,
        before_opened.st_size,
        before_opened.st_mtime_ns,
    )
    stable_after = (
        after_opened.st_dev,
        after_opened.st_ino,
        after_opened.st_mode,
        after_opened.st_uid,
        after_opened.st_gid,
        after_opened.st_nlink,
        after_opened.st_size,
        after_opened.st_mtime_ns,
    )
    if stable_after != stable_before or metadata_identity(
        after_path
    ) != metadata_identity(after_opened):
        fail(f"{label} changed while its inherited ACL was cleared")
    require_acl_free_path(path, label, descriptor=descriptor)


def canonical_path(path: Path, label: str) -> Path:
    """Require an existing absolute canonical path without aliases."""

    if not path.is_absolute():
        fail(f"{label} must be absolute")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise DeploymentError(f"{label} is unavailable: {path}") from error
    if resolved != path:
        fail(f"{label} must be canonical and symlink-free: {path}")
    return path


def metadata_identity(info: os.stat_result) -> tuple[int, ...]:
    """Return stable regular-file identity fields used around reads."""

    return (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def open_regular(path: Path, maximum_bytes: int) -> tuple[int, os.stat_result]:
    """Open one bounded single-link regular file without following links."""

    before = path.lstat()
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        fail(f"unsafe or oversized regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    after = os.fstat(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        os.close(descriptor)
        fail(f"file changed while it was opened: {path}")
    return descriptor, after


def read_regular(path: Path, maximum_bytes: int) -> tuple[bytes, os.stat_result]:
    """Read a bounded regular file and recheck its complete identity."""

    descriptor, before = open_regular(path, maximum_bytes)
    try:
        chunks: list[bytes] = []
        while chunk := os.read(descriptor, 1024 * 1024):
            chunks.append(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        fail(f"file changed while it was read: {path}")
    return b"".join(chunks), after


def sha256_regular(path: Path, maximum_bytes: int) -> tuple[str, os.stat_result]:
    """Hash a stable bounded regular file through a no-follow descriptor."""

    descriptor, before = open_regular(path, maximum_bytes)
    try:
        digest = hashlib.sha256()
        while chunk := os.read(descriptor, 1024 * 1024):
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        fail(f"file changed while it was hashed: {path}")
    return digest.hexdigest(), after


def parse_json_bytes(raw: bytes, label: str) -> dict[str, Any]:
    """Decode one canonical JSON object without duplicate members."""

    try:
        payload = json.loads(raw, object_pairs_hook=reject_duplicate_keys)
    except (UnicodeDecodeError, ValueError, json.JSONDecodeError) as error:
        raise DeploymentError(f"{label} is invalid JSON") from error
    if not isinstance(payload, dict):
        fail(f"{label} must be a JSON object")
    return payload


def require_private_entry(
    path: Path, owner_uid: int, owner_gid: int, *, directory: bool
) -> os.stat_result:
    """Require one owner-private bundle entry with a stable type and owner."""

    info = path.lstat()
    expected = stat.S_ISDIR(info.st_mode) if directory else stat.S_ISREG(info.st_mode)
    if (
        stat.S_ISLNK(info.st_mode)
        or not expected
        or info.st_uid != owner_uid
        or info.st_gid != owner_gid
        or stat.S_IMODE(info.st_mode) & 0o077
        or (not directory and info.st_nlink != 1)
    ):
        fail(f"unsafe owner-private bundle entry: {path}")
    return info


def require_exact_names(path: Path, expected: set[str], label: str) -> None:
    """Require a directory to contain an exact, alias-free name set."""

    actual = {entry.name for entry in path.iterdir()}
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        fail(f"{label} inventory is not exact (missing={missing}, extra={extra})")


def inspect_private_bundle_tree(bundle: Path, owner_uid: int, owner_gid: int) -> int:
    """Reject aliases, special files, loose permissions, or an oversized bundle."""

    total = 0
    for current, directory_names, file_names in os.walk(bundle, followlinks=False):
        directory_names.sort()
        file_names.sort()
        current_path = Path(current)
        require_private_entry(current_path, owner_uid, owner_gid, directory=True)
        for name in directory_names:
            require_private_entry(
                current_path / name, owner_uid, owner_gid, directory=True
            )
        for name in file_names:
            info = require_private_entry(
                current_path / name, owner_uid, owner_gid, directory=False
            )
            total += info.st_size
            if total > MAX_BUNDLE_BYTES:
                fail("reset bundle exceeds the bounded 64 GiB deployment corridor")
    return total


CONFIG_PROJECTION_FIELDS: dict[tuple[str, ...], dict[str, str]] = {
    (): {
        "chain": "string",
        "chain_discriminant": "integer",
    },
    ("network",): {"address": "string"},
    ("torii",): {"address": "string"},
    ("torii", "kagemusha_commands"): {"enabled": "boolean"},
    ("nexus", "storage"): {"local_budget_bytes": "integer"},
    ("nexus", "storage", "disk_budget_weights"): {
        "kura_blocks_bps": "integer",
        "wsv_snapshots_bps": "integer",
        "sorafs_bps": "integer",
        "soranet_spool_bps": "integer",
        "soravpn_spool_bps": "integer",
    },
    ("settlement", "offline"): {
        "enabled": "boolean",
        "escrow_required": "boolean",
        "kagemusha_release_policy_path": "string",
        "kagemusha_artifact_dir": "string",
        "kagemusha_catalog_qualification_seal_path": "string",
    },
    ("genesis",): {"file": "string"},
}
TOML_TABLE_RE = re.compile(r"^\[([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)\]$")
TOML_ARRAY_TABLE_RE = re.compile(r"^\[\[([A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*)\]\]$")
TOML_ASSIGNMENT_RE = re.compile(r"^([A-Za-z0-9_-]+)\s*=\s*(.*)$")
TOML_UNSIGNED_INTEGER_RE = re.compile(r"^(?:0|[1-9][0-9]*)$")
TOML_HEX_RE = re.compile(r"^[0-9A-Fa-f]+$")


def _strip_toml_comment(line: str, label: str, line_number: int) -> str:
    """Strip a TOML comment without mistaking ``#`` inside a string."""

    quote: Optional[str] = None
    escaped = False
    index = 0
    while index < len(line):
        character = line[index]
        if quote is None:
            if character == "#":
                return line[:index]
            if character in ('"', "'"):
                if line[index : index + 3] == character * 3:
                    fail(
                        f"{label} uses an unsupported multiline string "
                        f"at line {line_number}"
                    )
                quote = character
        elif quote == '"':
            if escaped:
                escaped = False
            elif character == "\\":
                escaped = True
            elif character == '"':
                quote = None
        elif character == "'":
            quote = None
        index += 1
    if quote is not None or escaped:
        fail(f"{label} has an unterminated string at line {line_number}")
    return line


def _decode_toml_string(value: str, label: str, line_number: int) -> str:
    """Decode one single-line TOML basic or literal string."""

    if len(value) < 2 or value[0] not in ('"', "'"):
        fail(f"{label} has a malformed string at line {line_number}")
    quote = value[0]
    output: list[str] = []
    index = 1
    while index < len(value):
        character = value[index]
        if character == quote:
            if index != len(value) - 1:
                fail(f"{label} has trailing data at line {line_number}")
            return "".join(output)
        if quote == "'" or character != "\\":
            if ord(character) < 0x20 or ord(character) == 0x7F:
                fail(f"{label} contains a control character at line {line_number}")
            output.append(character)
            index += 1
            continue
        index += 1
        if index >= len(value):
            fail(f"{label} has an incomplete escape at line {line_number}")
        escape = value[index]
        simple_escapes = {
            '"': '"',
            "\\": "\\",
            "b": "\b",
            "t": "\t",
            "n": "\n",
            "f": "\f",
            "r": "\r",
        }
        if escape in simple_escapes:
            output.append(simple_escapes[escape])
            index += 1
            continue
        if escape not in ("u", "U"):
            fail(f"{label} has an invalid escape at line {line_number}")
        width = 4 if escape == "u" else 8
        digits = value[index + 1 : index + 1 + width]
        if len(digits) != width or TOML_HEX_RE.fullmatch(digits) is None:
            fail(f"{label} has an invalid Unicode escape at line {line_number}")
        codepoint = int(digits, 16)
        if codepoint > 0x10FFFF or 0xD800 <= codepoint <= 0xDFFF:
            fail(f"{label} has an invalid Unicode scalar at line {line_number}")
        output.append(chr(codepoint))
        index += width + 1
    fail(f"{label} has an unterminated string at line {line_number}")


def _decode_projection_value(
    raw: str, value_type: str, label: str, line_number: int
) -> object:
    """Decode one required projected TOML scalar with no coercion."""

    value = raw.strip()
    if value_type == "string":
        return _decode_toml_string(value, label, line_number)
    if value_type == "boolean":
        if value == "true":
            return True
        if value == "false":
            return False
        fail(f"{label} has a malformed boolean at line {line_number}")
    if TOML_UNSIGNED_INTEGER_RE.fullmatch(value) is None:
        fail(f"{label} has a malformed integer at line {line_number}")
    return int(value)


def parse_config_projection_text(text: str, label: str) -> dict[str, Any]:
    """Extract exactly the fail-closed validator fields consumed by preflight."""

    projected: dict[str, Any] = {}
    current_table: tuple[str, ...] = ()
    seen_tables: set[tuple[str, ...]] = {()}
    seen_fields: set[tuple[tuple[str, ...], str]] = set()
    for line_number, raw_line in enumerate(text.splitlines(), start=1):
        line = _strip_toml_comment(raw_line, label, line_number).strip()
        if not line:
            continue
        if line.startswith("["):
            array_match = TOML_ARRAY_TABLE_RE.fullmatch(line)
            table_match = TOML_TABLE_RE.fullmatch(line)
            if array_match is None and table_match is None:
                fail(f"{label} has a malformed table header at line {line_number}")
            table_text = (
                array_match.group(1)
                if array_match is not None
                else table_match.group(1)
            )
            current_table = tuple(table_text.split("."))
            if current_table in CONFIG_PROJECTION_FIELDS:
                if array_match is not None:
                    fail(
                        f"{label} declares required table {table_text} as an array "
                        f"at line {line_number}"
                    )
                if current_table in seen_tables:
                    fail(
                        f"{label} duplicates required table {table_text} "
                        f"at line {line_number}"
                    )
                seen_tables.add(current_table)
            continue
        assignment = TOML_ASSIGNMENT_RE.fullmatch(line)
        required = CONFIG_PROJECTION_FIELDS.get(current_table)
        if assignment is None:
            if required is not None and any(
                re.match(rf"^{re.escape(key)}(?:\s|=|$)", line) for key in required
            ):
                fail(
                    f"{label} has a malformed required assignment "
                    f"at line {line_number}"
                )
            continue
        key, raw_value = assignment.groups()
        if required is None or key not in required:
            continue
        identity = (current_table, key)
        if identity in seen_fields:
            dotted = ".".join((*current_table, key))
            fail(f"{label} duplicates required field {dotted} at line {line_number}")
        seen_fields.add(identity)
        value = _decode_projection_value(
            raw_value,
            required[key],
            label,
            line_number,
        )
        destination = projected
        for component in current_table:
            child = destination.setdefault(component, {})
            if not isinstance(child, dict):
                fail(f"{label} has an ambiguous required table projection")
            destination = child
        destination[key] = value

    missing = [
        ".".join((*table, key))
        for table, fields in CONFIG_PROJECTION_FIELDS.items()
        for key in fields
        if (table, key) not in seen_fields
    ]
    if missing:
        fail(f"{label} lacks required fields: {', '.join(sorted(missing))}")
    return projected


def parse_toml(path: Path, owner_uid: int, owner_gid: int) -> dict[str, Any]:
    """Extract one bounded private validator config projection."""

    require_private_entry(path, owner_uid, owner_gid, directory=False)
    raw, _ = read_regular(path, MAX_CONFIG_BYTES)
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise DeploymentError(f"validator config is not UTF-8: {path}") from error
    return parse_config_projection_text(text, f"validator config {path}")


def address_port(value: object, label: str) -> int:
    """Extract the TCP port from one canonical ``addr:HOST:PORT#CRC`` literal."""

    if not isinstance(value, str) or not value.startswith("addr:") or "#" not in value:
        fail(f"{label} is not a canonical address literal")
    address = value[5:].split("#", 1)[0]
    _, separator, port_text = address.rpartition(":")
    if not separator or not port_text.isascii() or not port_text.isdecimal():
        fail(f"{label} has no canonical TCP port")
    port = int(port_text)
    if not 1 <= port <= 65_535:
        fail(f"{label} TCP port is out of range")
    return port


@dataclasses.dataclass(frozen=True)
class ReleaseTreeSeal:
    """One post-hash release-tree entry identity used for O(1) cutover checks."""

    relative_path: str
    identity: tuple[int, ...]


def release_tree_snapshot(
    root: Path, owner_uid: int, owner_gid: int
) -> tuple[str, tuple[ReleaseTreeSeal, ...]]:
    """Hash one exact release tree and bind every entry's final stat identity."""

    digest = hashlib.sha256()
    paths = sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix())
    seals: list[ReleaseTreeSeal] = [
        ReleaseTreeSeal(".", metadata_identity(root.lstat()))
    ]
    for path in paths:
        relative_text = path.relative_to(root).as_posix()
        relative = relative_text.encode()
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode):
            fail(f"release tree contains a symlink: {path}")
        if stat.S_ISDIR(info.st_mode):
            info = require_private_entry(path, owner_uid, owner_gid, directory=True)
            digest.update(b"d\0" + relative + b"\0")
        else:
            require_private_entry(path, owner_uid, owner_gid, directory=False)
            file_sha256, info = sha256_regular(path, MAX_RELEASE_FILE_BYTES)
            digest.update(b"f\0" + relative + b"\0")
            digest.update(info.st_size.to_bytes(8, "big"))
            digest.update(bytes.fromhex(file_sha256))
        seals.append(ReleaseTreeSeal(relative_text, metadata_identity(info)))

    final_paths = sorted(
        root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()
    )
    if [path.relative_to(root).as_posix() for path in final_paths] != [
        seal.relative_path for seal in seals[1:]
    ]:
        fail("release tree inventory changed while it was hashed")
    for seal in seals:
        path = root if seal.relative_path == "." else root / seal.relative_path
        if metadata_identity(path.lstat()) != seal.identity:
            fail(f"release tree entry changed while it was hashed: {path}")
    return digest.hexdigest(), tuple(seals)


def release_tree_sha256(root: Path, owner_uid: int, owner_gid: int) -> str:
    """Hash the release tree using the packager's canonical name/size projection."""

    digest, _ = release_tree_snapshot(root, owner_uid, owner_gid)
    return digest


def require_manifest_hash(
    manifest: dict[str, Any], field: str, path: Path, maximum: int
) -> tuple[str, os.stat_result]:
    """Require a file digest to equal its reset-manifest binding."""

    expected = manifest.get(field)
    if not isinstance(expected, str):
        fail(f"reset manifest omitted {field}")
    require_sha256(expected, f"reset manifest {field}")
    actual, info = sha256_regular(path, maximum)
    if actual != expected:
        fail(f"reset manifest {field} does not match {path.name}")
    return actual, info


@dataclasses.dataclass(frozen=True)
class PeerPlan:
    """Authenticated per-validator runtime paths and identities."""

    number: int
    label: str
    slug: str
    torii_port: int
    p2p_port: int
    workdir: Path
    storage: Path
    config: Path
    config_sha256: str
    config_identity: tuple[int, ...]
    workdir_identity: tuple[int, ...]
    storage_identity: tuple[int, ...]
    workdir_device: int
    workdir_inode: int
    storage_device: int
    storage_inode: int


@dataclasses.dataclass(frozen=True)
class BundlePlan:
    """Complete authenticated dry-run result used by apply."""

    root: Path
    owner_uid: int
    owner_gid: int
    runtime_user: str
    runtime_group: str
    manifest: dict[str, Any]
    manifest_sha256: str
    manifest_identity: tuple[int, ...]
    signed_genesis_identity: tuple[int, ...]
    release: ReleasePlan
    peers: tuple[PeerPlan, ...]
    bundle_bytes: int
    free_bytes: int
    free_bytes_by_device: tuple[tuple[int, int], ...]
    fsync_latency_ms: float


@dataclasses.dataclass(frozen=True)
class ReleasePlan:
    """Receipt-bound release identity and its single-copy cutover seal."""

    source_root: Path
    installed_root: Path
    tree_sha256: str
    tree_seals: tuple[ReleaseTreeSeal, ...]
    manifest_sha256: str
    release_policy_sha256: str
    release_attestation_sha256: str
    generation: str
    activation_height: int
    withdrawal_height: int
    max_proof_bytes: int
    asset_scale: int


def validate_release(
    bundle: Path,
    manifest: dict[str, Any],
    owner_uid: int,
    owner_gid: int,
) -> ReleasePlan:
    """Authenticate the exact reset-manifest-bound Kagemusha catalog."""

    manifest_sha256 = require_sha256(
        manifest.get("kagemusha_manifest_sha256"),
        "reset manifest Kagemusha manifest SHA-256",
    )
    policy_sha256 = require_sha256(
        manifest.get("kagemusha_release_policy_sha256"),
        "reset manifest Kagemusha release policy SHA-256",
    )
    attestation_sha256 = require_sha256(
        manifest.get("kagemusha_release_attestation_sha256"),
        "reset manifest Kagemusha release attestation SHA-256",
    )

    root = bundle / "kagemusha"
    require_exact_names(
        root,
        {RELEASE_CATALOG_DIRECTORY_NAME, RELEASE_POLICY_FILE_NAME},
        "Kagemusha root",
    )
    policy_sha, _ = sha256_regular(root / RELEASE_POLICY_FILE_NAME, 64 * 1024)
    if policy_sha != policy_sha256:
        fail("Kagemusha release policy does not match the reset manifest")
    catalog = root / RELEASE_CATALOG_DIRECTORY_NAME
    releases = list(catalog.iterdir())
    if len(releases) != 1 or SHA256_RE.fullmatch(releases[0].name) is None:
        fail("Kagemusha catalog must contain exactly one manifest-addressed release")
    release = releases[0]
    require_private_entry(release, owner_uid, owner_gid, directory=True)
    entries = list(release.iterdir())
    if (
        len(entries) != EXPECTED_RELEASE_FILE_COUNT
        or {path.name for path in entries} != EXPECTED_RELEASE_FILE_NAMES
        or any(path.is_dir() for path in entries)
    ):
        fail(
            "Kagemusha release does not contain the exact authenticated "
            f"{EXPECTED_RELEASE_FILE_COUNT}-file inventory"
        )
    manifest_norito_sha, _ = sha256_regular(
        release / "manifest.norito", MAX_MANIFEST_BYTES
    )
    if (
        manifest_norito_sha != release.name
        or manifest_norito_sha != manifest_sha256
    ):
        fail("Kagemusha manifest identity is not content-addressed")
    digest_body, _ = read_regular(release / "manifest.norito.sha256", 65)
    if digest_body != f"{manifest_norito_sha}\n".encode("ascii"):
        fail("Kagemusha manifest digest sidecar is invalid")
    release_json_raw, _ = read_regular(release / "manifest.json", MAX_MANIFEST_BYTES)
    release_json = parse_json_bytes(release_json_raw, "Kagemusha release manifest")
    attestation_sha, _ = sha256_regular(
        release / RELEASE_ATTESTATION_FILE_NAME, 2 * 1024 * 1024
    )
    withdrawal_height = release_json.get("withdrawal_height")
    max_proof_bytes = release_json.get("max_proof_bytes")
    if (
        release_json.get("chain_id") != CHAIN_ID
        or release_json.get("asset") != OFFLINE_ASSET_ID
        or release_json.get("asset_scale") != OFFLINE_ASSET_SCALE
        or release_json.get("activation_height") != 2
        or release_json.get("bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or release_json.get("generation") != "production-gate-real-artifacts-v4"
        or not isinstance(withdrawal_height, int)
        or isinstance(withdrawal_height, bool)
        or withdrawal_height < MINIMUM_RELEASE_WITHDRAWAL_HEIGHT
        or not isinstance(max_proof_bytes, int)
        or isinstance(max_proof_bytes, bool)
        or max_proof_bytes <= 0
        or release_json.get("release_attestation_sha256") != attestation_sha
        or attestation_sha != attestation_sha256
    ):
        fail("Kagemusha release manifest is not the exact Taira ABI-21/V4 release")
    actual_tree, tree_seals = release_tree_snapshot(root, owner_uid, owner_gid)
    if actual_tree != manifest.get("kagemusha_release_tree_sha256"):
        fail("Kagemusha release tree does not match the reset manifest")
    return ReleasePlan(
        source_root=root,
        installed_root=INSTALL_ROOT / "releases" / actual_tree,
        tree_sha256=actual_tree,
        tree_seals=tree_seals,
        manifest_sha256=manifest_norito_sha,
        release_policy_sha256=policy_sha,
        release_attestation_sha256=attestation_sha,
        generation="production-gate-real-artifacts-v4",
        activation_height=2,
        withdrawal_height=withdrawal_height,
        max_proof_bytes=max_proof_bytes,
        asset_scale=OFFLINE_ASSET_SCALE,
    )


def validate_config_projection(
    config: dict[str, Any],
    bundle: Path,
    release_root: Path,
    *,
    torii_port: int,
    p2p_port: int,
) -> None:
    """Require exact public-Taira, storage, port, and mandatory-offline config."""

    if (
        config.get("chain") != CHAIN_ID
        or config.get("chain_discriminant") != CHAIN_DISCRIMINANT
    ):
        fail("validator config does not target canonical public Taira")
    network = config.get("network")
    torii = config.get("torii")
    nexus = config.get("nexus")
    settlement = config.get("settlement")
    genesis = config.get("genesis")
    if not all(
        isinstance(value, dict)
        for value in (network, torii, nexus, settlement, genesis)
    ):
        fail(
            "validator config lacks required network/Torii/Nexus/offline/genesis tables"
        )
    assert isinstance(network, dict) and isinstance(torii, dict)
    assert (
        isinstance(nexus, dict)
        and isinstance(settlement, dict)
        and isinstance(genesis, dict)
    )
    if address_port(network.get("address"), "network.address") != p2p_port:
        fail(f"validator config P2P port is not exact {p2p_port}")
    if address_port(torii.get("address"), "torii.address") != torii_port:
        fail(f"validator config Torii port is not exact {torii_port}")
    storage = nexus.get("storage")
    if (
        not isinstance(storage, dict)
        or storage.get("local_budget_bytes") != NODE_STORAGE_BUDGET_BYTES
        or storage.get("disk_budget_weights") != NODE_STORAGE_WEIGHTS
    ):
        fail("validator config lacks the exact bounded 64 GiB storage policy")
    offline = settlement.get("offline")
    commands = torii.get("kagemusha_commands")
    if (
        not isinstance(offline, dict)
        or offline.get("enabled") is not True
        or offline.get("escrow_required") is not True
        or not isinstance(commands, dict)
        or commands.get("enabled") is not True
    ):
        fail("validator config does not make offline cash mandatory")
    if (
        offline.get("kagemusha_release_policy_path")
        != str(release_root / RELEASE_POLICY_FILE_NAME)
        or offline.get("kagemusha_artifact_dir")
        != str(release_root / RELEASE_CATALOG_DIRECTORY_NAME)
        or offline.get("kagemusha_catalog_qualification_seal_path")
        != str(qualification_seal_path(release_root.name))
    ):
        fail("validator config does not bind the authenticated Kagemusha catalog")
    if genesis.get("file") != str(bundle / "genesis.signed.nrt"):
        fail("validator config does not bind the reset bundle signed genesis")


def measure_read_only_fsync(path: Path, maximum_ms: int) -> float:
    """Measure a read-only directory fsync barrier without creating any file."""

    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0),
    )
    try:
        started = time.monotonic_ns()
        os.fsync(descriptor)
        elapsed = (time.monotonic_ns() - started) / 1_000_000
    except OSError as error:
        raise DeploymentError(f"read-only fsync preflight failed for {path}") from error
    finally:
        os.close(descriptor)
    if elapsed > maximum_ms:
        fail(
            f"fsync latency {elapsed:.3f} ms exceeds the {maximum_ms} ms deployment bound"
        )
    return elapsed


def existing_ancestor(path: Path) -> Path:
    """Return the nearest existing ancestor of one absolute deployment path."""

    current = path
    while not current.exists():
        if current == current.parent:
            fail(f"deployment path has no existing ancestor: {path}")
        current = current.parent
    return current


def require_filesystem_headroom(
    paths: Sequence[Path], minimum_free_bytes: int
) -> tuple[tuple[int, int], ...]:
    """Require minimum free space on every distinct filesystem in ``paths``."""

    free_by_device: dict[int, int] = {}
    for path in paths:
        existing = existing_ancestor(path)
        info = existing.stat()
        free = shutil.disk_usage(existing).free
        prior = free_by_device.get(info.st_dev)
        free_by_device[info.st_dev] = free if prior is None else min(prior, free)
    for device, free in sorted(free_by_device.items()):
        if free < minimum_free_bytes:
            fail(
                f"filesystem device {device} has only {free} free bytes; "
                f"{minimum_free_bytes} are required"
            )
    return tuple(sorted(free_by_device.items()))


def validate_operator_release_identity(raw: bytes, release: ReleasePlan) -> None:
    """Bind the operator-facing artifact identity to the pinned release."""

    identity = parse_json_bytes(raw, "operator release identity")
    artifact = identity.get("artifact_set")
    if (
        identity.get("cash_handoff_capability") != OFFLINE_CAPABILITY
        or identity.get("required_bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or identity.get("asset_definition_id") != OFFLINE_ASSET_ID
        or identity.get("asset_scale") != OFFLINE_ASSET_SCALE
        or not isinstance(artifact, dict)
        or artifact.get("generation") != release.generation
        or artifact.get("manifest_sha256") != release.manifest_sha256
        or artifact.get("release_policy_sha256") != release.release_policy_sha256
        or artifact.get("release_attestation_sha256")
        != release.release_attestation_sha256
        or artifact.get("activation_height") != release.activation_height
        or artifact.get("withdrawal_height") != release.withdrawal_height
        or artifact.get("max_proof_bytes") != release.max_proof_bytes
        or artifact.get("asset_scale") != release.asset_scale
    ):
        fail("operator release identity does not bind the exact pinned release")


def validate_bundle(
    bundle: Path,
    *,
    expected_reset_manifest_sha256: str,
    expected_binary_sha256: str,
    expected_source_commit: str,
    minimum_free_bytes: int,
    maximum_fsync_latency_ms: int,
) -> BundlePlan:
    """Authenticate a fresh v21 bundle without changing it or running binaries."""

    bundle = canonical_path(bundle, "reset bundle")
    root_info = bundle.lstat()
    if stat.S_ISLNK(root_info.st_mode) or not stat.S_ISDIR(root_info.st_mode):
        fail("reset bundle is not a real directory")
    if root_info.st_uid == 0 or stat.S_IMODE(root_info.st_mode) & 0o077:
        fail("reset bundle must be owned by a non-root runtime user and mode 0700")
    owner_uid, owner_gid = root_info.st_uid, root_info.st_gid
    try:
        runtime_user = pwd.getpwuid(owner_uid).pw_name
        runtime_group = grp.getgrgid(owner_gid).gr_name
    except KeyError as error:
        raise DeploymentError(
            "reset bundle owner has no local user/group identity"
        ) from error
    require_exact_names(bundle, TOP_LEVEL_NAMES, "reset bundle")
    bundle_bytes = inspect_private_bundle_tree(bundle, owner_uid, owner_gid)

    manifest_path = bundle / "reset-manifest.json"
    manifest_raw, manifest_info = read_regular(manifest_path, MAX_MANIFEST_BYTES)
    manifest = parse_json_bytes(manifest_raw, "reset manifest")
    manifest_sha256 = hashlib.sha256(manifest_raw).hexdigest()
    if manifest_sha256 != expected_reset_manifest_sha256:
        fail("reset manifest does not match the verified admission receipt")
    if (
        manifest.get("schema") != "taira-exact2f-reset-bundle"
        or manifest.get("peer_count") != PEER_COUNT
        or manifest.get("chain_id") != CHAIN_ID
        or manifest.get("chain_discriminant") != CHAIN_DISCRIMINANT
        or manifest.get("node_storage_budget_bytes") != NODE_STORAGE_BUDGET_BYTES
        or manifest.get("node_storage_budget_weights") != NODE_STORAGE_WEIGHTS
        or manifest.get("nexus_storage_budget_policy") != NODE_STORAGE_BUDGET_POLICY
        or manifest.get("offline_release_policy")
        != "mandatory-authenticated-kagemusha-v4-activation-height-2"
        or manifest.get("offline_asset_definition_id") != OFFLINE_ASSET_ID
        or manifest.get("offline_asset_scale") != OFFLINE_ASSET_SCALE
    ):
        fail("reset manifest is not the exact bounded Taira v21 projection")
    if manifest.get("source_commit") != expected_source_commit:
        fail("reset manifest source commit does not match verified admission")
    if manifest.get("irohad_sha256") != expected_binary_sha256:
        fail("reset manifest binary does not match the verified admission receipt")
    _, signed_genesis_info = require_manifest_hash(
        manifest,
        "signed_genesis_sha256",
        bundle / "genesis.signed.nrt",
        64 * 1024 * 1024,
    )
    require_manifest_hash(
        manifest,
        "unsigned_genesis_sha256",
        bundle / "genesis.json",
        64 * 1024 * 1024,
    )
    require_manifest_hash(
        manifest,
        "base_config_sha256",
        bundle / "base-config.toml",
        MAX_CONFIG_BYTES,
    )
    operator_raw, _ = read_regular(bundle / "operator-identity.json", 64 * 1024)
    operator_sha = hashlib.sha256(operator_raw).hexdigest()
    if operator_sha != manifest.get("operator_identity_sha256"):
        fail("operator identity does not match the reset manifest")
    release = validate_release(
        bundle,
        manifest,
        owner_uid,
        owner_gid,
    )
    validate_operator_release_identity(operator_raw, release)
    if (
        release.source_root.stat().st_dev
        != existing_ancestor(release.installed_root).stat().st_dev
    ):
        fail(
            "Kagemusha release source and root-owned release store are on "
            "different filesystems; one-copy atomic deployment is impossible"
        )

    rendered = bundle / "rendered"
    require_exact_names(rendered, {"genesis.json", *SLUGS}, "rendered validator root")
    rendered_genesis_sha, _ = sha256_regular(
        rendered / "genesis.json", 64 * 1024 * 1024
    )
    if rendered_genesis_sha != manifest.get("unsigned_genesis_sha256"):
        fail("rendered genesis differs from the reset manifest")
    config_hashes = manifest.get("configs")
    empty_hashes = manifest.get("prewarmed_storage_sha256")
    if not isinstance(config_hashes, dict) or set(config_hashes) != set(SLUGS):
        fail("reset manifest lacks the exact four validator config identities")
    if not isinstance(empty_hashes, dict) or empty_hashes != {
        slug: EMPTY_TREE_SHA256 for slug in SLUGS
    }:
        fail("reset manifest does not seal four empty storage trees")
    peers: list[PeerPlan] = []
    peer_columns = (SLUGS, LABELS, TORII_PORTS, P2P_PORTS)
    if any(len(column) != PEER_COUNT for column in peer_columns):
        fail("internal Taira peer projection is not exactly four entries")
    for index, (slug, label, torii_port, p2p_port) in enumerate(
        zip(*peer_columns), start=1
    ):
        workdir = rendered / slug
        require_exact_names(workdir, VALIDATOR_NAMES, f"{slug} runtime root")
        workdir_info = require_private_entry(
            workdir, owner_uid, owner_gid, directory=True
        )
        storage = workdir / "storage"
        storage_info = require_private_entry(
            storage, owner_uid, owner_gid, directory=True
        )
        if any(storage.iterdir()):
            fail(f"fresh-reset storage is not empty: {slug}")
        config_path = workdir / "config.toml"
        config_sha, config_info = sha256_regular(config_path, MAX_CONFIG_BYTES)
        expected_config_sha = config_hashes.get(slug)
        if (
            not isinstance(expected_config_sha, str)
            or config_sha != expected_config_sha
        ):
            fail(f"validator config does not match the reset manifest: {slug}")
        config = parse_toml(config_path, owner_uid, owner_gid)
        validate_config_projection(
            config,
            bundle,
            release.installed_root,
            torii_port=torii_port,
            p2p_port=p2p_port,
        )
        peers.append(
            PeerPlan(
                number=index,
                label=label,
                slug=slug,
                torii_port=torii_port,
                p2p_port=p2p_port,
                workdir=workdir,
                storage=storage,
                config=config_path,
                config_sha256=config_sha,
                config_identity=metadata_identity(config_info),
                workdir_identity=metadata_identity(workdir_info),
                storage_identity=metadata_identity(storage_info),
                workdir_device=workdir_info.st_dev,
                workdir_inode=workdir_info.st_ino,
                storage_device=storage_info.st_dev,
                storage_inode=storage_info.st_ino,
            )
        )

    free_by_device = require_filesystem_headroom(
        [
            bundle,
            release.installed_root,
            *(peer.storage for peer in peers),
            INSTALL_ROOT / "runtime",
        ],
        minimum_free_bytes,
    )
    free_bytes = min(free for _, free in free_by_device)
    latencies = [
        measure_read_only_fsync(peer.storage, maximum_fsync_latency_ms)
        for peer in peers
    ]
    return BundlePlan(
        root=bundle,
        owner_uid=owner_uid,
        owner_gid=owner_gid,
        runtime_user=runtime_user,
        runtime_group=runtime_group,
        manifest=manifest,
        manifest_sha256=manifest_sha256,
        manifest_identity=metadata_identity(manifest_info),
        signed_genesis_identity=metadata_identity(signed_genesis_info),
        release=release,
        peers=tuple(peers),
        bundle_bytes=bundle_bytes,
        free_bytes=free_bytes,
        free_bytes_by_device=free_by_device,
        fsync_latency_ms=max(latencies),
    )


def require_root_controlled_file(path: Path, *, executable: bool) -> os.stat_result:
    """Require a root-owned path no runtime user can replace or rewrite."""

    canonical_path(path, "root-controlled file")
    components = [*reversed(path.parents), path]
    for index, component in enumerate(components):
        info = component.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or info.st_uid != 0
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail(f"root-controlled path has an unsafe component: {component}")
        if index + 1 == len(components):
            if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                fail(
                    f"root-controlled source is not a single-link regular file: {path}"
                )
            if executable and not info.st_mode & 0o111:
                fail(f"root-controlled source is not executable: {path}")
        elif not stat.S_ISDIR(info.st_mode):
            fail(f"root-controlled ancestor is not a directory: {component}")
        require_acl_free_path(component, "root-controlled path component")
    return path.lstat()


@dataclasses.dataclass(frozen=True)
class AdmissionPlan:
    """One complete archive verification bound to immutable deployment bytes."""

    archive: Path
    archive_state: rollout_admission.StableFile
    authority_dir: Path
    replay_ledger: Path
    receipt_id: str
    archive_sha256: str
    source_commit: str
    cargo_lock_sha256: str
    workspace_source_manifest_sha256: str
    reset_manifest_sha256: str
    binary_sha256: str
    supervisor_sha256: str
    validator_config_sha256: tuple[tuple[str, str], ...]
    restart_generation: str
    signer_fingerprint_sha256: str
    release_manifest_verifier_sha256: str


@dataclasses.dataclass(frozen=True)
class SourcePlan:
    """Authenticated release binary and supervisor source identities."""

    binary: Path
    binary_sha256: str
    supervisor: Path
    supervisor_sha256: str
    python: Path


def validate_supervisor_python(path: Path) -> Path:
    """Require the root-controlled macOS system Python and a 3.9-compatible ABI."""

    python = canonical_path(path, "supervisor Python")
    if python != DEFAULT_SUPERVISOR_PYTHON:
        fail(f"supervisor Python must be exactly {DEFAULT_SUPERVISOR_PYTHON}")
    require_root_controlled_file(python, executable=True)
    try:
        probe = subprocess.run(
            [
                str(python),
                "-I",
                "-S",
                "-c",
                (
                    "import sys;"
                    "print('%d.%d.%d' % "
                    "(sys.version_info.major,sys.version_info.minor,"
                    "sys.version_info.micro))"
                ),
            ],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
            timeout=5,
            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise DeploymentError("supervisor Python version probe failed") from error
    version_text = probe.stdout.strip()
    if (
        probe.returncode != 0
        or re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version_text) is None
    ):
        fail("supervisor Python did not return one bounded semantic version")
    major, minor, _patch = (int(part) for part in version_text.split("."))
    if major != 3 or minor < 9:
        fail("supervisor Python must be Python >=3.9,<4")
    return python


def validate_sources(
    args: argparse.Namespace,
    bundle: BundlePlan,
    admission: AdmissionPlan,
) -> SourcePlan:
    """Authenticate root binary and owner-private supervisor without executing either."""

    binary = canonical_path(args.binary, "validator binary")
    require_root_controlled_file(binary, executable=True)
    binary_sha, _ = sha256_regular(binary, MAX_BINARY_BYTES)
    if binary_sha != admission.binary_sha256:
        fail("validator binary does not match the verified admission receipt")

    supervisor = canonical_path(args.supervisor, "supervisor source")
    supervisor_info = supervisor.lstat()
    if (
        not stat.S_ISREG(supervisor_info.st_mode)
        or stat.S_ISLNK(supervisor_info.st_mode)
        or supervisor_info.st_nlink != 1
        or supervisor_info.st_uid != bundle.owner_uid
        or supervisor_info.st_gid != bundle.owner_gid
        or stat.S_IMODE(supervisor_info.st_mode) & 0o077
    ):
        fail("supervisor source is not an owner-private runtime-user file")
    supervisor_sha, _ = sha256_regular(supervisor, 4 * 1024 * 1024)
    if supervisor_sha != admission.supervisor_sha256:
        fail("supervisor source does not match the verified admission receipt")

    python = validate_supervisor_python(args.supervisor_python)
    return SourcePlan(
        binary=binary,
        binary_sha256=binary_sha,
        supervisor=supervisor,
        supervisor_sha256=supervisor_sha,
        python=python,
    )


def _stable_admission_file(
    path: Path, label: str
) -> rollout_admission.StableFile:
    """Capture one verifier-library stable file identity with a local error."""

    try:
        return rollout_admission.stable_hash_path(path)
    except rollout_admission.ReleaseArtifactError as error:
        raise DeploymentError(f"cannot stably read {label}: {error}") from error


def require_protected_replay_ledger(
    path: Path,
) -> rollout_admission.ReplayLedgerSnapshot:
    """Require the sole canonical root-controlled deployment replay ledger."""

    if path != ADMISSION_REPLAY_LEDGER:
        fail("deployment admission must use the canonical replay-ledger path")
    ledger = canonical_path(path, "deployment admission replay ledger")
    info = require_root_controlled_file(ledger, executable=False)
    if (
        info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) != ADMISSION_REPLAY_LEDGER_MODE
    ):
        fail("deployment admission replay ledger must be exact root:wheel 0644")
    try:
        snapshot = rollout_admission.load_replay_ledger(ledger)
    except (
        rollout_admission.ReleaseArtifactError,
        rollout_admission.TairaRolloutAdmissionError,
    ) as error:
        raise DeploymentError(
            f"invalid deployment admission replay ledger: {error}"
        ) from error
    after = require_root_controlled_file(ledger, executable=False)
    if (
        metadata_identity(after) != metadata_identity(info)
        or _stable_admission_file(ledger, "deployment admission replay ledger")
        != snapshot.file
    ):
        fail("deployment admission replay ledger changed during validation")
    return snapshot


def verify_deployment_admission(args: argparse.Namespace) -> AdmissionPlan:
    """Run the archive-only verifier with every independent trust input."""

    archive = canonical_path(args.admission_archive, "admission archive")
    authority_dir = canonical_path(
        args.admission_authority_dir, "admission authority directory"
    )
    verifier = canonical_path(
        args.release_manifest_verifier, "release-manifest verifier"
    )
    ledger = ADMISSION_REPLAY_LEDGER
    require_protected_replay_ledger(ledger)
    before_archive = _stable_admission_file(archive, "admission archive")
    source = rollout_admission.SourceIdentity(
        commit=args.expected_source_commit,
        cargo_lock_sha256=args.expected_cargo_lock_sha256,
        workspace_source_manifest_sha256=(
            args.expected_workspace_source_manifest_sha256
        ),
    )
    try:
        result = rollout_admission.verify_admission(
            archive_path=archive,
            authority_dir=authority_dir,
            expected_source=source,
            expected_receipt_id=args.expected_receipt_id,
            replay_ledger_path=ledger,
            trusted_signing_fingerprint=args.trusted_signing_fingerprint,
            release_manifest_verifier_path=verifier,
            trusted_release_manifest_verifier_sha256=(
                args.trusted_release_manifest_verifier_sha256
            ),
        )
    except (
        OSError,
        ValueError,
        rollout_admission.ReleaseArtifactError,
        rollout_admission.ReleaseManifestSignatureError,
        rollout_admission.TairaRolloutAdmissionError,
        rollout_admission.tarfile.TarError,
    ) as error:
        raise DeploymentError(
            f"rollout admission verification failed: {error}"
        ) from error
    after_archive = _stable_admission_file(archive, "admission archive")
    if before_archive != after_archive:
        fail("admission archive was substituted around verification")

    expected_fields = {
        "archive_sha256",
        "deployment_performed",
        "linux_authority_manifest_sha256",
        "macos_end_block_hash",
        "macos_end_height",
        "peer_count",
        "receipt_id",
        "release_manifest_sha256",
        "release_manifest_verifier_sha256",
        "reset_manifest_sha256",
        "restart_generation",
        "schema",
        "schema_version",
        "signer_fingerprint_sha256",
        "source",
        "supervisor_sha256",
        "validator_binary_sha256",
        "validator_config_sha256",
        "verified",
    }
    if set(result) != expected_fields:
        fail("rollout admission verifier returned a non-canonical result shape")
    if (
        result["verified"] is not True
        or result["deployment_performed"] is not False
        or result["schema"] != rollout_admission.VERIFICATION_SCHEMA
        or result["schema_version"] != rollout_admission.VERIFICATION_SCHEMA_VERSION
        or result["peer_count"] != PEER_COUNT
        or result["receipt_id"] != args.expected_receipt_id
        or result["archive_sha256"] != before_archive.sha256
        or result["source"] != source.as_dict()
        or result["signer_fingerprint_sha256"]
        != args.trusted_signing_fingerprint
        or result["release_manifest_verifier_sha256"]
        != args.trusted_release_manifest_verifier_sha256
    ):
        fail("rollout admission verifier result does not match trusted inputs")

    raw_configs = result["validator_config_sha256"]
    if not isinstance(raw_configs, dict) or set(raw_configs) != set(SLUGS):
        fail("rollout admission did not bind the exact four validator configs")
    config_digests = tuple(
        (slug, require_sha256(raw_configs[slug], f"verified {slug} config SHA-256"))
        for slug in SLUGS
    )
    return AdmissionPlan(
        archive=archive,
        archive_state=before_archive,
        authority_dir=authority_dir,
        replay_ledger=ledger,
        receipt_id=require_sha256(result["receipt_id"], "verified receipt ID"),
        archive_sha256=require_sha256(
            result["archive_sha256"], "verified archive SHA-256"
        ),
        source_commit=args.expected_source_commit,
        cargo_lock_sha256=args.expected_cargo_lock_sha256,
        workspace_source_manifest_sha256=(
            args.expected_workspace_source_manifest_sha256
        ),
        reset_manifest_sha256=require_sha256(
            result["reset_manifest_sha256"],
            "verified reset manifest SHA-256",
        ),
        binary_sha256=require_sha256(
            result["validator_binary_sha256"],
            "verified validator binary SHA-256",
        ),
        supervisor_sha256=require_sha256(
            result["supervisor_sha256"], "verified supervisor SHA-256"
        ),
        validator_config_sha256=config_digests,
        restart_generation=require_sha256(
            result["restart_generation"], "verified restart generation"
        ),
        signer_fingerprint_sha256=args.trusted_signing_fingerprint,
        release_manifest_verifier_sha256=(
            args.trusted_release_manifest_verifier_sha256
        ),
    )


def require_admission_archive_unchanged(admission: AdmissionPlan) -> None:
    """Reject archive replacement or byte changes after successful verification."""

    if (
        _stable_admission_file(admission.archive, "admission archive")
        != admission.archive_state
    ):
        fail("verified admission archive was substituted before rollout")


def require_inputs_match_admission(
    bundle: BundlePlan,
    sources: SourcePlan,
    admission: AdmissionPlan,
) -> None:
    """Bind every deployable byte identity to the verified signed receipt."""

    if (
        bundle.manifest_sha256 != admission.reset_manifest_sha256
        or sources.binary_sha256 != admission.binary_sha256
        or sources.supervisor_sha256 != admission.supervisor_sha256
        or bundle.manifest.get("source_commit") != admission.source_commit
        or tuple((peer.slug, peer.config_sha256) for peer in bundle.peers)
        != admission.validator_config_sha256
    ):
        fail("deployment inputs do not match the verified admission receipt")


def require_admission_bound_inputs_unchanged(
    bundle: BundlePlan,
    sources: SourcePlan,
    admission: AdmissionPlan,
) -> None:
    """Recheck receipt-bound mutable sources under the deployment lock."""

    require_bundle_runtime_unchanged(bundle)
    binary_sha256, _ = sha256_regular(sources.binary, MAX_BINARY_BYTES)
    supervisor_sha256, _ = sha256_regular(sources.supervisor, 4 * 1024 * 1024)
    if (
        binary_sha256 != admission.binary_sha256
        or supervisor_sha256 != admission.supervisor_sha256
    ):
        fail("receipt-bound binary or supervisor changed after admission")
    require_inputs_match_admission(bundle, sources, admission)


@dataclasses.dataclass(frozen=True)
class ProcessInfo:
    """Redaction-safe process identity used for exact parent/owner checks."""

    pid: int
    ppid: int
    uid: int
    argv: tuple[str, ...]


@dataclasses.dataclass(frozen=True)
class OldManagedIdentity:
    """Expected supervisor and child command identity for one rollback job."""

    supervisor_uid: int
    supervisor_argv: tuple[str, ...]
    child_uid: int
    child_argv: tuple[str, ...]
    pid_file: Path
    pid_file_gid: int
    child_was_present: bool


@dataclasses.dataclass(frozen=True)
class PlistSnapshot:
    """Exact old LaunchDaemon bytes, ownership, and managed process identity."""

    path: Path
    body: bytes
    mode: int
    uid: int
    gid: int
    managed: OldManagedIdentity


class SystemOps:
    """Small injectable boundary around launchd and process operations."""

    def run(self, command: Sequence[str]) -> subprocess.CompletedProcess[str]:
        """Run one noninteractive command without echoing its output on failure."""

        try:
            return subprocess.run(
                command,
                check=False,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                text=True,
                timeout=60,
            )
        except subprocess.TimeoutExpired as error:
            raise DeploymentError("bounded system command timed out") from error

    def launchd_print(self, label: str) -> Optional[str]:
        """Return launchd's system-domain record, or ``None`` when absent."""

        result = self.run(["/bin/launchctl", "print", f"system/{label}"])
        return result.stdout if result.returncode == 0 else None

    def bootout(self, label: str) -> None:
        """Unload one exact system-domain job."""

        result = self.run(["/bin/launchctl", "bootout", f"system/{label}"])
        if result.returncode != 0:
            fail(f"launchd bootout failed for {label} (status {result.returncode})")

    def bootstrap(self, plist: Path) -> None:
        """Load one exact LaunchDaemon plist."""

        result = self.run(["/bin/launchctl", "bootstrap", "system", str(plist)])
        if result.returncode != 0:
            fail(
                f"launchd bootstrap failed for {plist.name} (status {result.returncode})"
            )

    def inspect_process(self, pid: int) -> ProcessInfo:
        """Read parent, uid, and complete argv for one macOS process."""

        result = self.run(
            [
                "/bin/ps",
                "-ww",
                "-p",
                str(pid),
                "-o",
                "ppid=",
                "-o",
                "uid=",
                "-o",
                "command=",
            ]
        )
        if result.returncode != 0 or not result.stdout.strip():
            fail(f"managed process is not running: pid {pid}")
        fields = result.stdout.strip().split(maxsplit=2)
        if len(fields) != 3:
            fail(f"could not parse managed process identity: pid {pid}")
        try:
            argv = tuple(shlex.split(fields[2]))
            return ProcessInfo(
                pid=pid, ppid=int(fields[0]), uid=int(fields[1]), argv=argv
            )
        except (ValueError, TypeError) as error:
            raise DeploymentError(
                f"could not parse managed process identity: pid {pid}"
            ) from error

    def child_pids(self, parent_pid: int) -> tuple[int, ...]:
        """Return the stable PID inventory currently parented by one process."""

        result = self.run(["/bin/ps", "-axo", "pid=", "-o", "ppid="])
        if result.returncode != 0:
            fail(f"could not inspect children of managed process: pid {parent_pid}")
        children: list[int] = []
        for line in result.stdout.splitlines():
            fields = line.split()
            if len(fields) != 2:
                fail("could not parse the managed process child inventory")
            try:
                pid, ppid = (int(field) for field in fields)
            except ValueError as error:
                raise DeploymentError(
                    "could not parse the managed process child inventory"
                ) from error
            if pid < 1 or ppid < 0:
                fail("managed process child inventory contains an invalid PID")
            if ppid == parent_pid:
                children.append(pid)
        return tuple(sorted(children))

    def terminate(self, pid: int) -> None:
        """Terminate exactly one already-authenticated managed child."""

        os.kill(pid, signal.SIGTERM)

    def process_exists(self, pid: int) -> bool:
        """Return whether a process still exists."""

        result = self.run(["/bin/ps", "-p", str(pid), "-o", "pid="])
        return result.returncode == 0 and bool(result.stdout.strip())


def launchd_pid(record: Optional[str], label: str) -> int:
    """Extract one positive supervisor PID from launchd's printed record."""

    if record is None:
        fail(f"launchd job is not loaded: {label}")
    matches = re.findall(r"(?m)^\s*pid\s*=\s*([0-9]+)\s*$", record)
    if len(matches) != 1 or int(matches[0]) <= 1:
        fail(f"launchd job has no unique running supervisor PID: {label}")
    return int(matches[0])


def required_option(argv: tuple[str, ...], option: str, label: str) -> str:
    """Return one exact option value from a managed supervisor command."""

    indices = [index for index, value in enumerate(argv) if value == option]
    if (
        len(indices) != 1
        or indices[0] + 1 >= len(argv)
        or argv[indices[0] + 1].startswith("--")
    ):
        fail(f"{label} supervisor lacks one exact {option} argument")
    return argv[indices[0] + 1]


def inspect_old_managed_identity(
    payload: dict[str, Any],
    label: str,
    supervisor_pid: int,
    ops: SystemOps,
    *,
    allow_absent_child: bool = False,
) -> OldManagedIdentity:
    """Authenticate one old launchd supervisor and its exact managed child."""

    arguments = payload.get("ProgramArguments")
    runtime_user = payload.get("UserName")
    runtime_group = payload.get("GroupName")
    if (
        not isinstance(arguments, list)
        or not all(isinstance(value, str) for value in arguments)
        or not isinstance(runtime_user, str)
        or not isinstance(runtime_group, str)
    ):
        fail(f"old LaunchDaemon is not an explicit supervised job: {label}")
    try:
        uid = pwd.getpwnam(runtime_user).pw_uid
        gid = grp.getgrnam(runtime_group).gr_gid
    except KeyError as error:
        raise DeploymentError(
            f"old LaunchDaemon runtime identity is unknown: {label}"
        ) from error
    supervisor_argv = tuple(arguments)
    supervisor = ops.inspect_process(supervisor_pid)
    if (
        supervisor.ppid != 1
        or supervisor.uid != uid
        or supervisor.argv != supervisor_argv
    ):
        fail(f"old LaunchDaemon supervisor identity differs from its plist: {label}")
    pid_file = Path(required_option(supervisor_argv, "--pid-file", label))
    binary = required_option(supervisor_argv, "--binary", label)
    config = required_option(supervisor_argv, "--config", label)
    if (
        not pid_file.is_absolute()
        or not Path(binary).is_absolute()
        or not Path(config).is_absolute()
    ):
        fail(f"old LaunchDaemon contains a non-absolute managed path: {label}")
    child_argv = (binary, "--sora", "--config", config)
    child_was_present = verify_old_managed_child(
        label,
        supervisor_pid,
        uid,
        gid,
        pid_file,
        child_argv,
        ops,
        allow_absent=allow_absent_child,
    )
    return OldManagedIdentity(
        supervisor_uid=uid,
        supervisor_argv=supervisor_argv,
        child_uid=uid,
        child_argv=child_argv,
        pid_file=pid_file,
        pid_file_gid=gid,
        child_was_present=child_was_present,
    )


def verify_old_managed_child(
    label: str,
    supervisor_pid: int,
    uid: int,
    gid: int,
    pid_file: Path,
    child_argv: tuple[str, ...],
    ops: SystemOps,
    *,
    allow_absent: bool,
) -> bool:
    """Authenticate one old child or an explicitly allowed childless supervisor."""

    try:
        pid_file.lstat()
    except FileNotFoundError:
        if not allow_absent:
            fail(f"old LaunchDaemon managed-child PID file is absent: {label}")
        for _sample in range(2):
            try:
                pid_file.lstat()
            except FileNotFoundError:
                pass
            else:
                fail(
                    "old LaunchDaemon managed-child PID file appeared during "
                    f"capture: {label}"
                )
            if ops.child_pids(supervisor_pid):
                fail(
                    "old LaunchDaemon PID file is absent while its supervisor "
                    f"still owns a child: {label}"
                )
            try:
                pid_file.lstat()
            except FileNotFoundError:
                continue
            fail(
                "old LaunchDaemon managed-child PID file appeared during "
                f"capture: {label}"
            )
        return False

    child_pid = parse_pid_file(pid_file, uid, gid)
    child = ops.inspect_process(child_pid)
    if child.ppid != supervisor_pid or child.uid != uid or child.argv != child_argv:
        fail(f"old LaunchDaemon child identity differs from its supervisor: {label}")
    if ops.child_pids(supervisor_pid) != (child_pid,):
        fail(
            f"old LaunchDaemon supervisor does not own exactly its managed child: {label}"
        )
    return True


def verify_restored_snapshot(snapshot: PlistSnapshot, ops: SystemOps) -> None:
    """Require a restored old job to own its exact supervisor and child."""

    label = snapshot.path.stem
    supervisor_pid = launchd_pid(ops.launchd_print(label), label)
    supervisor = ops.inspect_process(supervisor_pid)
    expected = snapshot.managed
    if (
        supervisor.ppid != 1
        or supervisor.uid != expected.supervisor_uid
        or supervisor.argv != expected.supervisor_argv
    ):
        fail(f"restored LaunchDaemon supervisor identity is wrong: {label}")
    verify_old_managed_child(
        label,
        supervisor_pid,
        expected.child_uid,
        expected.pid_file_gid,
        expected.pid_file,
        expected.child_argv,
        ops,
        allow_absent=not expected.child_was_present,
    )


def capture_old_cohort(
    ops: SystemOps, *, allow_absent_child: bool = False
) -> tuple[PlistSnapshot, ...]:
    """Read the exact four old plists and require all four jobs loaded."""

    snapshots: list[PlistSnapshot] = []
    for label in LABELS:
        path = LAUNCH_DAEMONS / f"{label}.plist"
        body, info = read_regular(path, MAX_MANIFEST_BYTES)
        if info.st_uid != 0 or stat.S_IMODE(info.st_mode) & 0o022:
            fail(f"old LaunchDaemon plist is not root-controlled: {path}")
        try:
            payload = plistlib.loads(body)
        except Exception as error:
            raise DeploymentError(
                f"old LaunchDaemon plist is invalid: {path}"
            ) from error
        if not isinstance(payload, dict) or payload.get("Label") != label:
            fail(f"old LaunchDaemon plist label mismatch: {path}")
        supervisor_pid = launchd_pid(ops.launchd_print(label), label)
        managed = inspect_old_managed_identity(
            payload,
            label,
            supervisor_pid,
            ops,
            allow_absent_child=allow_absent_child,
        )
        snapshots.append(
            PlistSnapshot(
                path=path,
                body=body,
                mode=stat.S_IMODE(info.st_mode),
                uid=info.st_uid,
                gid=info.st_gid,
                managed=managed,
            )
        )
    return tuple(snapshots)


def wait_launchd_cohort_running(
    labels: Sequence[str], ops: SystemOps, timeout_seconds: float
) -> list[str]:
    """Wait for every loaded job to publish one positive supervisor PID."""

    deadline = time.monotonic() + timeout_seconds
    pending = set(labels)
    while pending and time.monotonic() < deadline:
        for label in tuple(pending):
            try:
                launchd_pid(ops.launchd_print(label), label)
            except DeploymentError:
                continue
            pending.remove(label)
        if pending:
            time.sleep(0.25)
    return sorted(pending)


def fsync_directory(path: Path) -> None:
    """Durably flush one directory after a publication rename."""

    descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_DIRECTORY", 0))
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def ensure_root_directory(path: Path, mode: int) -> None:
    """Create or validate a root-owned non-writable installation directory."""

    if not path.is_absolute():
        fail(f"root installation directory is not absolute: {path}")
    pending: list[Path] = []
    current = path
    while not current.exists():
        pending.append(current)
        current = current.parent
    for component in [*reversed(current.parents), current]:
        info = component.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid != 0
            or stat.S_IMODE(info.st_mode) & 0o022
        ):
            fail(f"unsafe root installation ancestor: {component}")
        require_acl_free_path(component, "root installation ancestor")
    for component in reversed(pending):
        os.mkdir(component, mode)
        os.chown(component, 0, 0)
        os.chmod(component, mode)
        require_acl_free_path(component, "new root installation directory")
        fsync_directory(component.parent)
    info = path.lstat()
    if (
        stat.S_ISLNK(info.st_mode)
        or not stat.S_ISDIR(info.st_mode)
        or info.st_uid != 0
        or info.st_gid != 0
        or stat.S_IMODE(info.st_mode) & 0o022
    ):
        fail(f"unsafe root installation directory: {path}")
    require_acl_free_path(path, "root installation directory")


def ensure_runtime_directory(path: Path, uid: int, gid: int) -> None:
    """Create or validate one owner-private runtime state directory."""

    if path.exists() or path.is_symlink():
        info = path.lstat()
        if (
            stat.S_ISLNK(info.st_mode)
            or not stat.S_ISDIR(info.st_mode)
            or info.st_uid != uid
            or info.st_gid != gid
            or stat.S_IMODE(info.st_mode) != 0o700
        ):
            fail(f"unsafe runtime state directory: {path}")
        return
    os.mkdir(path, 0o700)
    os.chown(path, uid, gid)
    os.chmod(path, 0o700)
    fsync_directory(path.parent)


@contextlib.contextmanager
def exclusive_deployment_lock() -> Any:
    """Hold one root-owned nonblocking lock across capture, apply, and rollback."""

    ensure_root_directory(INSTALL_ROOT, 0o755)
    existed = DEPLOYMENT_LOCK.exists()
    descriptor = os.open(
        DEPLOYMENT_LOCK,
        os.O_RDWR
        | os.O_CREAT
        | getattr(os, "O_NOFOLLOW", 0)
        | getattr(os, "O_CLOEXEC", 0),
        0o600,
    )
    try:
        info = os.fstat(descriptor)
        if (
            not stat.S_ISREG(info.st_mode)
            or info.st_nlink != 1
            or info.st_uid != 0
            or info.st_gid != 0
            or stat.S_IMODE(info.st_mode) != 0o600
        ):
            fail("deployment lock is not an exact root:wheel 0600 regular file")
        if not existed:
            os.fsync(descriptor)
            fsync_directory(DEPLOYMENT_LOCK.parent)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise DeploymentError(
                "another Taira reset controller holds the deployment lock"
            ) from error
        yield
    finally:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
        finally:
            os.close(descriptor)


def install_immutable(
    source: Path, destination: Path, expected_sha256: str
) -> os.stat_result:
    """Install authenticated bytes as root:wheel 0555 without overwriting."""

    if destination.exists() or destination.is_symlink():
        require_root_controlled_file(destination, executable=True)
        actual, info = sha256_regular(destination, MAX_BINARY_BYTES)
        if (
            actual != expected_sha256
            or stat.S_IMODE(info.st_mode) != 0o555
            or info.st_gid != 0
        ):
            fail(
                f"existing immutable installation has the wrong identity: {destination}"
            )
        return info
    parent = destination.parent
    if not parent.exists():
        ensure_root_directory(parent, 0o755)
    parent_info = parent.lstat()
    if stat.S_IMODE(parent_info.st_mode) & 0o022:
        fail(f"immutable installation parent is writable: {parent}")
    temporary = parent / f".{destination.name}.{os.getpid()}.tmp"
    source_fd = -1
    output_fd = -1
    try:
        source_fd, source_before = open_regular(source, MAX_BINARY_BYTES)
        output_fd = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            0o500,
        )
        digest = hashlib.sha256()
        while chunk := os.read(source_fd, 1024 * 1024):
            digest.update(chunk)
            offset = 0
            while offset < len(chunk):
                offset += os.write(output_fd, chunk[offset:])
        if metadata_identity(source_before) != metadata_identity(os.fstat(source_fd)):
            fail(f"immutable source changed while copied: {source}")
        if digest.hexdigest() != expected_sha256:
            fail(f"immutable source digest changed while copied: {source}")
        os.fsync(output_fd)
        os.fchown(output_fd, 0, 0)
        os.fchmod(output_fd, 0o555)
        os.fsync(output_fd)
        clear_owned_temporary_acl(
            temporary, output_fd, "immutable installation staging file"
        )
        os.close(source_fd)
        source_fd = -1
        os.close(output_fd)
        output_fd = -1
        os.replace(temporary, destination)
        fsync_directory(parent)
    finally:
        if source_fd >= 0:
            os.close(source_fd)
        if output_fd >= 0:
            os.close(output_fd)
        temporary.unlink(missing_ok=True)
    actual, info = sha256_regular(destination, MAX_BINARY_BYTES)
    if actual != expected_sha256:
        fail(f"immutable installation changed after publication: {destination}")
    published = require_root_controlled_file(destination, executable=True)
    if metadata_identity(published) != metadata_identity(info):
        fail(
            f"immutable installation identity changed after publication: {destination}"
        )
    return published


def atomic_replace_owned(
    path: Path, body: bytes, *, mode: int, uid: int, gid: int
) -> None:
    """Atomically replace a root-controlled plist with authenticated bytes."""

    if path.is_symlink():
        fail(f"refusing to replace symlink: {path}")
    temporary = path.parent / f".{path.name}.{os.getpid()}.tmp"
    descriptor = -1
    temporary_created = False
    try:
        descriptor = os.open(
            temporary,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0),
            mode,
        )
        temporary_created = True
        try:
            offset = 0
            while offset < len(body):
                offset += os.write(descriptor, body[offset:])
            os.fchown(descriptor, uid, gid)
            os.fchmod(descriptor, mode)
            os.fsync(descriptor)
            clear_owned_temporary_acl(
                temporary,
                descriptor,
                "root-controlled publication staging file",
            )
        finally:
            os.close(descriptor)
            descriptor = -1
        os.replace(temporary, path)
        fsync_directory(path.parent)
        require_root_controlled_file(path, executable=False)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if temporary_created:
            temporary.unlink(missing_ok=True)


@dataclasses.dataclass
class AdmissionReceiptConsumption:
    """One protected receipt publication awaiting the irreversible cutover."""

    admission: AdmissionPlan
    prior_payload: bytes
    consumed_payload: bytes
    rollout_started: bool = False

    def mark_rollout_started(self) -> None:
        """Commit replay consumption immediately before the first cohort change."""

        if self.rollout_started:
            fail("deployment rollout start was marked more than once")
        require_admission_archive_unchanged(self.admission)
        published = require_protected_replay_ledger(self.admission.replay_ledger)
        if published.payload != self.consumed_payload:
            fail("consumed admission receipt changed before rollout start")
        self.rollout_started = True


def _restore_unstarted_receipt_consumption(
    transaction: AdmissionReceiptConsumption,
) -> None:
    """Restore the exact prior ledger if this transaction still owns its update."""

    current = require_protected_replay_ledger(transaction.admission.replay_ledger)
    if current.payload == transaction.prior_payload:
        return
    if current.payload != transaction.consumed_payload:
        fail(
            "admission replay ledger changed outside this unstarted deployment; "
            "automatic restoration is unsafe"
        )
    atomic_replace_owned(
        transaction.admission.replay_ledger,
        transaction.prior_payload,
        mode=ADMISSION_REPLAY_LEDGER_MODE,
        uid=0,
        gid=0,
    )
    restored = require_protected_replay_ledger(transaction.admission.replay_ledger)
    if restored.payload != transaction.prior_payload:
        fail("admission replay ledger restoration did not persist exact prior bytes")


@contextlib.contextmanager
def consume_admission_receipt(
    admission: AdmissionPlan,
) -> Any:
    """Atomically consume one receipt, restoring it until rollout begins."""

    require_admission_archive_unchanged(admission)
    prior = require_protected_replay_ledger(admission.replay_ledger)
    if admission.receipt_id in prior.consumed_receipt_ids:
        fail("verified admission receipt was already consumed under deployment lock")
    consumed_ids = sorted((*prior.consumed_receipt_ids, admission.receipt_id))
    try:
        consumed_payload = rollout_admission.canonical_replay_ledger_bytes(
            consumed_ids
        )
    except rollout_admission.TairaRolloutAdmissionError as error:
        raise DeploymentError(
            f"cannot encode admission replay ledger: {error}"
        ) from error
    if len(consumed_payload) > rollout_admission.MAX_JSON_BYTES:
        fail("admission replay ledger has no capacity for another receipt")
    transaction = AdmissionReceiptConsumption(
        admission=admission,
        prior_payload=prior.payload,
        consumed_payload=consumed_payload,
    )
    try:
        atomic_replace_owned(
            admission.replay_ledger,
            consumed_payload,
            mode=ADMISSION_REPLAY_LEDGER_MODE,
            uid=0,
            gid=0,
        )
        published = require_protected_replay_ledger(admission.replay_ledger)
        if (
            published.payload != consumed_payload
            or published.consumed_receipt_ids != tuple(consumed_ids)
        ):
            fail("admission receipt consumption was not published atomically")
        yield transaction
        if not transaction.rollout_started:
            fail("deployment returned without beginning its admitted rollout")
        committed = require_protected_replay_ledger(admission.replay_ledger)
        if committed.payload != consumed_payload:
            fail("consumed admission receipt changed during rollout")
    except BaseException as deployment_error:
        if not transaction.rollout_started:
            try:
                _restore_unstarted_receipt_consumption(transaction)
            except BaseException as rollback_error:
                combined = DeploymentError(
                    "deployment did not begin and admission receipt rollback failed"
                )
                if hasattr(combined, "add_note"):
                    combined.add_note(
                        "deployment failure: "
                        f"{type(deployment_error).__name__}: {deployment_error}"
                    )
                raise combined from rollback_error
        raise


def render_plist(
    peer: PeerPlan,
    bundle: BundlePlan,
    sources: SourcePlan,
    *,
    installed_binary: Path,
    binary_info: os.stat_result,
    installed_supervisor: Path,
    runtime_root: Path,
    restart_generation: str,
) -> bytes:
    """Render one fresh known-argument LaunchDaemon with all five stat fields."""

    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    terminal_binding = supervisor_terminal_binding(
        sources.binary_sha256,
        binary_info,
        peer.config_sha256,
        restart_generation,
    )
    terminal_file = terminal_unhealthy_path(runtime_root, peer, terminal_binding)
    log_file = runtime_root / "logs" / f"validator-{peer.number}-supervisor.log"
    arguments = [
        str(sources.python),
        "-I",
        "-S",
        str(installed_supervisor),
        "--binary",
        str(installed_binary),
        "--binary-sha256",
        sources.binary_sha256,
        "--binary-device",
        str(binary_info.st_dev),
        "--binary-inode",
        str(binary_info.st_ino),
        "--binary-size",
        str(binary_info.st_size),
        "--binary-mtime-ns",
        str(binary_info.st_mtime_ns),
        "--binary-ctime-ns",
        str(binary_info.st_ctime_ns),
        "--config",
        str(peer.config),
        "--config-sha256",
        peer.config_sha256,
        "--workdir",
        str(peer.workdir),
        "--workdir-device",
        str(peer.workdir_device),
        "--workdir-inode",
        str(peer.workdir_inode),
        "--storage-dir",
        str(peer.storage),
        "--storage-device",
        str(peer.storage_device),
        "--storage-inode",
        str(peer.storage_inode),
        "--pid-file",
        str(pid_file),
        "--terminal-unhealthy-file",
        str(terminal_file),
        "--restart-generation",
        restart_generation,
        "--initial-backoff-seconds",
        "1.0",
        "--maximum-backoff-seconds",
        "30.0",
        "--stable-uptime-seconds",
        "120.0",
    ]
    payload = {
        "Label": peer.label,
        "ProgramArguments": arguments,
        "WorkingDirectory": str(peer.workdir),
        "RunAtLoad": True,
        "KeepAlive": True,
        "ThrottleInterval": 10,
        "ProcessType": "Background",
        "ExitTimeOut": 60,
        "AbandonProcessGroup": False,
        "UserName": bundle.runtime_user,
        "GroupName": bundle.runtime_group,
        "EnvironmentVariables": {
            "GENESIS": str(bundle.root / "genesis.signed.nrt"),
            "KURA_STORE_DIR": str(peer.storage / "kura"),
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "RUST_LOG": "info",
            "SNAPSHOT_STORE_DIR": str(peer.storage / "snapshot"),
            "ZK_HALO2_ENABLED": "true",
        },
        "StandardOutPath": str(log_file),
        "StandardErrorPath": str(log_file),
    }
    return plistlib.dumps(payload, fmt=plistlib.FMT_XML, sort_keys=True)


def require_mutable_bundle_identities(bundle: BundlePlan, *, phase: str) -> None:
    """Recheck every mutable manifest, genesis, config, and runtime identity."""

    manifest_raw, manifest_info = read_regular(
        bundle.root / "reset-manifest.json", MAX_MANIFEST_BYTES
    )
    if hashlib.sha256(manifest_raw).hexdigest() != bundle.manifest_sha256:
        fail(f"reset manifest changed {phase}")
    if metadata_identity(manifest_info) != bundle.manifest_identity:
        fail(f"reset manifest identity changed {phase}")
    signed_sha, signed_info = sha256_regular(
        bundle.root / "genesis.signed.nrt", 64 * 1024 * 1024
    )
    if signed_sha != bundle.manifest.get("signed_genesis_sha256"):
        fail(f"signed genesis changed {phase}")
    if metadata_identity(signed_info) != bundle.signed_genesis_identity:
        fail(f"signed genesis identity changed {phase}")
    for peer in bundle.peers:
        require_exact_names(peer.workdir, VALIDATOR_NAMES, f"{peer.slug} runtime root")
        workdir_info = require_private_entry(
            peer.workdir, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        storage_info = require_private_entry(
            peer.storage, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        config_sha, config_info = sha256_regular(peer.config, MAX_CONFIG_BYTES)
        if config_sha != peer.config_sha256:
            fail(f"validator config changed {phase}: {peer.slug}")
        if metadata_identity(config_info) != peer.config_identity:
            fail(f"validator config identity changed {phase}: {peer.slug}")
        if any(peer.storage.iterdir()):
            fail(f"fresh-reset storage changed {phase}: {peer.slug}")
        workdir_after = require_private_entry(
            peer.workdir, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        storage_after = require_private_entry(
            peer.storage, bundle.owner_uid, bundle.owner_gid, directory=True
        )
        if (
            metadata_identity(workdir_info) != peer.workdir_identity
            or metadata_identity(workdir_after) != peer.workdir_identity
            or metadata_identity(storage_info) != peer.storage_identity
            or metadata_identity(storage_after) != peer.storage_identity
        ):
            fail(f"fresh-reset runtime path changed {phase}: {peer.slug}")


def require_bundle_runtime_unchanged(bundle: BundlePlan) -> None:
    """Recheck mutable inputs and the private release before it is moved."""

    require_mutable_bundle_identities(bundle, phase="after preflight")
    current_paths = sorted(
        bundle.release.source_root.rglob("*"),
        key=lambda item: item.relative_to(bundle.release.source_root).as_posix(),
    )
    expected_paths = [seal.relative_path for seal in bundle.release.tree_seals[1:]]
    if [
        path.relative_to(bundle.release.source_root).as_posix()
        for path in current_paths
    ] != expected_paths:
        fail("Kagemusha release inventory changed after preflight")
    for seal in bundle.release.tree_seals:
        path = (
            bundle.release.source_root
            if seal.relative_path == "."
            else bundle.release.source_root / seal.relative_path
        )
        if metadata_identity(path.lstat()) != seal.identity:
            fail(
                f"Kagemusha release entry changed after preflight: {seal.relative_path}"
            )


def require_hardened_release_identity(bundle: BundlePlan) -> None:
    """Bind the moved release's immutable identity and exact hardened metadata."""

    root = bundle.release.installed_root
    current_paths = sorted(
        root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()
    )
    expected_paths = [seal.relative_path for seal in bundle.release.tree_seals[1:]]
    if [path.relative_to(root).as_posix() for path in current_paths] != expected_paths:
        fail("root-owned Kagemusha release inventory changed")
    for seal in bundle.release.tree_seals:
        path = root if seal.relative_path == "." else root / seal.relative_path
        info = path.lstat()
        expected = seal.identity
        expected_type = stat.S_IFMT(expected[2])
        if expected_type == stat.S_IFDIR:
            expected_mode = 0o550
        elif expected_type == stat.S_IFREG:
            expected_mode = 0o440
        else:
            fail(
                f"Kagemusha release preflight sealed an unsafe type: {seal.relative_path}"
            )
        immutable_identity = (
            info.st_dev,
            info.st_ino,
            stat.S_IFMT(info.st_mode),
            info.st_nlink,
            info.st_size,
            info.st_mtime_ns,
        )
        expected_identity = (
            expected[0],
            expected[1],
            expected_type,
            expected[5],
            expected[6],
            expected[7],
        )
        if immutable_identity != expected_identity:
            fail(f"root-owned Kagemusha release identity changed: {seal.relative_path}")
        if (
            info.st_uid != 0
            or info.st_gid != bundle.owner_gid
            or stat.S_IMODE(info.st_mode) != expected_mode
        ):
            fail(
                "root-owned Kagemusha release ownership or mode changed: "
                f"{seal.relative_path}"
            )


def require_post_qualification_cutover_identity(bundle: BundlePlan) -> None:
    """Recheck all reset inputs and the hardened release immediately pre-bootout."""

    require_mutable_bundle_identities(bundle, phase="during qualification")
    require_hardened_release_identity(bundle)


def rewrite_release_tree_ownership(
    root: Path,
    *,
    uid: int,
    gid: int,
    file_mode: int,
    directory_mode: int,
) -> None:
    """Rewrite one exact release tree without copying its multi-gigabyte files."""

    paths = sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix())
    files: list[Path] = []
    directories: list[Path] = [root]
    for path in paths:
        info = path.lstat()
        if stat.S_ISLNK(info.st_mode):
            fail(f"release tree contains a symlink during ownership rewrite: {path}")
        if stat.S_ISDIR(info.st_mode):
            directories.append(path)
        elif stat.S_ISREG(info.st_mode) and info.st_nlink == 1:
            files.append(path)
        else:
            fail(
                f"release tree contains an unsafe entry during ownership rewrite: {path}"
            )
    for path in files:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
        )
        try:
            os.fchown(descriptor, uid, gid)
            os.fchmod(descriptor, file_mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
    for path in sorted(directories, key=lambda item: len(item.parts), reverse=True):
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_NOFOLLOW", 0),
        )
        try:
            os.fchown(descriptor, uid, gid)
            os.fchmod(descriptor, directory_mode)
            os.fsync(descriptor)
        finally:
            os.close(descriptor)


def require_release_tree_content_seals(
    root: Path, seals: Sequence[ReleaseTreeSeal]
) -> None:
    """Recheck content identities after hardening, ignoring expected metadata changes."""

    paths = sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix())
    if [path.relative_to(root).as_posix() for path in paths] != [
        seal.relative_path for seal in seals[1:]
    ]:
        fail("Kagemusha release inventory changed during ownership hardening")
    for seal in seals:
        path = root if seal.relative_path == "." else root / seal.relative_path
        info = path.lstat()
        expected = seal.identity
        immutable_identity = (
            info.st_dev,
            info.st_ino,
            stat.S_IFMT(info.st_mode),
            info.st_nlink,
            info.st_size,
            info.st_mtime_ns,
        )
        expected_identity = (
            expected[0],
            expected[1],
            stat.S_IFMT(expected[2]),
            expected[5],
            expected[6],
            expected[7],
        )
        if immutable_identity != expected_identity:
            fail(
                "Kagemusha release content changed during ownership hardening: "
                f"{seal.relative_path}"
            )


def require_restored_release_source_identity(bundle: BundlePlan) -> None:
    """Require the failed move source to be fully private and inode-identical."""

    root = bundle.release.source_root
    current_paths = sorted(
        root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()
    )
    expected_paths = [seal.relative_path for seal in bundle.release.tree_seals[1:]]
    if [path.relative_to(root).as_posix() for path in current_paths] != expected_paths:
        fail("restored Kagemusha release inventory differs from preflight")
    for seal in bundle.release.tree_seals:
        path = root if seal.relative_path == "." else root / seal.relative_path
        info = path.lstat()
        expected = seal.identity
        expected_type = stat.S_IFMT(expected[2])
        if expected_type == stat.S_IFDIR:
            expected_mode = 0o700
        elif expected_type == stat.S_IFREG:
            expected_mode = 0o600
        else:
            fail(
                "Kagemusha release preflight sealed an unsafe source type: "
                f"{seal.relative_path}"
            )
        immutable_identity = (
            info.st_dev,
            info.st_ino,
            stat.S_IFMT(info.st_mode),
            info.st_nlink,
            info.st_size,
            info.st_mtime_ns,
        )
        expected_identity = (
            expected[0],
            expected[1],
            expected_type,
            expected[5],
            expected[6],
            expected[7],
        )
        if immutable_identity != expected_identity:
            fail(
                "restored Kagemusha release source changed identity: "
                f"{seal.relative_path}"
            )
        if (
            info.st_uid != bundle.owner_uid
            or info.st_gid != bundle.owner_gid
            or stat.S_IMODE(info.st_mode) != expected_mode
        ):
            fail(
                "restored Kagemusha release source is not owner-private: "
                f"{seal.relative_path}"
            )


def restore_failed_release_move(bundle: BundlePlan) -> None:
    """Best-effort restore and exact verification after hardening/move failure."""

    source = bundle.release.source_root
    destination = bundle.release.installed_root
    errors: list[str] = []
    source_present = source.exists() or source.is_symlink()
    destination_present = destination.exists() or destination.is_symlink()
    if destination_present and not source_present:
        try:
            if destination.is_symlink() or not destination.is_dir():
                fail("failed release move destination is not the sealed directory")
            os.rename(destination, source)
        except BaseException:
            errors.append("rename")
        else:
            try:
                fsync_directory(destination.parent)
                fsync_directory(source.parent)
            except BaseException:
                errors.append("directory-fsync")
    elif source_present and not destination_present:
        pass
    else:
        errors.append("path-state")

    source_present = source.exists() or source.is_symlink()
    destination_present = destination.exists() or destination.is_symlink()
    if source_present and not destination_present and not source.is_symlink():
        try:
            rewrite_release_tree_ownership(
                source,
                uid=bundle.owner_uid,
                gid=bundle.owner_gid,
                file_mode=0o600,
                directory_mode=0o700,
            )
        except BaseException:
            errors.append("ownership")
        try:
            require_restored_release_source_identity(bundle)
        except BaseException:
            errors.append("identity")
    elif "path-state" not in errors:
        errors.append("path-state")
    if errors:
        fail(
            "Kagemusha release hardening rollback is incomplete "
            f"({', '.join(errors)})"
        )


def move_release_to_root_store(bundle: BundlePlan) -> Path:
    """Move and harden the sole release copy in its content-addressed root store."""

    source = bundle.release.source_root
    destination = bundle.release.installed_root
    release_store = destination.parent
    ensure_root_directory(release_store, 0o755)
    if destination.exists() or destination.is_symlink():
        fail(f"content-addressed release destination already exists: {destination}")
    if source.stat().st_dev != release_store.stat().st_dev:
        fail("Kagemusha release move crossed filesystems")
    try:
        rewrite_release_tree_ownership(
            source,
            uid=0,
            gid=bundle.owner_gid,
            file_mode=0o440,
            directory_mode=0o550,
        )
        require_release_tree_content_seals(source, bundle.release.tree_seals)
        os.rename(source, destination)
        destination_info = destination.lstat()
        expected_root = bundle.release.tree_seals[0].identity
        if (
            destination_info.st_dev != expected_root[0]
            or destination_info.st_ino != expected_root[1]
        ):
            fail("Kagemusha release root changed during its atomic move")
        require_hardened_release_identity(bundle)
        fsync_directory(source.parent)
        fsync_directory(release_store)
    except BaseException as move_error:
        try:
            restore_failed_release_move(bundle)
        except BaseException as restore_error:
            combined = DeploymentError(
                "Kagemusha release move failed and its exact hardening rollback "
                "is incomplete"
            )
            if hasattr(combined, "add_note"):
                combined.add_note(
                    f"move failure: {type(move_error).__name__}: {move_error}"
                )
            raise combined from restore_error
        raise
    return destination


def restore_release_to_bundle(bundle: BundlePlan) -> None:
    """Restore the single release copy after a failed cohort rollout."""

    source = bundle.release.installed_root
    destination = bundle.release.source_root
    if destination.exists() or destination.is_symlink():
        fail("cannot restore Kagemusha release over an existing bundle path")
    if not source.is_dir() or source.is_symlink():
        fail("root-owned Kagemusha release is unavailable for rollback")
    os.rename(source, destination)
    fsync_directory(source.parent)
    fsync_directory(destination.parent)
    rewrite_release_tree_ownership(
        destination,
        uid=bundle.owner_uid,
        gid=bundle.owner_gid,
        file_mode=0o600,
        directory_mode=0o700,
    )


def prepare_qualification_seal_path(bundle: BundlePlan) -> Path:
    """Create the exact protected seal parent and require the seal absent."""

    path = qualification_seal_path(bundle.release.tree_sha256)
    ensure_root_directory(path.parent, 0o755)
    parent_info = path.parent.lstat()
    if (
        stat.S_ISLNK(parent_info.st_mode)
        or not stat.S_ISDIR(parent_info.st_mode)
        or parent_info.st_uid != 0
        or parent_info.st_gid != 0
        or stat.S_IMODE(parent_info.st_mode) != 0o755
    ):
        fail("Kagemusha qualification seal directory is not root:wheel 0755")
    try:
        path.lstat()
    except FileNotFoundError:
        return path
    fail(f"refusing to replace a preexisting Kagemusha qualification seal: {path}")


def authenticate_qualification_seal(path: Path) -> tuple[int, ...]:
    """Require one stable, bounded, immutable root-owned qualification seal."""

    descriptor, before = open_regular(path, MAX_QUALIFICATION_SEAL_BYTES)
    try:
        if (
            before.st_size == 0
            or before.st_uid != 0
            or before.st_gid != 0
            or stat.S_IMODE(before.st_mode) != 0o444
        ):
            fail("Kagemusha qualification seal is not root:wheel 0444")
        os.fsync(descriptor)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if metadata_identity(before) != metadata_identity(after):
        fail("Kagemusha qualification seal changed while authenticated")
    fsync_directory(path.parent)
    return metadata_identity(after)


def write_catalog_qualification_seal(
    installed_binary: Path,
    bundle: BundlePlan,
    seal_path: Path,
    *,
    runner: Callable[..., Any] = subprocess.run,
) -> tuple[int, ...]:
    """Run the exact candidate's one-time full catalog qualification gate."""

    if os.geteuid() != 0:
        fail("Kagemusha catalog qualification requires root")
    expected_path = qualification_seal_path(bundle.release.tree_sha256)
    if seal_path != expected_path:
        fail("Kagemusha qualification seal path is not release-bound")
    if len(bundle.peers) != PEER_COUNT:
        fail("catalog qualification requires exactly four peer configs")
    try:
        seal_path.lstat()
    except FileNotFoundError:
        pass
    else:
        fail("Kagemusha qualification seal appeared before candidate execution")
    validator_one = bundle.peers[0]
    try:
        result = runner(
            [
                str(installed_binary),
                "--sora",
                "--config",
                str(validator_one.config),
                "--check-config",
                "--write-kagemusha-catalog-qualification-seal",
                str(seal_path),
            ],
            check=False,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            timeout=CATALOG_QUALIFICATION_TIMEOUT_SECONDS,
            env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
        )
    except subprocess.TimeoutExpired as error:
        raise DeploymentError(
            "installed binary catalog qualification timed out"
        ) from error
    except OSError as error:
        raise DeploymentError(
            "installed binary catalog qualification could not execute"
        ) from error
    if result.returncode != 0:
        fail(
            "installed binary rejected validator config/genesis/catalog "
            f"during qualification (status={result.returncode})"
        )
    return authenticate_qualification_seal(seal_path)


def remove_created_qualification_seal(
    path: Path, expected_identity: tuple[int, ...]
) -> None:
    """Remove only the exact qualification seal created by this rollout."""

    try:
        current = path.lstat()
    except FileNotFoundError as error:
        raise DeploymentError(
            "created Kagemusha qualification seal disappeared before rollback"
        ) from error
    if metadata_identity(current) != expected_identity:
        fail("created Kagemusha qualification seal changed before rollback")
    path.unlink()
    fsync_directory(path.parent)
    if path.exists() or path.is_symlink():
        fail("created Kagemusha qualification seal survived rollback")


def validate_installed_peer_configs(
    installed_binary: Path,
    bundle: BundlePlan,
    *,
    runner: Callable[..., Any] = subprocess.run,
) -> None:
    """Have the exact installed binary validate every config and its inputs."""

    if len(bundle.peers) != PEER_COUNT:
        fail("binary config validation requires exactly four peer configs")
    for peer in bundle.peers:
        try:
            result = runner(
                [
                    str(installed_binary),
                    "--sora",
                    "--config",
                    str(peer.config),
                    "--check-config",
                ],
                check=False,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=CONFIG_CHECK_TIMEOUT_SECONDS,
                env={"PATH": "/usr/bin:/bin:/usr/sbin:/sbin"},
            )
        except subprocess.TimeoutExpired as error:
            raise DeploymentError(
                f"installed binary config check timed out: {peer.slug}"
            ) from error
        except OSError as error:
            raise DeploymentError(
                f"installed binary config check could not execute: {peer.slug}"
            ) from error
        if result.returncode != 0:
            fail(
                "installed binary rejected validator config/genesis/catalog "
                f"(peer={peer.slug}, status={result.returncode})"
            )


def http_json(url: str, timeout: float = 2.0) -> dict[str, Any]:
    """Fetch one bounded JSON response without retaining error bodies."""

    request = urllib.request.Request(
        url,
        method="GET",
        headers={"Accept": "application/json", "User-Agent": "taira-v21-reset/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                fail(f"health endpoint returned HTTP {response.status}: {url}")
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError, TimeoutError) as error:
        raise DeploymentError(f"health endpoint is unavailable: {url}") from error
    if len(body) > MAX_HTTP_BYTES:
        fail(f"health endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")
    return parse_json_bytes(body, f"health response from {url}")


def http_ok(url: str, timeout: float = 2.0) -> None:
    """Require one bounded HTTP 200 response without parsing or retaining its body."""

    request = urllib.request.Request(
        url,
        method="GET",
        headers={"Accept": "*/*", "User-Agent": "taira-v21-reset/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            if response.status != 200:
                fail(f"health endpoint returned HTTP {response.status}: {url}")
            body = response.read(MAX_HTTP_BYTES + 1)
    except (OSError, urllib.error.URLError, TimeoutError) as error:
        raise DeploymentError(f"health endpoint is unavailable: {url}") from error
    if len(body) > MAX_HTTP_BYTES:
        fail(f"health endpoint response exceeds {MAX_HTTP_BYTES} bytes: {url}")


def require_uint(value: object, label: str, *, positive: bool = False) -> int:
    """Require one non-boolean unsigned JSON integer."""

    if not isinstance(value, int) or isinstance(value, bool) or value < int(positive):
        fail(f"{label} is not a valid unsigned integer")
    return value


def normalized_block_hash(value: object, label: str) -> str:
    """Normalize a canonical Iroha block hash to lowercase hexadecimal."""

    if not isinstance(value, str):
        fail(f"{label} is not a block hash")
    match = BLOCK_HASH_RE.fullmatch(value)
    if match is None:
        fail(f"{label} is not a canonical block hash")
    return match.group(1).lower()


def nested(payload: dict[str, Any], *keys: str) -> object:
    """Return a nested mapping value, or ``None`` on a missing object."""

    current: object = payload
    for key in keys:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def tagged_unit(value: object, key: str, label: str, allowed: set[str]) -> str:
    """Decode one canonical tagged-unit status value."""

    if (
        not isinstance(value, dict)
        or set(value) != {key, "details"}
        or not isinstance(value.get(key), str)
        or value.get(key) not in allowed
        or value.get("details") is not None
    ):
        fail(f"{label} is not a canonical tagged unit")
    tag = value[key]
    assert isinstance(tag, str)
    return tag


def published_source_commit(status: dict[str, Any]) -> str:
    """Read the exact full build commit from public node status."""

    build = status.get("build")
    if not isinstance(build, dict):
        fail("/status omitted its build identity")
    for key in ("git_commit_sha", "git_sha", "commit_sha", "commit"):
        value = build.get(key)
        if isinstance(value, str) and COMMIT_RE.fullmatch(value.lower()):
            return value.lower()
    fail("/status omitted one full build Git commit")


@dataclasses.dataclass(frozen=True)
class PeerSample:
    """Coherent committed/offline identity observed from one validator."""

    label: str
    height: int
    block_hash: str
    context: str
    node: str
    build: str
    config: str
    offline_release: str


@dataclasses.dataclass(frozen=True)
class FleetSample:
    """One exact four-validator common-commit sample."""

    height: int
    block_hash: str
    context: str
    build: str
    config: str
    offline_release: str
    nodes: tuple[str, ...]


HttpGetter = Callable[[str, float], dict[str, Any]]
HealthGetter = Callable[[str, float], None]
TerminalChecker = Callable[[], None]


def no_terminal_check() -> None:
    """Default no-op for focused read-path tests without a runtime layout."""


def supervisor_terminal_binding(
    binary_sha256: str,
    binary_info: os.stat_result,
    config_sha256: str,
    restart_generation: str,
) -> str:
    """Reproduce the supervisor's redaction-safe runtime binding."""

    payload = {
        "binary_sha256": binary_sha256,
        "binary_stat_seal": [
            binary_info.st_dev,
            binary_info.st_ino,
            binary_info.st_size,
            binary_info.st_mtime_ns,
            binary_info.st_ctime_ns,
        ],
        "config_sha256": config_sha256,
        "restart_generation": restart_generation,
        "schema": TERMINAL_UNHEALTHY_SCHEMA,
    }
    return hashlib.sha256(
        json.dumps(
            payload,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    ).hexdigest()


def terminal_unhealthy_path(runtime_root: Path, peer: PeerPlan, binding: str) -> Path:
    """Return the identity-scoped private marker for one peer supervisor."""

    return (
        runtime_root
        / "terminal"
        / f"validator-{peer.number}-{binding}-terminal-unhealthy.json"
    )


def require_terminal_marker(
    path: Path,
    peer: PeerPlan,
    owner_uid: int,
    owner_gid: int,
    expected_binding: str,
) -> None:
    """Authenticate one marker and raise a redaction-safe terminal error."""

    try:
        before = require_acl_free_path(path, "terminal-unhealthy marker")
    except FileNotFoundError:
        return
    except (DeploymentError, OSError) as error:
        raise DeploymentError(
            f"{peer.label} terminal-unhealthy marker is unsafe"
        ) from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_uid != owner_uid
        or before.st_gid != owner_gid
        or before.st_nlink != 1
        or stat.S_IMODE(before.st_mode) != 0o600
        or not 0 < before.st_size <= MAX_TERMINAL_UNHEALTHY_BYTES
    ):
        fail(f"{peer.label} terminal-unhealthy marker is unsafe")
    try:
        descriptor = os.open(
            path,
            os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_CLOEXEC", 0),
        )
    except OSError as error:
        raise DeploymentError(
            f"{peer.label} terminal-unhealthy marker is unsafe"
        ) from error
    try:
        body = bytearray()
        while len(body) <= MAX_TERMINAL_UNHEALTHY_BYTES:
            chunk = os.read(
                descriptor,
                min(
                    256,
                    MAX_TERMINAL_UNHEALTHY_BYTES + 1 - len(body),
                ),
            )
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if (
        metadata_identity(before) != metadata_identity(after)
        or len(body) > MAX_TERMINAL_UNHEALTHY_BYTES
    ):
        fail(f"{peer.label} terminal-unhealthy marker is unsafe")
    try:
        payload = json.loads(body)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise DeploymentError(
            f"{peer.label} terminal-unhealthy marker is unsafe"
        ) from error
    if (
        not isinstance(payload, dict)
        or set(payload)
        != {
            "binding_sha256",
            "fatal_fingerprint_sha256",
            "hit_count",
            "schema",
        }
        or payload.get("schema") != TERMINAL_UNHEALTHY_SCHEMA
        or payload.get("hit_count") != 3
        or not isinstance(payload.get("binding_sha256"), str)
        or SHA256_RE.fullmatch(payload["binding_sha256"]) is None
        or not isinstance(payload.get("fatal_fingerprint_sha256"), str)
        or SHA256_RE.fullmatch(payload["fatal_fingerprint_sha256"]) is None
        or payload.get("binding_sha256") != expected_binding
        or (
            json.dumps(
                payload,
                ensure_ascii=True,
                sort_keys=True,
                separators=(",", ":"),
            )
            + "\n"
        ).encode("ascii")
        != body
    ):
        fail(f"{peer.label} terminal-unhealthy marker is unsafe")
    fail(f"{peer.label} entered terminal-unhealthy state")


def require_no_terminal_unhealthy(
    bundle: BundlePlan,
    runtime_root: Path,
    bindings: dict[str, str],
) -> None:
    """Fail fast when any supervisor has durably stopped respawning."""

    for peer in bundle.peers:
        binding = bindings.get(peer.label)
        if binding is None or SHA256_RE.fullmatch(binding) is None:
            fail("terminal-unhealthy binding map is incomplete")
        require_terminal_marker(
            terminal_unhealthy_path(runtime_root, peer, binding),
            peer,
            bundle.owner_uid,
            bundle.owner_gid,
            binding,
        )


def validate_peer_health(
    peer: PeerPlan,
    bundle: BundlePlan,
    expected_source_commit: str,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> PeerSample:
    """Validate readiness, durable consensus, and offline identity for one peer."""

    root = f"http://127.0.0.1:{peer.torii_port}"
    health_getter(f"{root}/health", 2.0)
    ready = getter(f"{root}/readyz", 2.0)
    if (
        ready.get("live") is not True
        or ready.get("mandatory") is not True
        or ready.get("ready") is not True
        or ready.get("cash_handoff_capability") != OFFLINE_CAPABILITY
        or ready.get("required_bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or ready.get("blockers") != []
    ):
        fail(f"{peer.label} /readyz is not mandatory-offline ready")

    status = getter(f"{root}/status", 2.0)
    blocks = require_uint(
        status.get("blocks"), f"{peer.label} /status.blocks", positive=True
    )
    if published_source_commit(status) != expected_source_commit:
        fail(f"{peer.label} publishes the wrong build source commit")

    sumeragi = getter(f"{root}/v1/sumeragi/status", 2.0)
    if (
        sumeragi.get("protocol_version") != 3
        or sumeragi.get("restart_required") is not False
    ):
        fail(f"{peer.label} is not running one restart-clean Sumeragi v2 reducer")
    reducer_height = require_uint(
        sumeragi.get("height"), f"{peer.label} reducer height", positive=True
    )
    committed = require_uint(
        sumeragi.get("last_committed_height"),
        f"{peer.label} last_committed_height",
        positive=True,
    )
    if committed != blocks:
        fail(f"{peer.label} /status.blocks differs from durable committed height")
    if committed > reducer_height:
        fail(f"{peer.label} committed height is ahead of its reducer height")
    context_record = sumeragi.get("height_context")
    if not isinstance(context_record, dict):
        fail(f"{peer.label} omitted its frozen height context")
    validator_count = require_uint(
        context_record.get("validator_count"),
        f"{peer.label} frozen validator count",
        positive=True,
    )
    quorum = context_record.get("quorum")
    if not isinstance(quorum, dict):
        fail(f"{peer.label} omitted its frozen quorum")
    context_min_signers = require_uint(
        quorum.get("min_signers"), f"{peer.label} frozen minimum signers"
    )
    context_total_power = require_uint(
        quorum.get("total_power"), f"{peer.label} frozen total power", positive=True
    )
    mode = tagged_unit(
        context_record.get("mode"),
        "mode",
        f"{peer.label} consensus mode",
        {"permissioned", "npos"},
    )
    if (
        validator_count != PEER_COUNT
        or context_min_signers != 3
        or context_total_power < PEER_COUNT
        or (mode == "permissioned" and context_total_power != PEER_COUNT)
    ):
        fail(f"{peer.label} frozen context is not the exact four-validator quorum")
    subject = sumeragi.get("last_committed_subject")
    if not isinstance(subject, dict):
        fail(f"{peer.label} omitted the durable committed subject")
    block_hash = normalized_block_hash(
        subject.get("block_hash"), f"{peer.label} committed block"
    )
    qc_height = require_uint(
        nested(sumeragi, "last_commit_qc", "certificate", "round", "height"),
        f"{peer.label} CommitQC height",
        positive=True,
    )
    if qc_height != committed:
        fail(f"{peer.label} CommitQC height differs from committed height")
    require_uint(
        nested(sumeragi, "last_commit_qc", "certificate", "round", "view"),
        f"{peer.label} CommitQC view",
    )
    tagged_unit(
        nested(sumeragi, "last_commit_qc", "certificate", "phase"),
        "phase",
        f"{peer.label} CommitQC phase",
        {"commit"},
    )
    qc_subject = nested(sumeragi, "last_commit_qc", "certificate", "subject")
    if qc_subject != subject:
        fail(f"{peer.label} CommitQC subject differs from committed subject")
    commit_record = sumeragi.get("last_commit_qc")
    assert isinstance(commit_record, dict)
    commit_validators = require_uint(
        commit_record.get("validator_count"),
        f"{peer.label} CommitQC validator count",
        positive=True,
    )
    commit_signers = require_uint(
        commit_record.get("signer_count"), f"{peer.label} CommitQC signer count"
    )
    commit_min_signers = require_uint(
        commit_record.get("min_signers"), f"{peer.label} CommitQC minimum signers"
    )
    commit_signed_power = require_uint(
        commit_record.get("signed_power"), f"{peer.label} CommitQC signed power"
    )
    commit_total_power = require_uint(
        commit_record.get("total_power"),
        f"{peer.label} CommitQC total power",
        positive=True,
    )
    if (
        commit_validators != PEER_COUNT
        or commit_min_signers != 3
        or not 3 <= commit_signers <= PEER_COUNT
        or commit_total_power != context_total_power
        or commit_signed_power > commit_total_power
        or commit_signed_power * 3 <= commit_total_power * 2
        or (mode == "permissioned" and commit_signed_power != commit_signers)
    ):
        fail(f"{peer.label} durable CommitQC lacks the exact four-validator quorum")
    context = sumeragi.get("height_context_id")
    node_fingerprint = sumeragi.get("node_fingerprint")
    build_fingerprint = sumeragi.get("build_fingerprint")
    config_fingerprint = sumeragi.get("config_fingerprint")
    if any(
        value in (None, "", {})
        for value in (context, node_fingerprint, build_fingerprint, config_fingerprint)
    ):
        fail(f"{peer.label} omitted a required reducer fingerprint")

    query = urllib.parse.urlencode({"asset_definition_id": OFFLINE_ASSET_ID})
    offline = getter(f"{root}/v1/offline/readiness?{query}", 2.0)
    offline_height = require_uint(
        offline.get("evaluated_block_height"),
        f"{peer.label} offline evaluated height",
        positive=True,
    )
    offline_hash = normalized_block_hash(
        offline.get("evaluated_block_hash"), f"{peer.label} offline evaluated block"
    )
    if (
        offline.get("ready") is not True
        or offline.get("blockers") != []
        or offline.get("cash_handoff_capability") != OFFLINE_CAPABILITY
        or offline.get("required_bridge_abi_version") != OFFLINE_BRIDGE_ABI
        or offline.get("asset_definition_id") != OFFLINE_ASSET_ID
        or offline.get("asset_scale") != OFFLINE_ASSET_SCALE
        or offline_height != committed
        or offline_hash != block_hash
    ):
        fail(f"{peer.label} offline readiness is not bound to its committed block")
    artifact = offline.get("artifact_set")
    if (
        not isinstance(artifact, dict)
        or artifact.get("generation") != bundle.release.generation
        or artifact.get("manifest_sha256") != bundle.release.manifest_sha256
        or artifact.get("release_policy_sha256") != bundle.release.release_policy_sha256
        or artifact.get("release_attestation_sha256")
        != bundle.release.release_attestation_sha256
        or artifact.get("activation_height") != bundle.release.activation_height
        or artifact.get("withdrawal_height") != bundle.release.withdrawal_height
        or artifact.get("max_proof_bytes") != bundle.release.max_proof_bytes
        or artifact.get("asset_scale") != bundle.release.asset_scale
    ):
        fail(f"{peer.label} offline release differs from the reset bundle")

    canonical = lambda value: json.dumps(
        value, ensure_ascii=True, sort_keys=True, separators=(",", ":")
    )
    return PeerSample(
        label=peer.label,
        height=committed,
        block_hash=block_hash,
        context=canonical(context),
        node=canonical(node_fingerprint),
        build=canonical(build_fingerprint),
        config=canonical(config_fingerprint),
        offline_release=canonical(artifact),
    )


def capture_fleet(
    bundle: BundlePlan,
    expected_source_commit: str,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
) -> FleetSample:
    """Require all four direct validators to expose one exact common commit."""

    samples = [
        validate_peer_health(
            peer,
            bundle,
            expected_source_commit,
            getter=getter,
            health_getter=health_getter,
        )
        for peer in bundle.peers
    ]
    baseline = samples[0]
    for sample in samples[1:]:
        for field in (
            "height",
            "block_hash",
            "context",
            "build",
            "config",
            "offline_release",
        ):
            if getattr(sample, field) != getattr(baseline, field):
                fail(f"four-validator fleet disagrees on {field}")
    nodes = tuple(sorted(sample.node for sample in samples))
    if len(set(nodes)) != PEER_COUNT:
        fail("four validator roots do not expose four distinct node identities")
    return FleetSample(
        height=baseline.height,
        block_hash=baseline.block_hash,
        context=baseline.context,
        build=baseline.build,
        config=baseline.config,
        offline_release=baseline.offline_release,
        nodes=nodes,
    )


def wait_for_fleet_sample(
    bundle: BundlePlan,
    expected_source_commit: str,
    deadline: float,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    terminal_checker: TerminalChecker = no_terminal_check,
) -> FleetSample:
    """Retry startup/alignment failures until one coherent sample is available."""

    last_error: Optional[Exception] = None
    while time.monotonic() < deadline:
        terminal_checker()
        try:
            sample = capture_fleet(
                bundle,
                expected_source_commit,
                getter=getter,
                health_getter=health_getter,
            )
        except (DeploymentError, OSError) as error:
            last_error = error
            time.sleep(1)
            continue
        terminal_checker()
        return sample
    raise DeploymentError(f"four-validator readiness did not converge: {last_error}")


def wait_for_advancement(
    bundle: BundlePlan,
    expected_source_commit: str,
    previous: FleetSample,
    deadline: float,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    terminal_checker: TerminalChecker = no_terminal_check,
) -> FleetSample:
    """Require a later common height with a different common block hash."""

    last_error: Optional[Exception] = None
    while time.monotonic() < deadline:
        terminal_checker()
        try:
            current = capture_fleet(
                bundle,
                expected_source_commit,
                getter=getter,
                health_getter=health_getter,
            )
            if (
                current.height > previous.height
                and current.block_hash != previous.block_hash
                and current.build == previous.build
                and current.config == previous.config
                and current.offline_release == previous.offline_release
                and current.nodes == previous.nodes
            ):
                advanced = True
            else:
                advanced = False
                last_error = DeploymentError(
                    "fleet has not advanced one stable common release"
                )
        except (DeploymentError, OSError) as error:
            last_error = error
            advanced = False
        if advanced:
            terminal_checker()
            return current
        time.sleep(1)
    raise DeploymentError(f"four-validator consensus did not advance: {last_error}")


def parse_pid_file(path: Path, uid: int, gid: int) -> int:
    """Read one private managed-child PID file through a no-follow descriptor."""

    body, info = read_regular(path, 64)
    if info.st_uid != uid or info.st_gid != gid or stat.S_IMODE(info.st_mode) & 0o077:
        fail(f"managed PID file has an unsafe owner or mode: {path}")
    try:
        text = body.decode("ascii")
    except UnicodeDecodeError as error:
        raise DeploymentError(f"managed PID file is not ASCII: {path}") from error
    if re.fullmatch(r"[1-9][0-9]*\n", text) is None or int(text) <= 1:
        fail(f"managed PID file is invalid: {path}")
    return int(text)


def expected_supervisor_argv(plist_body: bytes) -> tuple[str, ...]:
    """Return the exact generated supervisor argv from a plist."""

    try:
        payload = plistlib.loads(plist_body)
    except Exception as error:
        raise DeploymentError("generated LaunchDaemon plist is invalid") from error
    arguments = payload.get("ProgramArguments") if isinstance(payload, dict) else None
    if not isinstance(arguments, list) or not all(
        isinstance(value, str) for value in arguments
    ):
        fail("generated LaunchDaemon plist lacks exact ProgramArguments")
    return tuple(arguments)


def verify_managed_peer(
    peer: PeerPlan,
    bundle: BundlePlan,
    runtime_root: Path,
    plist_body: bytes,
    installed_binary: Path,
    ops: SystemOps,
) -> tuple[int, int]:
    """Require one launchd supervisor and its exact single validator child."""

    supervisor_pid = launchd_pid(ops.launchd_print(peer.label), peer.label)
    supervisor = ops.inspect_process(supervisor_pid)
    if supervisor.ppid != 1 or supervisor.uid != bundle.owner_uid:
        fail(f"{peer.label} supervisor has an unexpected owner")
    if supervisor.argv != expected_supervisor_argv(plist_body):
        fail(f"{peer.label} supervisor command differs from the generated plist")
    pid_file = runtime_root / "pids" / f"validator-{peer.number}.pid"
    child_pid = parse_pid_file(pid_file, bundle.owner_uid, bundle.owner_gid)
    child = ops.inspect_process(child_pid)
    expected_child = (str(installed_binary), "--sora", "--config", str(peer.config))
    if (
        child.ppid != supervisor_pid
        or child.uid != bundle.owner_uid
        or child.argv != expected_child
    ):
        fail(f"{peer.label} PID file does not name its exact managed validator child")
    return supervisor_pid, child_pid


def restart_proof(
    bundle: BundlePlan,
    expected_source_commit: str,
    runtime_root: Path,
    plist_bodies: dict[str, bytes],
    installed_binary: Path,
    baseline: FleetSample,
    ops: SystemOps,
    *,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    terminal_checker: TerminalChecker = no_terminal_check,
) -> FleetSample:
    """Terminate one exact child and prove O(1)-sealed independent recovery."""

    terminal_checker()
    peer = bundle.peers[0]
    supervisor_pid, child_pid = verify_managed_peer(
        peer,
        bundle,
        runtime_root,
        plist_bodies[peer.label],
        installed_binary,
        ops,
    )
    ops.terminate(child_pid)
    deadline = time.monotonic() + RESTART_PROOF_TIMEOUT_SECONDS
    new_child: Optional[int] = None
    while time.monotonic() < deadline:
        terminal_checker()
        try:
            current_supervisor, candidate = verify_managed_peer(
                peer,
                bundle,
                runtime_root,
                plist_bodies[peer.label],
                installed_binary,
                ops,
            )
            if current_supervisor != supervisor_pid:
                fail("restart proof replaced the launchd-owned supervisor")
            if candidate != child_pid:
                new_child = candidate
                break
        except (DeploymentError, OSError):
            pass
        time.sleep(0.25)
    if new_child is None:
        fail("managed validator child did not restart within 45 seconds")
    if ops.process_exists(child_pid):
        fail("old managed validator child remained alive after restart")
    terminal_checker()
    return wait_for_advancement(
        bundle,
        expected_source_commit,
        baseline,
        deadline,
        getter=getter,
        health_getter=health_getter,
        terminal_checker=terminal_checker,
    )


def rollback_cohort(snapshots: Sequence[PlistSnapshot], ops: SystemOps) -> None:
    """Unload any new jobs, restore every old plist, and reload all four old jobs."""

    errors: list[str] = []
    for snapshot in snapshots:
        if ops.launchd_print(snapshot.path.stem) is None:
            continue
        try:
            ops.bootout(snapshot.path.stem)
        except (DeploymentError, OSError):
            errors.append(f"bootout:{snapshot.path.stem}")
    for snapshot in snapshots:
        try:
            atomic_replace_owned(
                snapshot.path,
                snapshot.body,
                mode=snapshot.mode,
                uid=snapshot.uid,
                gid=snapshot.gid,
            )
        except (DeploymentError, OSError):
            errors.append(f"restore:{snapshot.path.stem}")
    for snapshot in snapshots:
        try:
            ops.bootstrap(snapshot.path)
        except (DeploymentError, OSError):
            errors.append(f"bootstrap:{snapshot.path.stem}")
    deadline = time.monotonic() + 30
    pending = {snapshot.path.stem: snapshot for snapshot in snapshots}
    while pending and time.monotonic() < deadline:
        for label, snapshot in tuple(pending.items()):
            try:
                verify_restored_snapshot(snapshot, ops)
            except (DeploymentError, OSError):
                continue
            del pending[label]
        if pending:
            time.sleep(0.25)
    errors.extend(f"verify:{label}" for label in sorted(pending))
    if errors:
        fail("four-job rollback was incomplete: " + ", ".join(errors))


def install_runtime_layout(bundle: BundlePlan) -> Path:
    """Create the root-controlled parent and owner-private reset runtime leaf."""

    ensure_root_directory(INSTALL_ROOT, 0o755)
    runtime_parent = INSTALL_ROOT / "runtime"
    ensure_root_directory(runtime_parent, 0o755)
    runtime_root = runtime_parent / bundle.manifest_sha256
    ensure_runtime_directory(runtime_root, bundle.owner_uid, bundle.owner_gid)
    ensure_runtime_directory(runtime_root / "pids", bundle.owner_uid, bundle.owner_gid)
    ensure_runtime_directory(runtime_root / "logs", bundle.owner_uid, bundle.owner_gid)
    ensure_runtime_directory(
        runtime_root / "terminal", bundle.owner_uid, bundle.owner_gid
    )
    return runtime_root


def apply_reset(
    args: argparse.Namespace,
    bundle: BundlePlan,
    sources: SourcePlan,
    old_cohort: Sequence[PlistSnapshot],
    *,
    rollout_starter: Callable[[], None],
    ops: Optional[SystemOps] = None,
    getter: HttpGetter = http_json,
    health_getter: HealthGetter = http_ok,
    config_checker: Callable[
        [Path, BundlePlan], None
    ] = validate_installed_peer_configs,
    cutover_identity_checker: Callable[
        [BundlePlan], None
    ] = require_post_qualification_cutover_identity,
    seal_preparer: Callable[[BundlePlan], Path] = prepare_qualification_seal_path,
    seal_writer: Callable[
        [Path, BundlePlan, Path], tuple[int, ...]
    ] = write_catalog_qualification_seal,
    seal_authenticator: Callable[
        [Path], tuple[int, ...]
    ] = authenticate_qualification_seal,
    seal_remover: Callable[
        [Path, tuple[int, ...]], None
    ] = remove_created_qualification_seal,
) -> dict[str, Any]:
    """Validate and install one fresh reset, rolling back every moved component."""

    if os.geteuid() != 0:
        fail("--apply requires root")
    if len(old_cohort) != PEER_COUNT:
        fail("--apply requires exactly four authenticated rollback plists")
    ops = ops or SystemOps()

    ensure_root_directory(INSTALL_ROOT, 0o755)
    binary_store = INSTALL_ROOT / "binaries"
    supervisor_store = INSTALL_ROOT / "supervisors"
    ensure_root_directory(binary_store, 0o755)
    ensure_root_directory(supervisor_store, 0o755)
    binary_dir = binary_store / sources.binary_sha256
    supervisor_dir = supervisor_store / sources.supervisor_sha256
    installed_binary = binary_dir / "irohad"
    installed_supervisor = supervisor_dir / "taira_peer_supervisor.py"
    if binary_dir.exists() and not (
        installed_binary.exists() or installed_binary.is_symlink()
    ):
        fail("content-addressed binary directory is incomplete")
    if supervisor_dir.exists() and not (
        installed_supervisor.exists() or installed_supervisor.is_symlink()
    ):
        fail("content-addressed supervisor directory is incomplete")
    if not binary_dir.exists():
        ensure_root_directory(binary_dir, 0o755)
    if not supervisor_dir.exists():
        ensure_root_directory(supervisor_dir, 0o755)
    binary_info = install_immutable(
        sources.binary, installed_binary, sources.binary_sha256
    )
    install_immutable(
        sources.supervisor, installed_supervisor, sources.supervisor_sha256
    )
    require_exact_names(binary_dir, {"irohad"}, "content-addressed binary directory")
    require_exact_names(
        supervisor_dir,
        {"taira_peer_supervisor.py"},
        "content-addressed supervisor directory",
    )
    os.chmod(binary_dir, 0o555)
    os.chmod(supervisor_dir, 0o555)
    fsync_directory(binary_dir)
    fsync_directory(supervisor_dir)
    fsync_directory(binary_store)
    fsync_directory(supervisor_store)
    require_root_controlled_file(installed_binary, executable=True)
    require_root_controlled_file(installed_supervisor, executable=True)
    runtime_root = install_runtime_layout(bundle)
    require_filesystem_headroom(
        [
            bundle.root,
            bundle.release.installed_root,
            runtime_root,
            *(peer.storage for peer in bundle.peers),
        ],
        args.minimum_free_bytes,
    )

    plist_bodies = {
        peer.label: render_plist(
            peer,
            bundle,
            sources,
            installed_binary=installed_binary,
            binary_info=binary_info,
            installed_supervisor=installed_supervisor,
            runtime_root=runtime_root,
            restart_generation=args.restart_generation,
        )
        for peer in bundle.peers
    }
    terminal_bindings = {
        peer.label: supervisor_terminal_binding(
            sources.binary_sha256,
            binary_info,
            peer.config_sha256,
            args.restart_generation,
        )
        for peer in bundle.peers
    }
    for label, body in plist_bodies.items():
        payload = plistlib.loads(body)
        if payload.get("Label") != label:
            fail(f"generated LaunchDaemon label mismatch: {label}")

    cohort_mutated = False
    release_moved = False
    seal_creation_attempted = False
    created_seal_identity: Optional[tuple[int, ...]] = None
    seal_path = qualification_seal_path(bundle.release.tree_sha256)
    guarded_signals = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
    previous_handlers = {signum: signal.getsignal(signum) for signum in guarded_signals}

    def request_rollback(signum: int, _frame: object) -> NoReturn:
        raise DeploymentError(
            f"controller received signal {signum}; rolling back the four-job cohort"
        )

    for signum in guarded_signals:
        signal.signal(signum, request_rollback)
    try:
        # Recheck all four old jobs immediately before the first cohort change.
        require_bundle_runtime_unchanged(bundle)
        move_release_to_root_store(bundle)
        release_moved = True
        # The exact candidate performs the expensive catalog qualification
        # once; all four ordinary checks below must consume its immutable seal.
        seal_path = seal_preparer(bundle)
        seal_creation_attempted = True
        created_seal_identity = seal_writer(
            installed_binary,
            bundle,
            seal_path,
        )
        config_checker(installed_binary, bundle)
        if seal_authenticator(seal_path) != created_seal_identity:
            fail("Kagemusha qualification seal changed during config checks")
        for snapshot in old_cohort:
            current_body, current_info = read_regular(snapshot.path, MAX_MANIFEST_BYTES)
            if (
                current_body != snapshot.body
                or stat.S_IMODE(current_info.st_mode) != snapshot.mode
                or current_info.st_uid != snapshot.uid
                or current_info.st_gid != snapshot.gid
            ):
                fail(
                    f"old LaunchDaemon changed after dry-run capture: {snapshot.path.name}"
                )
            verify_restored_snapshot(snapshot, ops)
        cutover_identity_checker(bundle)
        # Close the asynchronous-signal window between durable replay
        # consumption and the first external cohort mutation.  Once the first
        # bootout is attempted, its side effects cannot be proven absent, so
        # rollback and replay protection must both remain committed.
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, guarded_signals)
        try:
            rollout_starter()
            cohort_mutated = True
            ops.bootout(old_cohort[0].path.stem)
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
        # Stop the rest of the old cohort before publishing any new job.
        for snapshot in old_cohort[1:]:
            ops.bootout(snapshot.path.stem)
        for snapshot in old_cohort:
            if ops.launchd_print(snapshot.path.stem) is not None:
                fail(f"old LaunchDaemon remained loaded: {snapshot.path.stem}")

        # Publish all four fresh plists before bootstrapping the first new job.
        for peer in bundle.peers:
            atomic_replace_owned(
                LAUNCH_DAEMONS / f"{peer.label}.plist",
                plist_bodies[peer.label],
                mode=0o644,
                uid=0,
                gid=0,
            )
        for peer in bundle.peers:
            ops.bootstrap(LAUNCH_DAEMONS / f"{peer.label}.plist")

        terminal_checker = lambda: require_no_terminal_unhealthy(
            bundle, runtime_root, terminal_bindings
        )
        health_deadline = time.monotonic() + args.health_timeout_seconds
        baseline = wait_for_fleet_sample(
            bundle,
            args.expected_source_commit,
            health_deadline,
            getter=getter,
            health_getter=health_getter,
            terminal_checker=terminal_checker,
        )
        advanced = wait_for_advancement(
            bundle,
            args.expected_source_commit,
            baseline,
            health_deadline,
            getter=getter,
            health_getter=health_getter,
            terminal_checker=terminal_checker,
        )
        terminal_checker()
        for peer in bundle.peers:
            verify_managed_peer(
                peer,
                bundle,
                runtime_root,
                plist_bodies[peer.label],
                installed_binary,
                ops,
            )
        terminal_checker()
        restarted = restart_proof(
            bundle,
            args.expected_source_commit,
            runtime_root,
            plist_bodies,
            installed_binary,
            advanced,
            ops,
            getter=getter,
            health_getter=health_getter,
            terminal_checker=terminal_checker,
        )
    except BaseException as rollout_error:
        # A second termination request must not interrupt the rollback itself.
        for signum in guarded_signals:
            signal.signal(signum, signal.SIG_IGN)
        if (
            not release_moved
            and bundle.release.installed_root.exists()
            and not bundle.release.source_root.exists()
        ):
            # Close the signal-delivery window between a successful rename and
            # storing the local ``release_moved`` flag.
            release_moved = True
        rollback_error: Optional[BaseException] = None
        unattributed_seal = False
        if seal_creation_attempted and created_seal_identity is None:
            try:
                seal_path.lstat()
            except FileNotFoundError:
                pass
            except OSError as error:
                unattributed_seal = True
                rollback_error = DeploymentError(
                    "qualification-seal absence could not be proven after the "
                    "writer failed; preserving the installed release because "
                    "rollback is incomplete"
                )
                if hasattr(rollback_error, "add_note"):
                    rollback_error.add_note(
                        f"seal inspection failure: {type(error).__name__}: {error}"
                    )
            else:
                unattributed_seal = True
                rollback_error = DeploymentError(
                    "qualification writer returned no seal identity after a seal "
                    "appeared; preserving the unattributed seal and installed "
                    "release because rollback is incomplete"
                )
        if cohort_mutated:
            try:
                rollback_cohort(old_cohort, ops)
            except BaseException as error:
                rollback_error = error
        if rollback_error is None and created_seal_identity is not None:
            try:
                seal_remover(seal_path, created_seal_identity)
            except BaseException as error:
                rollback_error = error
        if rollback_error is None and release_moved:
            try:
                restore_release_to_bundle(bundle)
            except BaseException as error:
                rollback_error = error
        if rollback_error is not None:
            if unattributed_seal:
                combined = DeploymentError(
                    "Taira reset failed; rollback is incomplete because an "
                    "unattributed qualification seal must be preserved with its "
                    "installed release"
                )
            else:
                combined = DeploymentError(
                    "Taira reset failed and its exact cohort/release/seal rollback "
                    "did not complete"
                )
            if hasattr(combined, "add_note"):
                combined.add_note(
                    f"rollout failure: {type(rollout_error).__name__}: {rollout_error}"
                )
            raise combined from rollback_error
        raise
    finally:
        for signum, handler in previous_handlers.items():
            signal.signal(signum, handler)

    return {
        "applied": True,
        "absent_old_children": sorted(
            snapshot.path.stem
            for snapshot in old_cohort
            if not snapshot.managed.child_was_present
        ),
        "binary": str(installed_binary),
        "binary_sha256": sources.binary_sha256,
        "bundle": str(bundle.root),
        "end_block_hash": restarted.block_hash,
        "end_height": restarted.height,
        "mandatory_offline": True,
        "peer_count": PEER_COUNT,
        "qualification_seal": str(seal_path),
        "release": str(bundle.release.installed_root),
        "release_tree_sha256": bundle.release.tree_sha256,
        "restart_generation": args.restart_generation,
        "restart_proof": "passed",
        "source_commit": args.expected_source_commit,
        "start_height": baseline.height,
        "supervisor": str(installed_supervisor),
        "supervisor_sha256": sources.supervisor_sha256,
    }


def build_parser() -> argparse.ArgumentParser:
    """Build the exact single-command dry-run/apply interface."""

    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("--bundle", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--supervisor", type=Path, required=True)
    parser.add_argument("--admission-archive", type=Path, required=True)
    parser.add_argument("--admission-authority-dir", type=Path, required=True)
    parser.add_argument(
        "--supervisor-python",
        type=Path,
        default=DEFAULT_SUPERVISOR_PYTHON,
    )
    parser.add_argument("--expected-source-commit", required=True)
    parser.add_argument("--expected-cargo-lock-sha256", required=True)
    parser.add_argument(
        "--expected-workspace-source-manifest-sha256", required=True
    )
    parser.add_argument("--expected-receipt-id", required=True)
    parser.add_argument("--trusted-signing-fingerprint", required=True)
    parser.add_argument("--release-manifest-verifier", type=Path, required=True)
    parser.add_argument(
        "--trusted-release-manifest-verifier-sha256", required=True
    )
    parser.add_argument(
        "--health-timeout-seconds",
        type=int,
        default=DEFAULT_HEALTH_TIMEOUT_SECONDS,
    )
    parser.add_argument(
        "--minimum-free-bytes",
        type=int,
        default=DEFAULT_MINIMUM_FREE_BYTES,
    )
    parser.add_argument(
        "--maximum-fsync-latency-ms",
        type=int,
        default=DEFAULT_MAXIMUM_FSYNC_LATENCY_MS,
    )
    parser.add_argument(
        "--allow-absent-old-child",
        action="store_true",
        help=(
            "explicitly authorize capture of an already-degraded old job only "
            "when its exact loaded supervisor has no PID file and no child "
            "process; stale or mismatched children remain fatal"
        ),
    )
    parser.add_argument("--apply", action="store_true")
    return parser


def validate_arguments(args: argparse.Namespace) -> None:
    """Validate scalar inputs before reading deployment paths."""

    args.expected_source_commit = require_commit(args.expected_source_commit)
    args.expected_cargo_lock_sha256 = require_sha256(
        args.expected_cargo_lock_sha256, "expected Cargo.lock SHA-256"
    )
    args.expected_workspace_source_manifest_sha256 = require_sha256(
        args.expected_workspace_source_manifest_sha256,
        "expected workspace source manifest SHA-256",
    )
    args.expected_receipt_id = require_sha256(
        args.expected_receipt_id, "expected receipt ID"
    )
    args.trusted_signing_fingerprint = require_sha256(
        args.trusted_signing_fingerprint, "trusted signing fingerprint"
    )
    args.trusted_release_manifest_verifier_sha256 = require_sha256(
        args.trusted_release_manifest_verifier_sha256,
        "trusted release-manifest verifier SHA-256",
    )
    if args.health_timeout_seconds <= 0:
        fail("--health-timeout-seconds must be positive")
    if args.minimum_free_bytes < DEFAULT_MINIMUM_FREE_BYTES:
        fail(f"--minimum-free-bytes may not be below {DEFAULT_MINIMUM_FREE_BYTES}")
    if not 1 <= args.maximum_fsync_latency_ms <= DEFAULT_MAXIMUM_FSYNC_LATENCY_MS:
        fail("--maximum-fsync-latency-ms must be positive and may not exceed 250")


def execute(
    args: argparse.Namespace, *, ops: Optional[SystemOps] = None
) -> dict[str, Any]:
    """Run the read-only preflight and optional guarded apply transaction."""

    validate_arguments(args)
    if args.apply and os.geteuid() != 0:
        fail("--apply requires root; no changes were made")
    admission = verify_deployment_admission(args)
    bundle = validate_bundle(
        args.bundle,
        expected_reset_manifest_sha256=admission.reset_manifest_sha256,
        expected_binary_sha256=admission.binary_sha256,
        expected_source_commit=admission.source_commit,
        minimum_free_bytes=args.minimum_free_bytes,
        maximum_fsync_latency_ms=args.maximum_fsync_latency_ms,
    )
    sources = validate_sources(args, bundle, admission)
    require_inputs_match_admission(bundle, sources, admission)
    args.restart_generation = admission.restart_generation
    system_ops = ops or SystemOps()
    if not args.apply:
        old_cohort = capture_old_cohort(
            system_ops,
            allow_absent_child=args.allow_absent_old_child,
        )
        require_admission_archive_unchanged(admission)
        return {
            "admission_archive_sha256": admission.archive_sha256,
            "admission_receipt_consumed": False,
            "admission_receipt_id": admission.receipt_id,
            "applied": False,
            "absent_old_children": sorted(
                snapshot.path.stem
                for snapshot in old_cohort
                if not snapshot.managed.child_was_present
            ),
            "binary_sha256": sources.binary_sha256,
            "bundle": str(bundle.root),
            "bundle_bytes": bundle.bundle_bytes,
            "free_bytes": bundle.free_bytes,
            "fsync_latency_ms": round(bundle.fsync_latency_ms, 3),
            "mandatory_offline": True,
            "mode": "verified-read-only-dry-run",
            "peer_count": PEER_COUNT,
            "qualification_seal": str(
                qualification_seal_path(bundle.release.tree_sha256)
            ),
            "release_attestation_sha256": (bundle.release.release_attestation_sha256),
            "release_manifest_sha256": bundle.release.manifest_sha256,
            "release_policy_sha256": bundle.release.release_policy_sha256,
            "release_tree_sha256": bundle.release.tree_sha256,
            "restart_generation": args.restart_generation,
            "source_commit": args.expected_source_commit,
            "supervisor_sha256": sources.supervisor_sha256,
        }
    with exclusive_deployment_lock():
        locked_admission = verify_deployment_admission(args)
        if locked_admission != admission:
            fail("verified admission identity changed before the deployment lock")
        require_admission_bound_inputs_unchanged(bundle, sources, locked_admission)
        old_cohort = capture_old_cohort(
            system_ops,
            allow_absent_child=args.allow_absent_old_child,
        )
        require_admission_bound_inputs_unchanged(bundle, sources, locked_admission)
        with consume_admission_receipt(locked_admission) as consumption:
            report = apply_reset(
                args,
                bundle,
                sources,
                old_cohort,
                rollout_starter=consumption.mark_rollout_started,
                ops=system_ops,
            )
        report.update(
            {
                "admission_archive_sha256": locked_admission.archive_sha256,
                "admission_receipt_consumed": True,
                "admission_receipt_id": locked_admission.receipt_id,
            }
        )
        return report


def main(argv: Optional[list[str]] = None) -> int:
    """Run the controller and emit only one redaction-safe JSON summary."""

    args = build_parser().parse_args(argv)
    try:
        report = execute(args)
    except (DeploymentError, OSError, ValueError) as error:
        print(f"Taira v21 reset refused: {error}", file=sys.stderr)
        return 2
    print(json.dumps(report, ensure_ascii=True, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
