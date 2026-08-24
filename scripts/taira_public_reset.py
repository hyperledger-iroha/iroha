#!/usr/bin/env python3
"""Build a fail-closed operator handoff for a public Taira fresh reset.

This file is deliberately not a privileged deployment controller.  It performs
only local, read-only validation of an owner-private external inventory, its
source/artifact SHA-256 closure, four validator preflight attestations, and one
edge-authority attestation.  ``confirm`` emits a redacted phased handoff only
after an explicit acknowledgement bound to the exact inventory.  ``apply`` always
fails: the repository currently has no compiled, authenticated Linux public
reset executor with a deployment lock, bounded rollback, cohort convergence,
signed canary, and restart-proof authority.

No subcommand opens a network connection, invokes a transport, writes a file,
deletes state, installs an artifact, stops a service, or reloads the edge.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import ipaddress
import json
import os
import re
import stat
import struct
import sys
import time
import zlib
from pathlib import Path, PurePosixPath
from typing import Any, NoReturn

try:
    import taira_constants as _taira_constants
except ModuleNotFoundError:
    from scripts import taira_constants as _taira_constants


CHAIN_ID = _taira_constants.CHAIN_ID
CHAIN_DISCRIMINANT = _taira_constants.CHAIN_DISCRIMINANT
PEER_COUNT = _taira_constants.PEER_COUNT
SLUGS = _taira_constants.SLUGS
INVENTORY_SCHEMA = "iroha.taira.public-reset.inventory.v1"
SOURCE_SCHEMA = "iroha.taira.public-reset.source-closure.v1"
VALIDATOR_ATTESTATION_SCHEMA = "iroha.taira.public-reset.validator-preflight.v1"
EDGE_ATTESTATION_SCHEMA = "iroha.taira.public-reset.edge-preflight.v1"
HANDOFF_SCHEMA = "iroha.taira.public-reset.handoff.v1"
SCHEMA_VERSION = 1
KVM_API_VERSION = 12
MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_ARTIFACT_BYTES = 4 * 1024 * 1024 * 1024
MAX_SOURCE_FILES = 100_000
MAX_GIT_INDEX_BYTES = 64 * 1024 * 1024
MAX_WINDOW_SECONDS = 24 * 60 * 60
SOURCE_DOMAIN = b"iroha.taira.public-reset.source-closure.v1\x00"
HOST_ARTIFACT_DOMAIN = b"iroha.taira.public-reset.host-artifacts.v1\x00"
ARTIFACT_DOMAIN = b"iroha.taira.public-reset.artifact-closure.v1\x00"
CONFIRMATION_PREFIX = "confirm-public-taira-reset"
APPLY_BARRIER = (
    "public reset execution is unavailable: install a compiled authenticated "
    "Linux/AArch64/KVM deployment executor that owns the global deployment "
    "lock, exact-root reset, rollback/replay journal, four-validator "
    "convergence, signed Applied canary, bounded restart proof, and edge "
    "cutover authority"
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
IDENTIFIER_RE = re.compile(r"[a-z0-9](?:[a-z0-9._-]{0,126}[a-z0-9])?")
UNIT_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.@:-]{0,254}\.service")
REMOTE_PATH_RE = re.compile(r"/[A-Za-z0-9._+/-]+")
CANONICAL_DNS_LABEL_RE = re.compile(r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?")
CANONICAL_PORT_RE = re.compile(r"[1-9][0-9]{0,4}")
LOCALHOST_ALIASES = frozenset(
    {"localhost", "localhost.localdomain", "ip6-localhost", "ip6-loopback"}
)
GIT_BRANCH = "optimizations"
PUBLIC_ROOT = "https://taira.sora.org"
VALIDATOR_HOSTS = tuple(f"{slug}.sora.org" for slug in SLUGS)
SYSTEMCTL_PATH = "/usr/bin/systemctl"
# Closed paths introduced by the public-reset V1 executor handoff.  Inventories
# may attest hashes and ownership, but may not select a different filesystem layout.
VALIDATOR_SERVICE_ROOT = "/srv/taira/{validator_id}"
VALIDATOR_STATE_ROOT = "/var/lib/taira/{validator_id}"
VALIDATOR_ARTIFACT_SPECS = {
    "binary": ("iroha3d_taira", "bin/iroha3d_taira", "0755", 512 * 1024 * 1024),
    "config": ("{validator_id}.toml", "config/config.toml", "0640", 1024 * 1024),
    "genesis": ("genesis.json", "genesis/genesis.json", "0644", 64 * 1024 * 1024),
    "genesis_hash": ("genesis.sha256", "genesis/genesis.sha256", "0644", 65),
    "iroha_cli": ("iroha", "bin/iroha", "0755", 512 * 1024 * 1024),
}
REQUIRED_ARTIFACT_ROLES = frozenset(VALIDATOR_ARTIFACT_SPECS)
EDGE_SERVICE = {
    "manager_path": SYSTEMCTL_PATH,
    "unit": "nginx.service",
    "unit_path": "/etc/systemd/system/nginx.service",
    "service_root": "/etc/nginx/conf.d",
    "service_guard_path": "/etc/nginx/conf.d/.taira-edge-root",
    "temporary_root": "/etc/nginx/conf.d/.taira-staging",
    "target_config_path": "/etc/nginx/conf.d/taira.conf",
}
EDGE_NGINX_PATH = "/usr/sbin/nginx"
EDGE_CONFIG_SOURCE_NAME = "taira.sora.org.conf"
EDGE_CONFIG_MODE = "0640"
EDGE_CONFIG_MAX_BYTES = 1024 * 1024
INROU_NAME = "iroha-inrou-0"
INROU_ID = 70_000
INROU_HOME = "/nonexistent"
INROU_SHELLS = frozenset(
    {"/usr/sbin/nologin", "/sbin/nologin", "/usr/bin/false", "/bin/false"}
)
FORBIDDEN_FIELD_FRAGMENTS = (
    "authorization_header",
    "bearer",
    "credential",
    "password",
    "private_key",
    "secret",
    "token",
)


class PublicResetError(RuntimeError):
    """The operator inventory or handoff failed closed."""


def fail(message: str) -> NoReturn:
    raise PublicResetError(message)


def _duplicate_keys(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail(f"JSON contains duplicate field {key!r}")
        result[key] = value
    return result


def _reject_float(value: str) -> NoReturn:
    fail(f"JSON values must not contain floating-point value {value!r}")


def canonical_json_bytes(value: object) -> bytes:
    try:
        return (
            json.dumps(
                value,
                allow_nan=False,
                ensure_ascii=True,
                separators=(",", ":"),
                sort_keys=True,
            )
            + "\n"
        ).encode("ascii")
    except (TypeError, ValueError, UnicodeEncodeError) as error:
        raise PublicResetError("value is not canonically JSON encodable") from error


def _decode_json(payload: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(
            payload,
            object_pairs_hook=_duplicate_keys,
            parse_float=_reject_float,
            parse_constant=_reject_float,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise PublicResetError(f"{label} is not strict UTF-8 JSON") from error
    if not isinstance(value, dict):
        fail(f"{label} root must be an object")
    return value


def _exact(value: object, fields: set[str], label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{label} must be an object")
    expected = frozenset(fields)
    actual = frozenset(value)
    for key in actual:
        lowered = key.lower()
        if any(fragment in lowered for fragment in FORBIDDEN_FIELD_FRAGMENTS):
            fail(f"{label} contains forbidden secret-bearing field {key!r}")
    if actual != expected:
        missing = ", ".join(sorted(expected - actual)) or "none"
        extra = ", ".join(sorted(actual - expected)) or "none"
        fail(f"{label} fields differ (missing: {missing}; extra: {extra})")
    return value


def _text(value: object, label: str, maximum: int = 1024) -> str:
    if (
        not isinstance(value, str)
        or not value
        or len(value) > maximum
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        fail(f"{label} must be one nonempty bounded printable string")
    return value


def _identifier(value: object, label: str) -> str:
    value = _text(value, label, 128)
    if IDENTIFIER_RE.fullmatch(value) is None:
        fail(f"{label} must be one canonical identifier")
    return value


def _sha256(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        fail(f"{label} must be one nonzero lowercase SHA-256 digest")
    return value


def _integer(value: object, label: str, minimum: int, maximum: int) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        fail(f"{label} must be an integer in {minimum}..={maximum}")
    return value


def _local_path(value: object, label: str) -> Path:
    value = _text(value, label, 4096)
    path = Path(value)
    if not path.is_absolute() or str(path) != value or ".." in path.parts:
        fail(f"{label} must be one canonical absolute local path")
    return path


def _private_input_path(path: Path, label: str) -> Path:
    """Require a canonical file path below symlink-free trusted ancestry."""

    if not path.is_absolute() or str(path) != path.as_posix() or ".." in path.parts:
        fail(f"{label} must use one canonical absolute local path")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise PublicResetError(f"{label} is unavailable") from error
    if resolved != path:
        fail(f"{label} must not traverse symlink ancestry")
    for ancestor in path.parents:
        try:
            metadata = ancestor.lstat()
        except OSError as error:
            raise PublicResetError(f"cannot inspect {label} ancestry") from error
        if (
            stat.S_ISLNK(metadata.st_mode)
            or not stat.S_ISDIR(metadata.st_mode)
            or metadata.st_uid not in {0, os.geteuid()}
            or stat.S_IMODE(metadata.st_mode) & 0o022
        ):
            fail(f"{label} ancestry must be root/operator-owned and non-group-writable")
    return path


def _remote_path(value: object, label: str, *, guarded_root: bool = False) -> str:
    value = _text(value, label, 4096)
    if value.startswith("//") or REMOTE_PATH_RE.fullmatch(value) is None:
        fail(f"{label} must be one absolute safe-character POSIX path")
    path = PurePosixPath(value)
    if (
        path == PurePosixPath("/")
        or not path.is_absolute()
        or str(path) != value
        or ".." in path.parts
    ):
        fail(f"{label} must be one canonical absolute POSIX path")
    if guarded_root and len(path.parts) < 4:
        fail(f"{label} is too broad to be a guarded root")
    return value


def _relative_path(value: object, label: str) -> str:
    value = _text(value, label, 4096)
    path = PurePosixPath(value)
    if path.is_absolute() or str(path) != value or ".." in path.parts or value == ".":
        fail(f"{label} must be one canonical relative path")
    return value


def _descendant(path: str, root: str) -> bool:
    candidate = PurePosixPath(path)
    parent = PurePosixPath(root)
    return candidate != parent and parent in candidate.parents


def _direct_child(path: str, root: str, label: str) -> None:
    if PurePosixPath(path).parent != PurePosixPath(root):
        fail(f"{label} must be one direct child of its guarded root")


def _canonical_dns_name(value: str, label: str) -> str:
    """Apply the edge renderer's exact lowercase DNS spelling rules."""

    labels = value.split(".")
    if (
        not value
        or value != value.lower()
        or value.endswith(".")
        or len(value) > 253
        or any(
            len(part) > 63 or CANONICAL_DNS_LABEL_RE.fullmatch(part) is None
            for part in labels
        )
    ):
        fail(f"{label} must use canonical lowercase DNS spelling")
    return value


def _canonical_port(value: str, label: str) -> str:
    if CANONICAL_PORT_RE.fullmatch(value) is None or int(value) > 65_535:
        fail(f"{label} must use a canonical port in 1..65535")
    return value


def _canonical_upstream(value: object, label: str) -> str:
    """Mirror the renderer's host:port rules and additionally reject loopback."""

    value = _text(value, label, 255)
    bracketed = value.startswith("[")
    if bracketed:
        end = value.find("]")
        if end < 1 or end + 2 >= len(value) or value[end + 1] != ":":
            fail(f"{label} must be one canonical [IPv6]:port upstream")
        host = value[1:end]
        port = value[end + 2 :]
        if "]" in port:
            fail(f"{label} must be one canonical [IPv6]:port upstream")
    else:
        if value.count(":") != 1:
            fail(f"{label} must be one canonical host:port upstream")
        host, port = value.split(":", 1)
        if not host or not port:
            fail(f"{label} must be one canonical host:port upstream")
    _canonical_port(port, label)
    if bracketed:
        try:
            address = ipaddress.IPv6Address(host)
        except ipaddress.AddressValueError as error:
            raise PublicResetError(f"{label} contains malformed IPv6") from error
        mapped = address.ipv4_mapped
        if (
            host != address.compressed
            or address.is_unspecified
            or address.is_loopback
            or (mapped is not None and (mapped.is_unspecified or mapped.is_loopback))
        ):
            fail(f"{label} IPv6 must be compressed lowercase non-wildcard non-loopback")
        return value
    try:
        address = ipaddress.IPv4Address(host)
    except ipaddress.AddressValueError:
        address = None
    if address is not None:
        if host != str(address) or address.is_unspecified or address.is_loopback:
            fail(f"{label} IPv4 must be canonical non-wildcard non-loopback")
        return value
    if re.fullmatch(r"[0-9.]+", host):
        fail(f"{label} contains malformed IPv4")
    canonical_host = _canonical_dns_name(host, f"{label} host")
    if canonical_host in LOCALHOST_ALIASES or canonical_host.endswith(".localhost"):
        fail(f"{label} must not use a localhost alias")
    return value


def _read_regular(path: Path, maximum: int, label: str) -> tuple[bytes, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PublicResetError(f"cannot inspect {label}: {path}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum
    ):
        fail(f"{label} must be one bounded direct single-link regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            fail(f"{label} changed while opening")
        body = bytearray()
        while len(body) <= maximum:
            chunk = os.read(descriptor, min(1024 * 1024, maximum + 1 - len(body)))
            if not chunk:
                break
            body.extend(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if len(body) != before.st_size or _identity(after) != _identity(before):
        fail(f"{label} changed while reading")
    return bytes(body), after


def _identity(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_gid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def _hash_regular(path: Path, maximum: int, label: str) -> tuple[str, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise PublicResetError(f"cannot inspect {label}: {path}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum
    ):
        fail(f"{label} must be one bounded direct single-link regular file: {path}")
    descriptor = os.open(
        path,
        os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0),
    )
    digest = hashlib.sha256()
    total = 0
    try:
        opened = os.fstat(descriptor)
        if _identity(opened) != _identity(before):
            fail(f"{label} changed while opening")
        while chunk := os.read(descriptor, 1024 * 1024):
            total += len(chunk)
            if total > maximum:
                fail(f"{label} exceeded its byte bound")
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    if total != before.st_size or _identity(after) != _identity(before):
        fail(f"{label} changed while hashing")
    return digest.hexdigest(), after


def _owner_private(metadata: os.stat_result, label: str) -> None:
    if metadata.st_uid != os.geteuid() or stat.S_IMODE(metadata.st_mode) not in {0o400, 0o600}:
        fail(f"{label} must be effective-user owned and owner-only")


def _git_blob_sha1(body: bytes) -> str:
    return hashlib.sha1(f"blob {len(body)}\0".encode("ascii") + body).hexdigest()


def _git_snapshot(root: Path) -> tuple[str, str, tuple[dict[str, str], ...]]:
    """Read the current optimizations HEAD/tree and complete stage-0 Git index."""

    git_dir = root / ".git"
    try:
        if git_dir.resolve(strict=True) != git_dir or not git_dir.is_dir():
            fail("source .git must be one direct directory")
    except OSError as error:
        raise PublicResetError("source .git is unavailable") from error
    head_body, _ = _read_regular(git_dir / "HEAD", 1024, "Git HEAD")
    expected_head = f"ref: refs/heads/{GIT_BRANCH}\n".encode("ascii")
    if head_body != expected_head:
        fail(f"source checkout must be on the exact {GIT_BRANCH} branch")
    ref_body, _ = _read_regular(
        git_dir / "refs" / "heads" / GIT_BRANCH, 1024, "Git branch ref"
    )
    try:
        head_commit = ref_body.decode("ascii").removesuffix("\n")
    except UnicodeDecodeError as error:
        raise PublicResetError("Git branch ref is not ASCII") from error
    if COMMIT_RE.fullmatch(head_commit) is None or ref_body != f"{head_commit}\n".encode():
        fail("Git branch ref is not one canonical SHA-1")

    object_path = git_dir / "objects" / head_commit[:2] / head_commit[2:]
    compressed, _ = _read_regular(object_path, 4 * 1024 * 1024, "Git HEAD commit object")
    decompressor = zlib.decompressobj()
    inflated = decompressor.decompress(compressed, 4 * 1024 * 1024 + 1)
    if decompressor.unconsumed_tail or len(inflated) > 4 * 1024 * 1024:
        fail("Git HEAD commit object exceeds its bound")
    inflated += decompressor.flush()
    if (
        len(inflated) > 4 * 1024 * 1024
        or not decompressor.eof
        or decompressor.unused_data
        or hashlib.sha1(inflated).hexdigest() != head_commit
    ):
        fail("Git HEAD commit object disagrees with the branch ref")
    header, separator, commit_body = inflated.partition(b"\0")
    if separator != b"\0" or header != f"commit {len(commit_body)}".encode("ascii"):
        fail("Git HEAD commit object is malformed")
    first_line = commit_body.splitlines()[0] if commit_body else b""
    if not first_line.startswith(b"tree "):
        fail("Git HEAD commit does not name a root tree")
    try:
        head_tree = first_line[5:].decode("ascii")
    except UnicodeDecodeError as error:
        raise PublicResetError("Git HEAD tree is not ASCII") from error
    if COMMIT_RE.fullmatch(head_tree) is None:
        fail("Git HEAD tree is not one canonical SHA-1")

    index, _ = _read_regular(git_dir / "index", MAX_GIT_INDEX_BYTES, "Git index")
    if len(index) < 32 or hashlib.sha1(index[:-20]).digest() != index[-20:]:
        fail("Git index checksum is invalid")
    if index[:4] != b"DIRC":
        fail("Git index signature is invalid")
    version, count = struct.unpack(">II", index[4:12])
    if version != 2 or not 1 <= count <= MAX_SOURCE_FILES:
        fail("Git index must use bounded canonical version 2")
    offset = 12
    entries: list[dict[str, str]] = []
    previous = b""
    for _ in range(count):
        start = offset
        if offset + 62 > len(index) - 20:
            fail("Git index entry is truncated")
        mode = struct.unpack(">I", index[offset + 24 : offset + 28])[0]
        blob = index[offset + 40 : offset + 60].hex()
        flags = struct.unpack(">H", index[offset + 60 : offset + 62])[0]
        try:
            end = index.index(b"\0", offset + 62, len(index) - 20)
        except ValueError:
            fail("Git index path is unterminated")
        raw_path = index[offset + 62 : end]
        if (
            not raw_path
            or raw_path <= previous
            or flags & 0x3000
            or mode not in {0o100644, 0o100755, 0o120000, 0o160000}
        ):
            fail("Git index contains a noncanonical tracked entry")
        previous = raw_path
        try:
            path = raw_path.decode("utf-8")
        except UnicodeDecodeError as error:
            raise PublicResetError("Git index path is not UTF-8") from error
        if (flags & 0x0FFF) < 0x0FFF and (flags & 0x0FFF) != len(raw_path):
            fail("Git index path length flags are inconsistent")
        entries.append({"path": path, "mode": f"{mode:o}", "blob_sha1": blob})
        offset = start + ((end + 1 - start + 7) // 8) * 8

    cached_tree = None
    cached_count = None
    while offset < len(index) - 20:
        if offset + 8 > len(index) - 20:
            fail("Git index extension header is truncated")
        signature = index[offset : offset + 4]
        size = struct.unpack(">I", index[offset + 4 : offset + 8])[0]
        data_start = offset + 8
        data_end = data_start + size
        if data_end > len(index) - 20:
            fail("Git index extension is truncated")
        if signature == b"TREE":
            data = index[data_start:data_end]
            try:
                nul = data.index(b"\0")
                newline = data.index(b"\n", nul + 1)
                entry_count, _subtrees = data[nul + 1 : newline].split(b" ", 1)
                cached_count = int(entry_count)
            except (ValueError, IndexError) as error:
                raise PublicResetError("Git index cache-tree root is malformed") from error
            if nul != 0 or cached_count < 0 or newline + 21 > len(data):
                fail("Git index cache-tree root is invalid")
            cached_tree = data[newline + 1 : newline + 21].hex()
        offset = data_end
    if offset != len(index) - 20 or cached_count != count or cached_tree != head_tree:
        fail("Git index is not the exact current HEAD tree")
    return head_commit, head_tree, tuple(entries)


@dataclasses.dataclass(frozen=True)
class Artifact:
    role: str
    source_path: Path
    install_path: str
    sha256: str
    mode: str
    max_bytes: int

    def wire(self) -> dict[str, object]:
        return {
            "role": self.role,
            "source_path": str(self.source_path),
            "install_path": self.install_path,
            "sha256": self.sha256,
            "mode": self.mode,
            "max_bytes": self.max_bytes,
        }


@dataclasses.dataclass(frozen=True)
class Validator:
    id: str
    host_identity_sha256: str
    platform: dict[str, object]
    service: dict[str, object]
    artifacts: tuple[Artifact, ...]
    artifact_set_sha256: str
    attestation_path: Path
    attestation_sha256: str
    attestation: dict[str, object]

    def closure_wire(self) -> dict[str, object]:
        return {
            "id": self.id,
            "host_identity_sha256": self.host_identity_sha256,
            "platform": self.platform,
            "service": self.service,
            "artifacts": [artifact.wire() for artifact in self.artifacts],
            "artifact_set_sha256": self.artifact_set_sha256,
            "preflight_attestation_sha256": self.attestation_sha256,
        }


@dataclasses.dataclass(frozen=True)
class Edge:
    id: str
    host_identity_sha256: str
    platform: dict[str, object]
    service: dict[str, object]
    nginx: dict[str, object]
    config: Artifact
    authority: dict[str, object]
    routes: tuple[dict[str, str], ...]
    attestation_path: Path
    attestation_sha256: str
    attestation: dict[str, object]

    def closure_wire(self) -> dict[str, object]:
        return {
            "id": self.id,
            "host_identity_sha256": self.host_identity_sha256,
            "platform": self.platform,
            "service": self.service,
            "nginx": self.nginx,
            "config": self.config.wire(),
            "authority": self.authority,
            "validator_routes": list(self.routes),
            "preflight_attestation_sha256": self.attestation_sha256,
        }


@dataclasses.dataclass(frozen=True)
class Inventory:
    deployment_id: str
    previous_genesis_hash: str
    genesis_hash: str
    source_commit: str
    source_tree_sha256: str
    source_manifest_sha256: str
    validators: tuple[Validator, ...]
    edge: Edge
    not_before: int
    expires_at: int
    approval_nonce_sha256: str
    artifact_closure_sha256: str
    inventory_sha256: str

    @property
    def confirmation(self) -> str:
        return (
            f"{CONFIRMATION_PREFIX}:{self.inventory_sha256}:"
            f"{self.approval_nonce_sha256}"
        )


def _parse_artifact(value: object, label: str) -> Artifact:
    payload = _exact(
        value,
        {"role", "source_path", "install_path", "sha256", "mode", "max_bytes"},
        label,
    )
    role = _identifier(payload["role"], f"{label}.role")
    source = _local_path(payload["source_path"], f"{label}.source_path")
    install = _remote_path(payload["install_path"], f"{label}.install_path")
    digest = _sha256(payload["sha256"], f"{label}.sha256")
    mode = _text(payload["mode"], f"{label}.mode", 4)
    if re.fullmatch(r"0[0-7]{3}", mode) is None:
        fail(f"{label}.mode must be one four-digit octal string")
    maximum = _integer(payload["max_bytes"], f"{label}.max_bytes", 1, MAX_ARTIFACT_BYTES)
    actual, metadata = _hash_regular(source, maximum, f"{label} staged file")
    if actual != digest:
        fail(f"{label} staged file SHA-256 disagrees with the inventory")
    if stat.S_IMODE(metadata.st_mode) != int(mode, 8):
        fail(f"{label} staged file mode disagrees with the inventory")
    return Artifact(role, source, install, digest, mode, maximum)


def _host_artifact_sha256(artifacts: tuple[Artifact, ...]) -> str:
    return hashlib.sha256(
        HOST_ARTIFACT_DOMAIN
        + canonical_json_bytes([artifact.wire() for artifact in artifacts])
    ).hexdigest()


def _load_pinned_json(path_value: object, sha_value: object, label: str) -> tuple[Path, str, dict[str, Any]]:
    path = _private_input_path(_local_path(path_value, f"{label}.path"), label)
    expected = _sha256(sha_value, f"{label}.sha256")
    body, metadata = _read_regular(path, MAX_JSON_BYTES, label)
    _owner_private(metadata, label)
    if hashlib.sha256(body).hexdigest() != expected:
        fail(f"{label} SHA-256 disagrees with the inventory")
    payload = _decode_json(body, label)
    if body != canonical_json_bytes(payload):
        fail(f"{label} must use canonical JSON bytes")
    return path, expected, payload


def _load_source(value: object) -> tuple[str, str, str]:
    payload = _exact(
        value,
        {
            "root",
            "manifest_path",
            "manifest_sha256",
            "branch",
            "head_commit_sha1",
            "head_tree_sha1",
            "tracked_tree_sha256",
            "cargo_lock_sha256",
            "planner_relative_path",
            "planner_sha256",
            "constants_relative_path",
            "constants_sha256",
        },
        "source",
    )
    root = _local_path(payload["root"], "source.root")
    try:
        root_metadata = root.lstat()
        resolved_root = root.resolve(strict=True)
    except OSError as error:
        raise PublicResetError("source root is unavailable") from error
    if (
        root != resolved_root
        or not stat.S_ISDIR(root_metadata.st_mode)
        or stat.S_IMODE(root_metadata.st_mode) & 0o022
    ):
        fail("source root must be one direct non-group-writable directory")

    actual_commit, actual_tree, actual_entries = _git_snapshot(root)
    if payload["branch"] != GIT_BRANCH:
        fail(f"source.branch must be exactly {GIT_BRANCH}")
    commit = _text(payload["head_commit_sha1"], "source.head_commit_sha1", 40)
    tree = _text(payload["head_tree_sha1"], "source.head_tree_sha1", 40)
    if commit != actual_commit or tree != actual_tree:
        fail("source HEAD/tree disagrees with the current optimizations checkout")
    tracked_tree = _sha256(payload["tracked_tree_sha256"], "source.tracked_tree_sha256")
    computed_tree = hashlib.sha256(
        SOURCE_DOMAIN + canonical_json_bytes(list(actual_entries))
    ).hexdigest()
    if tracked_tree != computed_tree:
        fail("source tracked-tree SHA-256 disagrees with the current Git index")

    _manifest_path, manifest_sha, manifest = _load_pinned_json(
        payload["manifest_path"], payload["manifest_sha256"], "source manifest"
    )
    manifest = _exact(
        manifest,
        {
            "schema",
            "schema_version",
            "branch",
            "head_commit_sha1",
            "head_tree_sha1",
            "tracked_tree_sha256",
            "tracked_files",
        },
        "source manifest",
    )
    if (
        manifest["schema"] != SOURCE_SCHEMA
        or manifest["schema_version"] != SCHEMA_VERSION
        or manifest["branch"] != GIT_BRANCH
        or manifest["head_commit_sha1"] != actual_commit
        or manifest["head_tree_sha1"] != actual_tree
        or manifest["tracked_tree_sha256"] != tracked_tree
        or manifest["tracked_files"] != list(actual_entries)
    ):
        fail("source manifest is not the complete exact current tracked tree")

    tracked_by_path = {entry["path"]: entry for entry in actual_entries}

    def bind_worktree_file(
        relative_value: object,
        digest_value: object,
        label: str,
        *,
        require_tracked: bool,
    ) -> tuple[str, Path]:
        relative = _relative_path(relative_value, f"source.{label}_relative_path")
        expected = _sha256(digest_value, f"source.{label}_sha256")
        path = root.joinpath(*PurePosixPath(relative).parts)
        try:
            resolved = path.resolve(strict=True)
            resolved.relative_to(resolved_root)
        except (OSError, ValueError) as error:
            raise PublicResetError(f"source {label} escapes or is unavailable") from error
        if resolved != path:
            fail(f"source {label} traverses a symlink")
        body, _ = _read_regular(path, MAX_JSON_BYTES, f"source {label}")
        if hashlib.sha256(body).hexdigest() != expected:
            fail(f"source {label} SHA-256 disagrees with the inventory")
        tracked = tracked_by_path.get(relative)
        if require_tracked and tracked is None:
            fail(f"source {label} must be tracked by the current HEAD tree")
        if tracked is not None and (
            tracked["mode"] != "100644" or tracked["blob_sha1"] != _git_blob_sha1(body)
        ):
            fail(f"source {label} worktree bytes disagree with the current HEAD tree")
        return relative, path

    cargo_relative, _cargo_path = bind_worktree_file(
        "Cargo.lock", payload["cargo_lock_sha256"], "cargo_lock", require_tracked=True
    )
    if cargo_relative != "Cargo.lock":
        fail("source Cargo.lock path is not canonical")
    planner_relative, planner_path = bind_worktree_file(
        payload["planner_relative_path"],
        payload["planner_sha256"],
        "planner",
        require_tracked=False,
    )
    constants_relative, constants_path = bind_worktree_file(
        payload["constants_relative_path"],
        payload["constants_sha256"],
        "constants",
        require_tracked=True,
    )
    if planner_path != Path(__file__).resolve():
        fail("running reset planner is not the source-closed planner path")
    if constants_path != Path(str(_taira_constants.__file__)).resolve():
        fail("loaded Taira constants are not the source-closed constants path")
    if planner_relative != "scripts/taira_public_reset.py":
        fail("source planner path is not canonical")
    if constants_relative != "scripts/taira_constants.py":
        fail("source constants path is not canonical")
    return commit, tracked_tree, manifest_sha


def _parse_platform(value: object, label: str, *, validator: bool) -> dict[str, object]:
    fields = {"system", "machine"}
    if validator:
        fields |= {"kvm_device_path", "kvm_api_version", "mountinfo_path"}
    payload = _exact(value, fields, label)
    system = _text(payload["system"], f"{label}.system")
    machine = _text(payload["machine"], f"{label}.machine")
    if system != "Linux" or (validator and machine != "aarch64"):
        fail(f"{label} must declare Linux{'/aarch64' if validator else ''}")
    result: dict[str, object] = {"system": system, "machine": machine}
    if validator:
        result.update(
            {
                "kvm_device_path": _remote_path(
                    payload["kvm_device_path"], f"{label}.kvm_device_path"
                ),
                "kvm_api_version": _integer(
                    payload["kvm_api_version"], f"{label}.kvm_api_version", 1, 255
                ),
                "mountinfo_path": _remote_path(
                    payload["mountinfo_path"], f"{label}.mountinfo_path"
                ),
            }
        )
        if result["kvm_api_version"] != KVM_API_VERSION:
            fail(f"{label} must declare KVM API version 12")
        if result["kvm_device_path"] != "/dev/kvm":
            fail(f"{label} must bind the canonical /dev/kvm device")
        if result["mountinfo_path"] != "/proc/self/mountinfo":
            fail(f"{label} must bind the canonical /proc/self/mountinfo path")
    return result


def _parse_service(value: object, label: str, *, validator: bool) -> dict[str, object]:
    fields = {
        "manager_path",
        "unit",
        "unit_path",
        "unit_sha256",
        "service_root",
        "service_guard_path",
    }
    if validator:
        fields |= {"state_root", "state_guard_path", "state_lock_path"}
    else:
        fields |= {"operator_uid", "temporary_root", "target_config_path"}
    payload = _exact(value, fields, label)
    unit = _text(payload["unit"], f"{label}.unit", 255)
    if UNIT_RE.fullmatch(unit) is None:
        fail(f"{label}.unit must be one explicit systemd service unit")
    service_root = _remote_path(payload["service_root"], f"{label}.service_root", guarded_root=True)
    service_guard = _remote_path(payload["service_guard_path"], f"{label}.service_guard_path")
    _direct_child(service_guard, service_root, f"{label}.service_guard_path")
    result: dict[str, object] = {
        "manager_path": _remote_path(payload["manager_path"], f"{label}.manager_path"),
        "unit": unit,
        "unit_path": _remote_path(payload["unit_path"], f"{label}.unit_path"),
        "unit_sha256": _sha256(payload["unit_sha256"], f"{label}.unit_sha256"),
        "service_root": service_root,
        "service_guard_path": service_guard,
    }
    if validator:
        state_root = _remote_path(payload["state_root"], f"{label}.state_root", guarded_root=True)
        if state_root == service_root or _descendant(state_root, service_root) or _descendant(service_root, state_root):
            fail(f"{label} service and state roots must be disjoint")
        state_guard = _remote_path(payload["state_guard_path"], f"{label}.state_guard_path")
        state_lock = _remote_path(payload["state_lock_path"], f"{label}.state_lock_path")
        _direct_child(state_guard, state_root, f"{label}.state_guard_path")
        _direct_child(state_lock, state_root, f"{label}.state_lock_path")
        if state_guard == state_lock:
            fail(f"{label} state guard and lock paths must differ")
        result.update(
            {"state_root": state_root, "state_guard_path": state_guard, "state_lock_path": state_lock}
        )
    else:
        operator_uid = _integer(payload["operator_uid"], f"{label}.operator_uid", 0, 1 << 31)
        temporary_root = _remote_path(
            payload["temporary_root"], f"{label}.temporary_root", guarded_root=True
        )
        target = _remote_path(payload["target_config_path"], f"{label}.target_config_path")
        if not _descendant(temporary_root, service_root) or not _descendant(target, service_root):
            fail(f"{label} temporary root and target config must stay below service root")
        _direct_child(target, service_root, f"{label}.target_config_path")
        result.update(
            {
                "operator_uid": operator_uid,
                "temporary_root": temporary_root,
                "target_config_path": target,
            }
        )
    return result


def _parse_attestation_ref(value: object, label: str) -> tuple[Path, str, dict[str, Any]]:
    payload = _exact(value, {"path", "sha256"}, label)
    return _load_pinned_json(payload["path"], payload["sha256"], label)


def _parse_validator(
    value: object,
    index: int,
    *,
    deployment_id: str,
    genesis_hash: str,
    source_tree_sha256: str,
) -> Validator:
    label = f"validators[{index}]"
    payload = _exact(
        value,
        {"id", "host_identity_sha256", "platform", "service", "artifacts", "preflight_attestation"},
        label,
    )
    identifier = _identifier(payload["id"], f"{label}.id")
    if identifier != SLUGS[index]:
        fail(f"{label}.id must be exactly {SLUGS[index]!r}")
    identity = _sha256(payload["host_identity_sha256"], f"{label}.host_identity_sha256")
    platform_value = _parse_platform(payload["platform"], f"{label}.platform", validator=True)
    service = _parse_service(payload["service"], f"{label}.service", validator=True)
    service_root = VALIDATOR_SERVICE_ROOT.format(validator_id=identifier)
    state_root = VALIDATOR_STATE_ROOT.format(validator_id=identifier)
    expected_service = {
        "manager_path": SYSTEMCTL_PATH,
        "unit": f"{identifier}.service",
        "unit_path": f"/etc/systemd/system/{identifier}.service",
        "service_root": service_root,
        "service_guard_path": f"{service_root}/.taira-service-root",
        "state_root": state_root,
        "state_guard_path": f"{state_root}/.taira-state-root",
        "state_lock_path": f"{state_root}/.taira-state-lock",
    }
    if any(service[field] != expected for field, expected in expected_service.items()):
        fail(f"{label}.service must use the exact canonical Taira paths and unit")
    raw_artifacts = payload["artifacts"]
    if not isinstance(raw_artifacts, list) or len(raw_artifacts) != len(
        REQUIRED_ARTIFACT_ROLES
    ):
        fail(f"{label}.artifacts must contain exactly the five V1 roles")
    artifacts = tuple(
        _parse_artifact(raw, f"{label}.artifacts[{artifact_index}]")
        for artifact_index, raw in enumerate(raw_artifacts)
    )
    roles = [artifact.role for artifact in artifacts]
    if roles != sorted(REQUIRED_ARTIFACT_ROLES):
        fail(f"{label}.artifacts must be exactly the five canonical V1 roles in order")
    installs = {artifact.install_path for artifact in artifacts}
    if len(installs) != len(artifacts):
        fail(f"{label}.artifact install paths must be unique")
    controls = {
        str(service["unit_path"]),
        str(service["service_guard_path"]),
        str(service["state_guard_path"]),
        str(service["state_lock_path"]),
    }
    if installs & controls:
        fail(f"{label}.artifacts collide with a service/state control path")
    for artifact in artifacts:
        source_name, install_relative, mode, maximum = VALIDATOR_ARTIFACT_SPECS[
            artifact.role
        ]
        source_name = source_name.format(validator_id=identifier)
        expected_install = f"{service_root}/{install_relative}"
        if (
            artifact.source_path.name != source_name
            or artifact.install_path != expected_install
            or artifact.mode != mode
            or artifact.max_bytes != maximum
        ):
            fail(f"{label}.{artifact.role} does not use its exact V1 source/install contract")
    artifact_set = _host_artifact_sha256(artifacts)
    attestation_path, attestation_sha, attestation = _parse_attestation_ref(
        payload["preflight_attestation"], f"{label}.preflight_attestation"
    )
    fields = {
        "schema", "schema_version", "deployment_id", "host_id", "host_identity_sha256",
        "platform", "service", "artifact_set_sha256", "genesis_hash", "source_tree_sha256",
        "daemon_config_validated", "inrou_startup_qualified", "inrou_identity",
        "attestation_authority", "expires_at_unix_seconds",
    }
    attestation = _exact(attestation, fields, f"{label} attestation")
    if (
        attestation["schema"] != VALIDATOR_ATTESTATION_SCHEMA
        or attestation["schema_version"] != SCHEMA_VERSION
        or attestation["deployment_id"] != deployment_id
        or attestation["host_id"] != identifier
        or attestation["host_identity_sha256"] != identity
        or attestation["platform"] != platform_value
        or attestation["service"] != service
        or attestation["artifact_set_sha256"] != artifact_set
        or attestation["genesis_hash"] != genesis_hash
        or attestation["source_tree_sha256"] != source_tree_sha256
        or attestation["daemon_config_validated"] is not True
        or attestation["inrou_startup_qualified"] is not True
    ):
        fail(f"{label} attestation does not bind the complete host preflight")
    inrou_identity = _exact(
        attestation["inrou_identity"],
        {
            "name",
            "slot",
            "uid",
            "gid",
            "home",
            "shell",
            "locked",
            "primary_group_members",
            "nss_supplementary_groups",
            "nss_sources",
        },
        f"{label} attestation.inrou_identity",
    )
    if (
        inrou_identity["name"] != INROU_NAME
        or inrou_identity["slot"] != 0
        or inrou_identity["uid"] != INROU_ID
        or inrou_identity["gid"] != INROU_ID
        or inrou_identity["home"] != INROU_HOME
        or inrou_identity["shell"] not in INROU_SHELLS
        or inrou_identity["locked"] is not True
        or inrou_identity["primary_group_members"] != []
        or inrou_identity["nss_supplementary_groups"] != []
        or inrou_identity["nss_sources"] != ["files"]
    ):
        fail(f"{label} attestation must bind the exact files-only iroha-inrou-0 identity")
    _identifier(attestation["attestation_authority"], f"{label}.attestation_authority")
    _integer(attestation["expires_at_unix_seconds"], f"{label}.attestation expiry", 1, 1 << 62)
    return Validator(
        identifier, identity, platform_value, service, artifacts, artifact_set,
        attestation_path, attestation_sha, attestation,
    )


def _parse_edge(
    value: object,
    *,
    deployment_id: str,
    genesis_hash: str,
    source_tree_sha256: str,
) -> Edge:
    payload = _exact(
        value,
        {
            "id", "host_identity_sha256", "platform", "service", "nginx", "config",
            "authority", "validator_routes", "preflight_attestation",
        },
        "edge_authority",
    )
    identifier = _identifier(payload["id"], "edge_authority.id")
    identity = _sha256(payload["host_identity_sha256"], "edge_authority.host_identity_sha256")
    platform_value = _parse_platform(payload["platform"], "edge_authority.platform", validator=False)
    service = _parse_service(payload["service"], "edge_authority.service", validator=False)
    nginx_payload = _exact(payload["nginx"], {"path", "sha256"}, "edge_authority.nginx")
    nginx = {
        "path": _remote_path(nginx_payload["path"], "edge_authority.nginx.path"),
        "sha256": _sha256(nginx_payload["sha256"], "edge_authority.nginx.sha256"),
    }
    config = _parse_artifact(payload["config"], "edge_authority.config")
    expected_edge_service = dict(EDGE_SERVICE)
    if any(service[field] != expected for field, expected in expected_edge_service.items()):
        fail("edge service must use the exact canonical Linux Taira paths and unit")
    if nginx["path"] != EDGE_NGINX_PATH:
        fail("edge nginx path must be the canonical Linux nginx binary")
    if (
        config.role != "edge_config"
        or config.source_path.name != EDGE_CONFIG_SOURCE_NAME
        or config.install_path != service["target_config_path"]
        or config.mode != EDGE_CONFIG_MODE
        or config.max_bytes != EDGE_CONFIG_MAX_BYTES
    ):
        fail("edge config must bind the exact canonical source and target include")
    authority_payload = _exact(
        payload["authority"],
        {
            "public_root",
            "tls_authority",
            "dns_authority",
            "public_validator",
            "colocation_validator",
        },
        "edge_authority.authority",
    )
    public_root = _text(authority_payload["public_root"], "edge public_root", 512)
    public_hostname = public_root[8:] if public_root.startswith("https://") else ""
    if not public_hostname:
        fail("edge public_root must be one path-free HTTPS origin")
    _canonical_dns_name(public_hostname, "edge public_root hostname")
    if public_root != f"https://{public_hostname}":
        fail("edge public_root must be one path-free HTTPS origin")
    if public_root != PUBLIC_ROOT:
        fail("edge public_root must be the canonical public Taira origin")
    colocation = authority_payload["colocation_validator"]
    if colocation is not None:
        colocation = _identifier(colocation, "edge colocation_validator")
        if colocation not in SLUGS:
            fail("edge colocation_validator must name one canonical validator or null")
    authority = {
        "public_root": public_root,
        "tls_authority": _identifier(authority_payload["tls_authority"], "edge tls_authority"),
        "dns_authority": _identifier(authority_payload["dns_authority"], "edge dns_authority"),
        "public_validator": _identifier(authority_payload["public_validator"], "edge public_validator"),
        "colocation_validator": colocation,
    }
    if authority["public_validator"] not in SLUGS:
        fail("edge public_validator must name one canonical validator")
    routes_raw = payload["validator_routes"]
    if not isinstance(routes_raw, list) or len(routes_raw) != PEER_COUNT:
        fail("edge validator_routes must contain exactly four entries")
    routes: list[dict[str, str]] = []
    for index, raw in enumerate(routes_raw):
        route = _exact(raw, {"validator_id", "hostname", "upstream"}, f"edge route[{index}]")
        validator_id = _identifier(route["validator_id"], f"edge route[{index}].validator_id")
        hostname = _text(route["hostname"], f"edge route[{index}].hostname", 253)
        upstream = _canonical_upstream(route["upstream"], f"edge route[{index}].upstream")
        if validator_id != SLUGS[index] or hostname != VALIDATOR_HOSTS[index]:
            fail("edge routes must use canonical validator order and hostnames")
        _canonical_dns_name(hostname, f"edge route[{index}].hostname")
        routes.append({"validator_id": validator_id, "hostname": hostname, "upstream": upstream})
    if len({route["hostname"] for route in routes}) != PEER_COUNT or len(
        {route["upstream"] for route in routes}
    ) != PEER_COUNT:
        fail("edge route hostnames and upstreams must each be distinct")
    attestation_path, attestation_sha, attestation = _parse_attestation_ref(
        payload["preflight_attestation"], "edge_authority.preflight_attestation"
    )
    fields = {
        "schema", "schema_version", "deployment_id", "edge_id", "host_identity_sha256",
        "platform", "service", "nginx_sha256", "config_sha256", "genesis_hash",
        "source_tree_sha256", "public_validator", "target_parent_direct",
        "target_parent_owner_uid", "target_parent_non_group_writable",
        "target_leaf_direct_regular",
        "target_leaf_nlink", "staged_nginx_validated", "rollback_armed_through_reload",
        "attestation_authority", "expires_at_unix_seconds",
    }
    attestation = _exact(attestation, fields, "edge attestation")
    if (
        attestation["schema"] != EDGE_ATTESTATION_SCHEMA
        or attestation["schema_version"] != SCHEMA_VERSION
        or attestation["deployment_id"] != deployment_id
        or attestation["edge_id"] != identifier
        or attestation["host_identity_sha256"] != identity
        or attestation["platform"] != platform_value
        or attestation["service"] != service
        or attestation["nginx_sha256"] != nginx["sha256"]
        or attestation["config_sha256"] != config.sha256
        or attestation["genesis_hash"] != genesis_hash
        or attestation["source_tree_sha256"] != source_tree_sha256
        or attestation["public_validator"] != authority["public_validator"]
        or attestation["target_parent_direct"] is not True
        or attestation["target_parent_owner_uid"] != service["operator_uid"]
        or attestation["target_parent_non_group_writable"] is not True
        or attestation["target_leaf_direct_regular"] is not True
        or attestation["target_leaf_nlink"] != 1
        or attestation["staged_nginx_validated"] is not True
        or attestation["rollback_armed_through_reload"] is not True
    ):
        fail("edge attestation does not bind the complete safe cutover preflight")
    _identifier(attestation["attestation_authority"], "edge attestation authority")
    _integer(attestation["expires_at_unix_seconds"], "edge attestation expiry", 1, 1 << 62)
    return Edge(
        identifier, identity, platform_value, service, nginx, config, authority,
        tuple(routes), attestation_path, attestation_sha, attestation,
    )


def load_inventory(path: Path, *, now: int | None = None) -> Inventory:
    path = _private_input_path(path, "operator inventory")
    body, metadata = _read_regular(path, MAX_JSON_BYTES, "operator inventory")
    _owner_private(metadata, "operator inventory")
    inventory_sha = hashlib.sha256(body).hexdigest()
    payload = _decode_json(body, "operator inventory")
    if body != canonical_json_bytes(payload):
        fail("operator inventory must use canonical JSON bytes")
    payload = _exact(
        payload,
        {
            "schema", "schema_version", "deployment_id", "chain_id", "chain_discriminant",
            "previous_genesis_hash", "genesis_hash", "source", "mutation_window",
            "validators", "edge_authority", "artifact_closure_sha256",
        },
        "operator inventory",
    )
    if payload["schema"] != INVENTORY_SCHEMA or payload["schema_version"] != SCHEMA_VERSION:
        fail("operator inventory schema/version is unsupported")
    deployment_id = _identifier(payload["deployment_id"], "deployment_id")
    if payload["chain_id"] != CHAIN_ID:
        fail("operator inventory chain_id must match canonical Taira")
    if payload["chain_discriminant"] != CHAIN_DISCRIMINANT:
        fail("operator inventory chain_discriminant must match canonical Taira")
    previous = _sha256(payload["previous_genesis_hash"], "previous_genesis_hash")
    genesis = _sha256(payload["genesis_hash"], "genesis_hash")
    if previous == genesis:
        fail("fresh reset genesis hash must differ from previous genesis hash")
    source_commit, source_tree, source_manifest = _load_source(payload["source"])
    window = _exact(
        payload["mutation_window"],
        {"not_before_unix_seconds", "expires_at_unix_seconds", "approval_nonce_sha256"},
        "mutation_window",
    )
    not_before = _integer(window["not_before_unix_seconds"], "mutation not_before", 1, 1 << 62)
    expires_at = _integer(window["expires_at_unix_seconds"], "mutation expires_at", 1, 1 << 62)
    if expires_at <= not_before or expires_at - not_before > MAX_WINDOW_SECONDS:
        fail("mutation window must be positive and no longer than 24 hours")
    nonce = _sha256(window["approval_nonce_sha256"], "mutation approval nonce SHA-256")
    validators_raw = payload["validators"]
    if not isinstance(validators_raw, list) or len(validators_raw) != PEER_COUNT:
        fail("operator inventory must contain exactly four validators")
    validators = tuple(
        _parse_validator(
            raw, index, deployment_id=deployment_id, genesis_hash=genesis,
            source_tree_sha256=source_tree,
        )
        for index, raw in enumerate(validators_raw)
    )
    if len({validator.host_identity_sha256 for validator in validators}) != PEER_COUNT:
        fail("the four Linux/AArch64/KVM validator hosts must be distinct")
    by_role = [
        {artifact.role: artifact.sha256 for artifact in validator.artifacts}
        for validator in validators
    ]
    for role in {"binary", "genesis", "genesis_hash", "iroha_cli"}:
        if len({artifacts[role] for artifacts in by_role}) != 1:
            fail(f"all validators must share the same {role} artifact")
    if len({artifacts["config"] for artifacts in by_role}) != PEER_COUNT:
        fail("each validator must have a distinct config artifact")
    for validator in validators:
        genesis_hash_artifact = next(
            artifact for artifact in validator.artifacts if artifact.role == "genesis_hash"
        )
        body, _ = _read_regular(
            genesis_hash_artifact.source_path,
            65,
            f"{validator.id} genesis_hash artifact",
        )
        if body != f"{genesis}\n".encode("ascii"):
            fail(f"{validator.id} genesis_hash artifact does not contain the declared hash")
    edge = _parse_edge(
        payload["edge_authority"], deployment_id=deployment_id,
        genesis_hash=genesis, source_tree_sha256=source_tree,
    )
    identities = {validator.id: validator.host_identity_sha256 for validator in validators}
    colocation = edge.authority["colocation_validator"]
    if colocation is None:
        if edge.host_identity_sha256 in identities.values():
            fail("a distinct edge host must not reuse a validator host identity")
    elif edge.host_identity_sha256 != identities[colocation]:
        fail("edge host identity must match its explicit colocation validator")
    closure_payload = {
        "schema": "iroha.taira.public-reset.artifact-closure.v1",
        "schema_version": SCHEMA_VERSION,
        "deployment_id": deployment_id,
        "chain_id": CHAIN_ID,
        "chain_discriminant": CHAIN_DISCRIMINANT,
        "previous_genesis_hash": previous,
        "genesis_hash": genesis,
        "source_commit": source_commit,
        "source_tree_sha256": source_tree,
        "source_manifest_sha256": source_manifest,
        "validators": [validator.closure_wire() for validator in validators],
        "edge_authority": edge.closure_wire(),
    }
    computed_closure = hashlib.sha256(
        ARTIFACT_DOMAIN + canonical_json_bytes(closure_payload)
    ).hexdigest()
    expected_closure = _sha256(
        payload["artifact_closure_sha256"], "artifact_closure_sha256"
    )
    if computed_closure != expected_closure:
        fail("artifact closure SHA-256 disagrees with the canonical inventory")
    current = int(time.time()) if now is None else now
    if not not_before <= current < expires_at:
        fail("operator inventory mutation window is not currently active")
    for validator in validators:
        expiry = int(validator.attestation["expires_at_unix_seconds"])
        if not current < expiry <= expires_at:
            fail(f"{validator.id} preflight attestation is expired or outlives approval")
    edge_expiry = int(edge.attestation["expires_at_unix_seconds"])
    if not current < edge_expiry <= expires_at:
        fail("edge preflight attestation is expired or outlives approval")
    return Inventory(
        deployment_id, previous, genesis, source_commit, source_tree, source_manifest,
        validators, edge, not_before, expires_at, nonce, expected_closure, inventory_sha,
    )


def handoff_report(
    inventory: Inventory, *, confirmation_validated: bool
) -> dict[str, object]:
    report: dict[str, object] = {
        "schema": HANDOFF_SCHEMA,
        "schema_version": SCHEMA_VERSION,
        "status": (
            "confirmed-offline-handoff"
            if confirmation_validated
            else "offline-evidence-validated"
        ),
        "confirmation_validated": confirmation_validated,
        "authorization_granted": False,
        "executor_available": False,
        "live_preflight_performed": False,
        "mutation_possible": False,
        "deployment_id": inventory.deployment_id,
        "chain_id": CHAIN_ID,
        "chain_discriminant": CHAIN_DISCRIMINANT,
        "previous_genesis_hash": inventory.previous_genesis_hash,
        "genesis_hash": inventory.genesis_hash,
        "inventory_sha256": inventory.inventory_sha256,
        "source_commit": inventory.source_commit,
        "source_tree_sha256": inventory.source_tree_sha256,
        "artifact_closure_sha256": inventory.artifact_closure_sha256,
        "expires_at_unix_seconds": inventory.expires_at,
        "validators": [validator.id for validator in inventory.validators],
        "edge_authority": inventory.edge.id,
        "public_validator": inventory.edge.authority["public_validator"],
        "phases": [
            "acquire-durable-global-deployment-lock-and-edge-rollback-barrier",
            "replay-attestations-and-stop-exact-four-validator-services",
            "install-exact-per-host-artifact-closure-and-reset-only-guarded-state-roots",
            "start-all-four-and-require-genesis-bound-readiness-height-convergence",
            "require-signed-applied-canary-and-bounded-validator-restart-proof",
            "isolated-nginx-test-install-live-test-reload-with-rollback-armed",
            "persist-nonreplayable-completion-receipt-and-release-barriers",
        ],
        "apply_blocker": APPLY_BARRIER,
    }
    if not confirmation_validated:
        report["confirmation_format"] = (
            f"{CONFIRMATION_PREFIX}:<inventory_sha256>:<approval_nonce_sha256>"
        )
    return report


def confirm(inventory: Inventory, confirmation: str) -> dict[str, object]:
    if confirmation != inventory.confirmation:
        fail("public mutation confirmation does not bind the exact inventory and approval nonce")
    return handoff_report(inventory, confirmation_validated=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subcommands = parser.add_subparsers(dest="command", required=True)
    preflight_parser = subcommands.add_parser("preflight")
    preflight_parser.add_argument("--inventory", type=Path, required=True)
    confirm_parser = subcommands.add_parser("confirm")
    confirm_parser.add_argument("--inventory", type=Path, required=True)
    confirm_parser.add_argument("--confirm-public-mutation", required=True)
    apply_parser = subcommands.add_parser("apply")
    apply_parser.add_argument("--inventory", type=Path, required=True)
    apply_parser.add_argument("--confirm-public-mutation", required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        inventory = load_inventory(args.inventory)
        if args.command == "preflight":
            report = handoff_report(inventory, confirmation_validated=False)
        elif args.command == "confirm":
            report = confirm(inventory, args.confirm_public_mutation)
        else:
            confirm(inventory, args.confirm_public_mutation)
            fail(APPLY_BARRIER)
        sys.stdout.buffer.write(canonical_json_bytes(report))
        return 0
    except PublicResetError as error:
        print(f"taira public reset refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
