#!/usr/bin/env python3
"""Prepare or preflight a canonical offline-enabled Taira reset bundle.

The preparation path starts from a sealed four-validator reset bundle so that
validator identities and runtime-only sidecars are preserved.  It always
creates a new bundle, replaces the archived genesis with an already
offline-bootstrapped canonical Taira genesis, copies an authenticated
Kagemusha V4 catalog and its reviewed operator identity into the bundle,
signs genesis, leaves every validator with brand-new empty storage, and binds
every generated config to the release-tree-addressed root qualification seal
that the deployment controller will create.

No secret value is written to stdout.  The command-submission signer is read
from the same owner-private file as the genesis signer and is only persisted
inside owner-private runtime configs.  Preparation rejects the archived
command key and requires the explicitly pinned command authority to be the
canonical Taira account derived from the fresh genesis public key.
"""

from __future__ import annotations

import argparse
import base64
import binascii
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import tomllib
from typing import BinaryIO, NoReturn

try:
    from scripts import prepare_taira_empty_reset_bundle as empty_reset
except ModuleNotFoundError:  # Direct `python3 scripts/...` execution.
    sibling = Path(__file__).resolve().with_name(
        "prepare_taira_empty_reset_bundle.py"
    )
    specification = importlib.util.spec_from_file_location(
        "prepare_taira_empty_reset_bundle", sibling
    )
    if specification is None or specification.loader is None:
        raise RuntimeError(
            "cannot load the Taira empty-reset helper from its absolute "
            "sibling path"
        )
    empty_reset = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(empty_reset)


PEER_COUNT = 4
VALIDATOR_SLUGS = tuple(
    f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1)
)
PUBLIC_TAIRA_CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
PUBLIC_TAIRA_CHAIN_DISCRIMINANT = 369
PUBLIC_TAIRA_BLOCK_CADENCE_MS = 4_000
PUBLIC_TAIRA_OFFLINE_ASSET_ID = "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS = "ds#boi.is"
PUBLIC_TAIRA_OFFLINE_ASSET_NAME = "ds"
PUBLIC_TAIRA_OFFLINE_ASSET_SCALE = 2
PUBLIC_TAIRA_OFFLINE_ASSET_METADATA = {
    "currency_code": "DS",
    "display_code": "DS",
    "display_name": "Digital Shekel",
    "iso_currency_code": "ILS",
    "symbol": "₪",
}
PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT = (
    "testuﾛ1Nｿyｵn2PHﾕG6VxﾊﾁpﾏR1uｼM8JｻXBpYcﾆﾎRKjAWvｾALWT5T"
)
PUBLIC_TAIRA_FEE_ASSET_ID = "6TEAJqbb8oEPmLncoNiMRbLEK6tw"
TAIRA_I105_ALPHABET = tuple(
    "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    "ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ"
)
TAIRA_I105_CHECKSUM_LEN = 6
TAIRA_I105_BECH32M_CONST = 0x2BC830A3
PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT = 2
PUBLIC_TAIRA_RELEASE_MINIMUM_WITHDRAWAL_HEIGHT = 1_000_000
# Four validators share one roughly 460 GiB APFS volume.  Bound every node's
# aggregate Nexus storage footprint so the testnet cannot consume the host's
# final restart headroom again.
PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES = 64 * 1024 * 1024 * 1024
PUBLIC_TAIRA_NODE_STORAGE_BUDGET_POLICY = "bounded-64-gib-per-validator"
PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS = {
    "kura_blocks_bps": 7_500,
    "wsv_snapshots_bps": 2_000,
    "sorafs_bps": 0,
    "soranet_spool_bps": 250,
    "soravpn_spool_bps": 250,
}
RELEASE_POLICY_FILE_NAME = "release-policy-v1.norito"
RELEASE_CATALOG_DIRECTORY_NAME = "catalog"
RELEASE_ATTESTATION_FILE_NAME = "release-attestation-v4.norito"
BENCHMARK_EVIDENCE_FILE_NAME = "physical-device-benchmark.evidence"
CRYPTOGRAPHIC_REVIEW_FILE_NAME = "cryptographic-review.evidence"
PROMOTION_RECORD_FILE_NAME = "promotion-record-v4.norito"
OPERATOR_IDENTITY_FILE_NAME = "operator-identity.json"
TAIRA_RELEASE_INSTALL_ROOT = Path("/Library/SORA/Taira/releases")
TAIRA_QUALIFICATION_SEAL_ROOT = Path("/Library/SORA/Taira/seals")
KAGEMUSHA_RELEASE_GENERATION = "production-gate-real-artifacts-v4"
KAGEMUSHA_BRIDGE_ABI_VERSION = 21
KAGEMUSHA_MAX_HOPS = 8
# Keep the packager's fail-closed per-file corridor identical to the runtime's
# non-configurable KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MAX_FILE_BYTES_V4.  The
# reviewed compact-V5 processed proving key is 5,347,763,078 bytes, so retaining
# the stale 4 GiB corridor here would reject a release that core accepts.
KAGEMUSHA_ARTIFACT_MAX_BYTES = 5 * 1024 * 1024 * 1024
KAGEMUSHA_ARTIFACT_ROLES = (
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
KAGEMUSHA_ARTIFACT_FILE_NAMES = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
KAGEMUSHA_ARTIFACT_KIND_LABELS = (
    "params_ipa",
    "proving_key",
    "verifying_key",
    "bootstrap_witness",
) * 2
KAGEMUSHA_VERIFIER_ROLES = {
    "active_transfer_verifier": (
        "halo2/ipa",
        "confidential_transfer_v2_verifier_record",
        "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    ),
    "active_topup_shield_verifier": (
        "halo2/ipa",
        "kagemusha_topup_shield_v2_verifier_record",
        "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    ),
    "active_unshield_verifier": (
        "halo2/ipa",
        "confidential_unshield_v3_verifier_record",
        "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
    ),
    "active_recursive_step_eq_verifier": (
        "halo2/ipa",
        "kagemusha_recursive_step_eq_v4_verifier_record",
        "kagemusha-recursive-spend-step-eq-compact-layout-v5",
    ),
    "active_recursive_step_ep_verifier": (
        "halo2/ipa",
        "kagemusha_recursive_step_ep_v4_verifier_record",
        "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
    ),
}
GENESIS_PUBLIC_KEY_RE = re.compile(r"ed0120[0-9A-F]{64}")
GENESIS_PRIVATE_KEY_RE = re.compile(r"802620[0-9A-F]{64}")
SHA256_RE = re.compile(r"[0-9a-f]{64}")
GENESIS_EXPECTED_HASH_PLACEHOLDER = "REPLACE_WITH_GENESIS_EXPECTED_HASH"
MANIFEST_SCHEMA = "taira-exact2f-reset-bundle"
EXPECTED_RELEASE_FILE_COUNT = 16
MAX_CONFIG_BYTES = 2 * 1024 * 1024
MAX_GENESIS_BYTES = 64 * 1024 * 1024
NORITO_HEADER_SIZE = 40
NORITO_PACKED_SEQ = 0x01
NORITO_COMPACT_LEN = 0x02
NORITO_PACKED_STRUCT = 0x04
NORITO_FIELD_BITSET = 0x20
NORITO_SUPPORTED_FLAGS = (
    NORITO_PACKED_SEQ
    | NORITO_COMPACT_LEN
    | NORITO_PACKED_STRUCT
    | NORITO_FIELD_BITSET
)


def fail(message: str) -> NoReturn:
    """Raise one preparation or preflight failure."""

    raise RuntimeError(message)


def sha256(path: Path) -> str:
    """Return the lowercase SHA-256 digest of one regular file."""

    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def require_sha256(value: str, label: str) -> str:
    """Require one canonical lowercase SHA-256 literal."""

    if SHA256_RE.fullmatch(value) is None:
        fail(f"{label} must be a lowercase SHA-256 digest")
    return value


def require_genesis_expected_hash(value: object) -> str:
    """Require one canonical Iroha hash suitable as a genesis trust root."""

    if not isinstance(value, str) or SHA256_RE.fullmatch(value) is None:
        fail("genesis expected hash must be one lowercase 32-byte Iroha hash")
    if int(value[-2:], 16) & 1 == 0:
        fail("genesis expected hash must carry the Iroha marker bit")
    return value


def require_private_directory(path: Path) -> None:
    """Require one real owner-private directory."""

    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
    ):
        fail(f"unsafe owner-private directory identity: {path}")


def private_file_identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return the stable identity fields for one owner-private file."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_uid,
        metadata.st_gid,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def require_private_file_metadata(
    path: Path, maximum_size: int
) -> os.stat_result:
    """Require bounded owner-private single-link regular-file metadata."""

    try:
        metadata = path.lstat()
    except OSError as error:
        raise RuntimeError(f"cannot inspect owner-private file: {path}") from error
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.getuid()
        or stat.S_IMODE(metadata.st_mode) & 0o077
        or metadata.st_size <= 0
        or metadata.st_size > maximum_size
    ):
        fail(f"unsafe owner-private file identity: {path}")
    return metadata


def require_private_file(path: Path, maximum_size: int) -> bytes:
    """Read a stable owner-private single-link regular file."""

    metadata = require_private_file_metadata(path, maximum_size)
    before = private_file_identity(metadata)
    payload = path.read_bytes()
    after = private_file_identity(
        require_private_file_metadata(path, maximum_size)
    )
    if after != before:
        fail(f"owner-private file changed while it was read: {path}")
    return payload


def read_genesis_expected_hash(path: Path) -> str:
    """Read Kagami's exact one-line signed-genesis hash output."""

    raw = require_private_file(path, 65)
    try:
        text = raw.decode("ascii")
    except UnicodeDecodeError as error:
        raise RuntimeError("Kagami genesis expected hash is not ASCII") from error
    if not text.endswith("\n") or text.count("\n") != 1:
        fail("Kagami genesis expected hash is not one canonical line")
    return require_genesis_expected_hash(text[:-1])


def _write_stream_chunk(stream: BinaryIO, payload: bytes) -> None:
    """Write one complete chunk to a binary stream."""

    remaining = memoryview(payload)
    while remaining:
        written = stream.write(remaining)
        if written is None or written <= 0:
            fail("owner-private streaming copy made no progress")
        remaining = remaining[written:]


def stream_private_file_sha256(
    path: Path,
    maximum_size: int,
    *,
    destination: BinaryIO | None = None,
) -> tuple[int, str]:
    """Stream, optionally copy, and hash one stable owner-private file."""

    expected_metadata = require_private_file_metadata(path, maximum_size)
    expected_identity = private_file_identity(expected_metadata)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise RuntimeError(f"cannot open owner-private file: {path}") from error
    try:
        opened_identity = private_file_identity(os.fstat(descriptor))
        if opened_identity != expected_identity:
            fail(f"owner-private file changed before it was read: {path}")
        digest = hashlib.sha256()
        remaining = expected_metadata.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                fail(f"owner-private file changed while it was read: {path}")
            remaining -= len(chunk)
            digest.update(chunk)
            if destination is not None:
                _write_stream_chunk(destination, chunk)
        if os.read(descriptor, 1):
            fail(f"owner-private file changed while it was read: {path}")
        descriptor_identity = private_file_identity(os.fstat(descriptor))
        path_identity = private_file_identity(
            require_private_file_metadata(path, maximum_size)
        )
        if (
            descriptor_identity != expected_identity
            or path_identity != expected_identity
        ):
            fail(f"owner-private file changed while it was read: {path}")
        return expected_metadata.st_size, digest.hexdigest()
    finally:
        os.close(descriptor)


def copy_private_file_streaming(
    source: Path, destination: Path, maximum_size: int
) -> tuple[int, str]:
    """Atomically stream one stable private file into a new private file."""

    require_private_directory(destination.parent)
    if destination.exists() or destination.is_symlink():
        fail(f"destination owner-private file already exists: {destination}")
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{destination.name}.",
        suffix=".tmp",
        dir=destination.parent,
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as output_stream:
            result = stream_private_file_sha256(
                source,
                maximum_size,
                destination=output_stream,
            )
            output_stream.flush()
            os.fsync(output_stream.fileno())
        os.chmod(temporary, 0o600)
        os.replace(temporary, destination)
        require_private_file_metadata(destination, maximum_size)
        return result
    finally:
        temporary.unlink(missing_ok=True)


@dataclass(frozen=True)
class ReleaseTreeEntrySnapshot:
    """Stable metadata and content identity for one release-tree entry."""

    relative_path: str
    kind: str
    device: int
    inode: int
    mode: int
    uid: int
    gid: int
    link_count: int
    size: int
    modification_time_ns: int
    content_sha256: str | None

    def metadata_identity(self) -> tuple[object, ...]:
        """Return the fields that must survive an atomic directory rename."""

        return (
            self.relative_path,
            self.kind,
            self.device,
            self.inode,
            self.mode,
            self.uid,
            self.gid,
            self.link_count,
            self.size,
            self.modification_time_ns,
        )


@dataclass(frozen=True)
class ReleaseBundleSnapshot:
    """Exact pre-move identity of one authenticated release tree."""

    entries: tuple[ReleaseTreeEntrySnapshot, ...]
    tree_sha256: str

    @property
    def device(self) -> int:
        """Return the device containing the snapshotted release root."""

        return self.entries[0].device


def _release_move_directory_metadata(
    path: Path, *, writable: bool = False
) -> os.stat_result:
    """Require one canonical owner-private directory used by a release move."""

    try:
        metadata = path.lstat()
    except OSError as error:
        raise RuntimeError(
            f"cannot inspect release-move directory: {path}"
        ) from error
    permissions = stat.S_IMODE(metadata.st_mode)
    allowed_permissions = {0o700} if writable else {0o500, 0o700}
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or permissions not in allowed_permissions
    ):
        fail(f"unsafe owner-private release-move directory identity: {path}")
    return metadata


def _release_move_file_metadata(
    path: Path, maximum_size: int
) -> os.stat_result:
    """Require one canonical owner-private file used by a release move."""

    try:
        metadata = path.lstat()
    except OSError as error:
        raise RuntimeError(f"cannot inspect release-move file: {path}") from error
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.getuid()
        or metadata.st_gid != os.getgid()
        or stat.S_IMODE(metadata.st_mode) not in {0o400, 0o600}
        or metadata.st_size <= 0
        or metadata.st_size > maximum_size
    ):
        fail(f"unsafe owner-private release-move file identity: {path}")
    return metadata


def _release_move_metadata_identity(
    metadata: os.stat_result,
) -> tuple[int, ...]:
    """Return metadata fields preserved by an in-filesystem rename."""

    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_uid,
        metadata.st_gid,
        metadata.st_nlink,
        metadata.st_size,
        metadata.st_mtime_ns,
    )


def _release_tree_entry_snapshot(
    root: Path,
    path: Path,
    *,
    kind: str,
    hash_contents: bool,
) -> ReleaseTreeEntrySnapshot:
    """Snapshot one stable private release entry without following aliases."""

    if kind == "directory":
        before = _release_move_directory_metadata(path)
        content_sha256 = None
    else:
        before = _release_move_file_metadata(
            path, KAGEMUSHA_ARTIFACT_MAX_BYTES
        )
        content_sha256 = None
        if hash_contents:
            size, content_sha256 = stream_private_file_sha256(
                path, KAGEMUSHA_ARTIFACT_MAX_BYTES
            )
            if size != before.st_size:
                fail(f"release-move file changed while it was hashed: {path}")
    if kind == "directory":
        after = _release_move_directory_metadata(path)
    else:
        after = _release_move_file_metadata(
            path, KAGEMUSHA_ARTIFACT_MAX_BYTES
        )
    if _release_move_metadata_identity(after) != _release_move_metadata_identity(
        before
    ):
        fail(f"release-move entry changed while it was inspected: {path}")
    relative = "." if path == root else path.relative_to(root).as_posix()
    return ReleaseTreeEntrySnapshot(
        relative_path=relative,
        kind=kind,
        device=after.st_dev,
        inode=after.st_ino,
        mode=after.st_mode,
        uid=after.st_uid,
        gid=after.st_gid,
        link_count=after.st_nlink,
        size=after.st_size,
        modification_time_ns=after.st_mtime_ns,
        content_sha256=content_sha256,
    )


def _exact_release_tree_paths(
    root: Path,
) -> tuple[tuple[Path, ...], tuple[Path, ...]]:
    """Return the exact directory and file inventory for one release tree."""

    _release_move_directory_metadata(root)
    root_entries = sorted(root.iterdir(), key=lambda path: path.name)
    if [path.name for path in root_entries] != [
        RELEASE_CATALOG_DIRECTORY_NAME,
        RELEASE_POLICY_FILE_NAME,
    ]:
        fail(
            "release-move bundle must contain only its catalog and release policy"
        )
    catalog = root / RELEASE_CATALOG_DIRECTORY_NAME
    policy = root / RELEASE_POLICY_FILE_NAME
    _release_move_directory_metadata(catalog)
    _release_move_file_metadata(policy, 64 * 1024)
    catalog_entries = sorted(catalog.iterdir(), key=lambda path: path.name)
    if len(catalog_entries) != 1:
        fail("release-move catalog must contain exactly one release directory")
    release = catalog_entries[0]
    if SHA256_RE.fullmatch(release.name) is None:
        fail("release-move catalog directory must be one manifest SHA-256")
    _release_move_directory_metadata(release)
    release_entries = sorted(release.iterdir(), key=lambda path: path.name)
    if len(release_entries) != EXPECTED_RELEASE_FILE_COUNT:
        fail(
            "release-move catalog directory must contain exactly "
            f"{EXPECTED_RELEASE_FILE_COUNT} files"
        )
    for path in release_entries:
        _release_move_file_metadata(path, KAGEMUSHA_ARTIFACT_MAX_BYTES)
    return (root, catalog, release), (policy, *release_entries)


def release_bundle_snapshot(
    root: Path, *, hash_contents: bool = True
) -> ReleaseBundleSnapshot:
    """Snapshot one canonical, exact, alias-free private release tree."""

    if not root.is_absolute() or root.resolve(strict=True) != root:
        fail("release-move bundle must be an absolute canonical path")
    directories, files = _exact_release_tree_paths(root)
    entries = tuple(
        sorted(
            (
                *(
                    _release_tree_entry_snapshot(
                        root,
                        path,
                        kind="directory",
                        hash_contents=False,
                    )
                    for path in directories
                ),
                *(
                    _release_tree_entry_snapshot(
                        root,
                        path,
                        kind="file",
                        hash_contents=hash_contents,
                    )
                    for path in files
                ),
            ),
            key=lambda entry: entry.relative_path,
        )
    )
    # Re-list after hashing so concurrent additions, removals, or renames fail
    # closed instead of escaping the pre-move identity.
    final_directories, final_files = _exact_release_tree_paths(root)
    if tuple(path.relative_to(root) for path in final_directories) != tuple(
        path.relative_to(root) for path in directories
    ) or tuple(path.relative_to(root) for path in final_files) != tuple(
        path.relative_to(root) for path in files
    ):
        fail("release-move inventory changed while it was snapshotted")
    digest = hashlib.sha256()
    for entry in entries:
        digest.update(
            json.dumps(
                (
                    *entry.metadata_identity(),
                    entry.content_sha256,
                ),
                ensure_ascii=True,
                separators=(",", ":"),
            ).encode("ascii")
        )
        digest.update(b"\0")
    return ReleaseBundleSnapshot(entries=entries, tree_sha256=digest.hexdigest())


def _assert_release_bundle_snapshot(
    root: Path,
    expected: ReleaseBundleSnapshot,
    *,
    hash_contents: bool,
) -> None:
    """Require a release tree to retain one exact pre-move identity."""

    actual = release_bundle_snapshot(root, hash_contents=hash_contents)
    if tuple(entry.metadata_identity() for entry in actual.entries) != tuple(
        entry.metadata_identity() for entry in expected.entries
    ):
        fail("release bundle metadata does not match its pre-move snapshot")
    if hash_contents and actual != expected:
        fail("release bundle content does not match its pre-move snapshot")


def _require_absent_path(path: Path, label: str) -> None:
    """Require a path to be absent, including as a broken symlink."""

    try:
        path.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise RuntimeError(f"cannot inspect {label}: {path}") from error
    fail(f"{label} already exists: {path}")


def _fsync_release_move_directory(path: Path) -> None:
    """Durably persist one release-move parent directory."""

    expected = _release_move_directory_metadata(path, writable=True)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise RuntimeError(
            f"cannot open release-move directory: {path}"
        ) from error
    try:
        if _release_move_metadata_identity(
            os.fstat(descriptor)
        ) != _release_move_metadata_identity(expected):
            fail(f"release-move directory changed before fsync: {path}")
        os.fsync(descriptor)
    except OSError as error:
        raise RuntimeError(f"cannot fsync release-move directory: {path}") from error
    finally:
        os.close(descriptor)


class ReleaseBundleMove:
    """One-copy, atomic, and reversible release materialization transaction."""

    def __init__(
        self,
        *,
        source: Path,
        output: Path,
        snapshot: ReleaseBundleSnapshot,
        binding: dict[str, object],
    ) -> None:
        self.source = source
        self.output = output
        self.destination = output / "kagemusha"
        self.snapshot = snapshot
        self.binding = binding
        self.moved = False

    @classmethod
    def preflight(cls, source: Path, output: Path) -> "ReleaseBundleMove":
        """Validate and snapshot a release before the reset skeleton exists."""

        if not source.is_absolute() or source.resolve(strict=True) != source:
            fail("release-move bundle must be an absolute canonical path")
        source_metadata = _release_move_directory_metadata(source)
        source_parent = source.parent
        if source_parent.resolve(strict=True) != source_parent:
            fail("release-move source parent must be canonical")
        source_parent_metadata = _release_move_directory_metadata(
            source_parent, writable=True
        )
        if not output.is_absolute():
            fail("output bundle must be absolute")
        _require_absent_path(output, "output bundle")
        if output.parent.resolve(strict=True) != output.parent:
            fail("output parent must be canonical")
        output_parent_metadata = _release_move_directory_metadata(
            output.parent, writable=True
        )
        if source in output.parents or output in source.parents:
            fail("release source and output may not alias or contain one another")
        if (
            source_metadata.st_dev,
            source_metadata.st_ino,
        ) == (
            output_parent_metadata.st_dev,
            output_parent_metadata.st_ino,
        ):
            fail("release source may not alias the output parent")
        if len(
            {
                source_metadata.st_dev,
                source_parent_metadata.st_dev,
                output_parent_metadata.st_dev,
            }
        ) != 1:
            fail(
                "release source and output must be on the same device for "
                "an atomic move"
            )
        snapshot = release_bundle_snapshot(source)
        binding = release_bundle_binding(source)
        _assert_release_bundle_snapshot(
            source, snapshot, hash_contents=False
        )
        return cls(
            source=source,
            output=output,
            snapshot=snapshot,
            binding=binding,
        )

    def move_into_output(self) -> str:
        """Atomically move, fsync, and verify the release in the output."""

        if self.moved:
            fail("release bundle has already been moved")
        if self.output.resolve(strict=True) != self.output:
            fail("materialized output bundle must be canonical")
        output_metadata = _release_move_directory_metadata(
            self.output, writable=True
        )
        if output_metadata.st_dev != self.snapshot.device:
            fail(
                "release source and output must be on the same device for "
                "an atomic move"
            )
        _require_absent_path(
            self.destination, "destination Kagemusha directory"
        )
        _assert_release_bundle_snapshot(
            self.source, self.snapshot, hash_contents=False
        )
        # Treat an interrupted rename as moved unless the complete source
        # identity is still present and the destination is absent.  This keeps
        # cleanup fail-closed even if a signal arrives between the syscall and
        # the following Python bytecode.
        self.moved = True
        try:
            os.rename(self.source, self.destination)
        except BaseException:
            try:
                _assert_release_bundle_snapshot(
                    self.source, self.snapshot, hash_contents=False
                )
                _require_absent_path(
                    self.destination, "destination Kagemusha directory"
                )
            except BaseException:
                pass
            else:
                self.moved = False
            raise
        _fsync_release_move_directory(self.source.parent)
        _fsync_release_move_directory(self.destination.parent)
        _assert_release_bundle_snapshot(
            self.destination, self.snapshot, hash_contents=True
        )
        manifest_sha256 = self.binding.get("manifest_sha256")
        if not isinstance(manifest_sha256, str):
            fail("authenticated release binding lacks its manifest identity")
        return require_sha256(
            manifest_sha256, "authenticated release manifest SHA-256"
        )

    def restore(self) -> None:
        """Atomically restore a moved release to its exact source identity."""

        if not self.moved:
            return
        _require_absent_path(self.source, "release rollback source")
        _assert_release_bundle_snapshot(
            self.destination, self.snapshot, hash_contents=False
        )
        os.rename(self.destination, self.source)
        self.moved = False
        _fsync_release_move_directory(self.destination.parent)
        _fsync_release_move_directory(self.source.parent)
        _assert_release_bundle_snapshot(
            self.source, self.snapshot, hash_contents=True
        )


def decode_json_object(raw: bytes, label: str) -> dict[str, object]:
    """Decode one UTF-8 JSON object while rejecting duplicate members."""

    def reject_duplicate_keys(
        pairs: list[tuple[str, object]],
    ) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                fail(f"{label} contains duplicate JSON member {key!r}")
            result[key] = value
        return result

    try:
        payload = json.loads(raw, object_pairs_hook=reject_duplicate_keys)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"{label} is not canonical JSON") from error
    if not isinstance(payload, dict):
        fail(f"{label} must be a JSON object")
    return payload


def is_positive_integer(value: object) -> bool:
    """Return whether a JSON value is one positive, non-boolean integer."""

    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def is_nonzero_sha256(value: object) -> bool:
    """Return whether a JSON value is one canonical non-zero SHA-256."""

    return (
        isinstance(value, str)
        and SHA256_RE.fullmatch(value) is not None
        and set(value) != {"0"}
    )


def fixed_sha256_hex(value: object, label: str) -> str:
    """Decode the canonical Norito-JSON `[u8; 32]` representation."""

    if (
        not isinstance(value, list)
        or len(value) != 32
        or any(
            not isinstance(byte, int)
            or isinstance(byte, bool)
            or byte < 0
            or byte > 255
            for byte in value
        )
    ):
        fail(f"{label} must be one canonical JSON [u8; 32]")
    return bytes(value).hex()


def crc64_xz(payload: bytes) -> int:
    """Return the CRC64-XZ checksum used by canonical Norito frames."""

    polynomial = 0xC96C5795D7870F42
    table: list[int] = []
    for index in range(256):
        value = index
        for _ in range(8):
            value = (value >> 1) ^ (
                polynomial if value & 1 else 0
            )
        table.append(value)
    checksum = (1 << 64) - 1
    for byte in payload:
        checksum = table[(checksum ^ byte) & 0xFF] ^ (checksum >> 8)
    return checksum ^ ((1 << 64) - 1)


def norito_frame_payload(raw: bytes, label: str) -> tuple[bytes, int]:
    """Validate and unwrap one uncompressed canonical Norito frame."""

    if (
        len(raw) < NORITO_HEADER_SIZE
        or raw[:4] != b"NRT0"
        or raw[4:6] != b"\0\0"
        or raw[22] != 0
    ):
        fail(f"{label} is not an uncompressed Norito V1 frame")
    flags = raw[NORITO_HEADER_SIZE - 1]
    if (
        flags & ~NORITO_SUPPORTED_FLAGS
        or flags & NORITO_FIELD_BITSET
        and flags & (NORITO_COMPACT_LEN | NORITO_PACKED_STRUCT)
        != (NORITO_COMPACT_LEN | NORITO_PACKED_STRUCT)
    ):
        fail(f"{label} advertises unsupported Norito layout flags")
    payload_size = int.from_bytes(raw[23:31], "little")
    checksum = int.from_bytes(raw[31:39], "little")
    padding_size = len(raw) - NORITO_HEADER_SIZE - payload_size
    if (
        padding_size < 0
        or padding_size > 64
        or any(raw[NORITO_HEADER_SIZE : NORITO_HEADER_SIZE + padding_size])
    ):
        fail(f"{label} has invalid Norito payload framing")
    payload = raw[NORITO_HEADER_SIZE + padding_size :]
    if crc64_xz(payload) != checksum:
        fail(f"{label} has an invalid Norito checksum")
    return payload, flags


def norito_length(
    payload: bytes, offset: int, flags: int, label: str
) -> tuple[int, int]:
    """Decode one canonical Norito per-value length prefix."""

    if flags & NORITO_COMPACT_LEN:
        value = 0
        shift = 0
        for index in range(10):
            if offset + index >= len(payload):
                fail(f"{label} contains a truncated Norito length")
            byte = payload[offset + index]
            if index == 9 and byte > 1:
                fail(f"{label} contains an overflowing Norito length")
            value |= (byte & 0x7F) << shift
            if byte & 0x80 == 0:
                if index and value < 1 << (7 * index):
                    fail(f"{label} contains a non-canonical Norito length")
                return value, index + 1
            shift += 7
        fail(f"{label} contains an overflowing Norito length")
    end = offset + 8
    if end > len(payload):
        fail(f"{label} contains a truncated Norito length")
    return int.from_bytes(payload[offset:end], "little"), 8


def norito_struct_fields(
    payload: bytes, count: int, flags: int, label: str
) -> tuple[bytes, ...]:
    """Split one canonical non-packed Norito struct into exact field bytes."""

    if flags & (NORITO_PACKED_STRUCT | NORITO_FIELD_BITSET):
        fail(f"{label} uses an unsupported packed-struct layout")
    fields: list[bytes] = []
    offset = 0
    for _ in range(count):
        size, header_size = norito_length(payload, offset, flags, label)
        start = offset + header_size
        end = start + size
        if end > len(payload):
            fail(f"{label} contains a truncated Norito field")
        fields.append(payload[start:end])
        offset = end
    if offset != len(payload):
        fail(f"{label} contains trailing Norito fields")
    return tuple(fields)


def norito_string(payload: bytes, flags: int, label: str) -> str:
    """Decode one canonical Norito UTF-8 string."""

    size, header_size = norito_length(payload, 0, flags, label)
    if header_size + size != len(payload):
        fail(f"{label} is not one canonical Norito string")
    try:
        return payload[header_size:].decode("utf-8")
    except UnicodeDecodeError as error:
        raise RuntimeError(f"{label} is not UTF-8") from error


def norito_byte_vector(payload: bytes, label: str) -> bytes:
    """Decode one canonical fixed-sequence Norito Vec<u8>."""

    if len(payload) < 8:
        fail(f"{label} is a truncated Norito byte vector")
    size = int.from_bytes(payload[:8], "little")
    if size != len(payload) - 8:
        fail(f"{label} is not one canonical Norito byte vector")
    return payload[8:]


def norito_option_u64(payload: bytes, flags: int, label: str) -> int | None:
    """Decode one canonical Norito Option<u64>."""

    if payload == b"\0":
        return None
    if not payload.startswith(b"\1"):
        fail(f"{label} is not a canonical Norito Option<u64>")
    size, header_size = norito_length(payload, 1, flags, label)
    start = 1 + header_size
    if size != 8 or start + size != len(payload):
        fail(f"{label} is not a canonical Norito Option<u64>")
    return int.from_bytes(payload[start:], "little")


def decode_genesis_instruction(
    encoded: str, label: str
) -> tuple[str, bytes, int]:
    """Decode one base64 canonical InstructionBox used by genesis JSON."""

    try:
        raw = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error) as error:
        raise RuntimeError(f"{label} is not canonical base64") from error
    pair, pair_flags = norito_frame_payload(raw, label)
    name_field, payload_field = norito_struct_fields(
        pair, 2, pair_flags, label
    )
    name = norito_string(name_field, pair_flags, f"{label} type")
    if len(payload_field) < 8:
        fail(f"{label} has a truncated instruction payload")
    frame_size = int.from_bytes(payload_field[:8], "little")
    if frame_size != len(payload_field) - 8:
        fail(f"{label} has an invalid instruction payload length")
    instruction_payload, instruction_flags = norito_frame_payload(
        payload_field[8:], label
    )
    return name, instruction_payload, instruction_flags


def instruction_short_name(name: str) -> str:
    """Return the terminal Rust type component of one instruction wire id."""

    return name.rsplit("::", 1)[-1]


def decode_verifier_id(
    payload: bytes, flags: int, label: str
) -> tuple[str, str]:
    """Decode the two strings in one canonical VerifyingKeyId."""

    backend, name = norito_struct_fields(payload, 2, flags, label)
    return (
        norito_string(backend, flags, f"{label} backend"),
        norito_string(name, flags, f"{label} name"),
    )


def decode_verifier_record(
    identifier: tuple[str, str],
    payload: bytes,
    flags: int,
    label: str,
) -> dict[str, object]:
    """Project one canonical VerifyingKeyRecord into operator identity."""

    fields = norito_struct_fields(payload, 17, flags, label)
    fixed_widths = {
        0: 4,
        6: 32,
        7: 32,
        9: 4,
    }
    if any(len(fields[index]) != size for index, size in fixed_widths.items()):
        fail(f"{label} contains an invalid fixed-width verifier field")
    return {
        "backend": identifier[0],
        "name": identifier[1],
        "version": int.from_bytes(fields[0], "little"),
        "circuit_id": norito_string(
            fields[1], flags, f"{label} circuit id"
        ),
        "commitment": fields[7].hex(),
        "public_inputs_schema_hash": fields[6].hex(),
        "max_proof_bytes": int.from_bytes(fields[9], "little"),
        "activation_height": norito_option_u64(
            fields[13], flags, f"{label} activation height"
        ),
        "withdrawal_height": norito_option_u64(
            fields[14], flags, f"{label} withdrawal height"
        ),
    }


def require_regular_file(
    path: Path, maximum_size: int, *, executable: bool = False
) -> None:
    """Require one stable canonical regular artifact without reading it twice."""

    if not path.is_absolute() or path.resolve(strict=True) != path:
        fail(f"artifact must be an absolute canonical path: {path}")
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_size <= 0
        or metadata.st_size > maximum_size
        or (executable and not stat.S_IMODE(metadata.st_mode) & 0o100)
    ):
        fail(f"invalid regular artifact identity: {path}")


def require_private_key(path: Path) -> str:
    """Read one Ed25519 private key without exposing it."""

    raw = require_private_file(path, 1024)
    try:
        value = raw.decode("ascii").strip()
    except UnicodeDecodeError as error:
        raise RuntimeError("genesis private key is not ASCII") from error
    if GENESIS_PRIVATE_KEY_RE.fullmatch(value) is None:
        fail("genesis private key file does not contain one canonical Ed25519 key")
    return value


def _convert_to_base32(data: bytes) -> list[int]:
    """Convert canonical account bytes into the I105 checksum base."""

    accumulator = 0
    bits = 0
    result: list[int] = []
    for byte in data:
        accumulator = (accumulator << 8) | byte
        bits += 8
        while bits >= 5:
            bits -= 5
            result.append((accumulator >> bits) & 0x1F)
    if bits:
        result.append((accumulator << (5 - bits)) & 0x1F)
    return result


def _bech32_polymod(values: list[int]) -> int:
    """Return the checksum polymod used by canonical I105 account literals."""

    generators = (
        0x3B6A57B2,
        0x26508E6D,
        0x1EA119FA,
        0x3D4233DD,
        0x2A1462B3,
    )
    checksum = 1
    for value in values:
        top = checksum >> 25
        checksum = ((checksum & 0x1FF_FFFF) << 5) ^ value
        for index, generator in enumerate(generators):
            if (top >> index) & 1:
                checksum ^= generator
    return checksum


def _taira_i105_checksum_digits(canonical: bytes) -> list[int]:
    """Return the six canonical I105 checksum digits for an account."""

    values = [ord(character) >> 5 for character in "snx"]
    values.append(0)
    values.extend(ord(character) & 0x1F for character in "snx")
    values.extend(_convert_to_base32(canonical))
    values.extend([0] * TAIRA_I105_CHECKSUM_LEN)
    polymod = _bech32_polymod(values) ^ TAIRA_I105_BECH32M_CONST
    return [
        (
            polymod
            >> (5 * (TAIRA_I105_CHECKSUM_LEN - 1 - index))
        )
        & 0x1F
        for index in range(TAIRA_I105_CHECKSUM_LEN)
    ]


def command_authority_for_genesis_public_key(
    genesis_public_key: str,
) -> str:
    """Derive the canonical Taira I105 account for one Ed25519 public key."""

    if GENESIS_PUBLIC_KEY_RE.fullmatch(genesis_public_key) is None:
        fail("genesis public key is not canonical Ed25519")
    # AccountAddress V1, single-key class, normalisation V1; single-key
    # controller; Ed25519 curve; 32-byte public-key payload.
    canonical = (
        b"\x02\x00\x01\x20"
        + bytes.fromhex(genesis_public_key.removeprefix("ed0120"))
    )
    leading_zeroes = len(canonical) - len(canonical.lstrip(b"\0"))
    value = int.from_bytes(canonical, "big")
    digits: list[int] = []
    while value:
        value, remainder = divmod(value, len(TAIRA_I105_ALPHABET))
        digits.append(remainder)
    encoded_digits = [0] * leading_zeroes + list(reversed(digits))
    if not encoded_digits:
        encoded_digits = [0]
    return "test" + "".join(
        TAIRA_I105_ALPHABET[digit]
        for digit in (
            *encoded_digits,
            *_taira_i105_checksum_digits(canonical),
        )
    )


def require_command_authority(
    value: object,
    *,
    genesis_public_key: str,
) -> str:
    """Require the explicit pin to equal the fresh genesis-key authority."""

    expected = command_authority_for_genesis_public_key(genesis_public_key)
    if not isinstance(value, str) or value != expected:
        fail(
            "command authority must equal the canonical Taira I105 account "
            "derived from the fresh genesis public key"
        )
    return value


def source_command_private_key_sha256(source_bundle: Path) -> bytes:
    """Hash the archived command key without returning or logging its value."""

    source = parse_config(source_bundle / "validator-secrets.toml")
    shared = source.get("shared")
    if not isinstance(shared, dict):
        fail("source bundle lacks the shared runtime secrets table")
    private_key = shared.get("kagemusha_commands_private_key")
    if (
        not isinstance(private_key, str)
        or GENESIS_PRIVATE_KEY_RE.fullmatch(private_key) is None
    ):
        fail("source bundle lacks one canonical Kagemusha command key")
    return hashlib.sha256(private_key.encode("ascii")).digest()


def require_rotated_command_key_projection(
    bundle: Path,
    *,
    command_private_key: str,
    previous_private_key_sha256: bytes,
) -> None:
    """Require the fresh key in every deployable projection and nowhere stale."""

    fresh_sha256 = hashlib.sha256(command_private_key.encode("ascii")).digest()
    if fresh_sha256 == previous_private_key_sha256:
        fail("fresh reset must rotate the archived Kagemusha command key")

    projections: tuple[tuple[Path, tuple[str, ...]], ...] = (
        (
            bundle / "base-config.toml",
            ("torii", "kagemusha_commands", "private_key"),
        ),
        (
            bundle / "validator-secrets.toml",
            ("shared", "kagemusha_commands_private_key"),
        ),
        *(
            (
                bundle / "rendered" / slug / "config.toml",
                ("torii", "kagemusha_commands", "private_key"),
            )
            for slug in VALIDATOR_SLUGS
        ),
    )
    for path, keys in projections:
        current: object = parse_config(path)
        for key in keys:
            if not isinstance(current, dict):
                fail("fresh reset command-key projection is incomplete")
            current = current.get(key)
        if (
            not isinstance(current, str)
            or GENESIS_PRIVATE_KEY_RE.fullmatch(current) is None
            or hashlib.sha256(current.encode("ascii")).digest()
            == previous_private_key_sha256
            or current != command_private_key
        ):
            fail("fresh reset command-key projection is stale or inconsistent")


def quote_toml(value: str) -> str:
    """Render one TOML basic string."""

    return json.dumps(value, ensure_ascii=False)


def qualification_seal_path(release_tree_sha256: str) -> Path:
    """Return the sole root-trusted seal path for one release-tree identity."""

    release_tree_sha256 = require_sha256(
        release_tree_sha256, "Kagemusha release tree SHA-256"
    )
    return (
        TAIRA_QUALIFICATION_SEAL_ROOT
        / f"kagemusha-v4-{release_tree_sha256}.norito"
    )


def replace_top_level_assignment(text: str, key: str, value: str) -> str:
    """Replace exactly one assignment before the first TOML table."""

    lines = text.splitlines()
    matches: list[int] = []
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("["):
            break
        if not stripped.startswith("#") and stripped.partition("=")[0].strip() == key:
            matches.append(index)
    if len(matches) != 1:
        fail(f"config must contain exactly one top-level `{key}` assignment")
    lines[matches[0]] = f"{key} = {value}"
    return "\n".join(lines) + "\n"


def replace_or_insert_section(
    text: str,
    section: str,
    body_lines: list[str],
    *,
    insert_before: str | None = None,
    append_if_insert_target_missing: bool = False,
) -> str:
    """Replace one exact TOML table or insert it before another exact table."""

    lines = text.splitlines()
    header = f"[{section}]"
    starts = [index for index, line in enumerate(lines) if line.strip() == header]
    replacement = [header, *body_lines, ""]
    if len(starts) > 1:
        fail(f"config contains duplicate [{section}] tables")
    if starts:
        start = starts[0]
        end = len(lines)
        for index in range(start + 1, len(lines)):
            if lines[index].strip().startswith("["):
                end = index
                break
        lines[start:end] = replacement
    else:
        if insert_before is None:
            fail(f"config is missing required [{section}] table")
        target = f"[{insert_before}]"
        targets = [
            index for index, line in enumerate(lines) if line.strip() == target
        ]
        if not targets and append_if_insert_target_missing:
            lines.extend(["", *replacement])
        elif len(targets) != 1:
            fail(
                f"config cannot insert [{section}] before non-unique [{insert_before}]"
            )
        else:
            lines[targets[0] : targets[0]] = replacement
    return "\n".join(lines).rstrip() + "\n"


def replace_section_assignment(
    text: str,
    section: str,
    key: str,
    value: str,
    *,
    insert_if_missing: bool = False,
) -> str:
    """Replace one assignment inside one exact TOML table."""

    lines = text.splitlines()
    header = f"[{section}]"
    starts = [index for index, line in enumerate(lines) if line.strip() == header]
    if len(starts) != 1:
        fail(f"config must contain exactly one [{section}] table")
    start = starts[0]
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if lines[index].strip().startswith("["):
            end = index
            break
    matches = [
        index
        for index in range(start + 1, end)
        if not lines[index].strip().startswith("#")
        and lines[index].strip().partition("=")[0].strip() == key
    ]
    if len(matches) > 1:
        fail(f"config contains duplicate [{section}].{key} assignments")
    rendered = f"{key} = {value}"
    if matches:
        lines[matches[0]] = rendered
    elif insert_if_missing:
        lines.insert(end, rendered)
    else:
        fail(f"config is missing [{section}].{key}")
    return "\n".join(lines).rstrip() + "\n"


def remove_section_assignment(text: str, section: str, key: str) -> str:
    """Remove exactly one assignment from one exact TOML table."""

    lines = text.splitlines()
    header = f"[{section}]"
    starts = [index for index, line in enumerate(lines) if line.strip() == header]
    if len(starts) != 1:
        fail(f"config must contain exactly one [{section}] table")
    start = starts[0]
    end = len(lines)
    for index in range(start + 1, len(lines)):
        if lines[index].strip().startswith("["):
            end = index
            break
    matches = [
        index
        for index in range(start + 1, end)
        if not lines[index].strip().startswith("#")
        and lines[index].strip().partition("=")[0].strip() == key
    ]
    if len(matches) != 1:
        fail(f"config must contain exactly one [{section}].{key} assignment")
    del lines[matches[0]]
    return "\n".join(lines).rstrip() + "\n"


def replace_or_insert_section_assignment(
    text: str,
    section: str,
    key: str,
    value: str,
    *,
    insert_before: str | None = None,
) -> str:
    """Replace one table assignment or create the table when it is absent."""

    lines = text.splitlines()
    header = f"[{section}]"
    starts = [index for index, line in enumerate(lines) if line.strip() == header]
    if len(starts) > 1:
        fail(f"config contains duplicate [{section}] tables")
    if starts:
        start = starts[0]
        end = len(lines)
        for index in range(start + 1, len(lines)):
            if lines[index].strip().startswith("["):
                end = index
                break
        matches = [
            index
            for index in range(start + 1, end)
            if not lines[index].strip().startswith("#")
            and lines[index].strip().partition("=")[0].strip() == key
        ]
        if len(matches) > 1:
            fail(f"config contains duplicate [{section}].{key} assignments")
        rendered = f"{key} = {value}"
        if matches:
            lines[matches[0]] = rendered
        else:
            lines.insert(end, rendered)
        return "\n".join(lines).rstrip() + "\n"

    replacement = [header, f"{key} = {value}", ""]
    if insert_before is None:
        lines.extend(["", *replacement])
    else:
        target = f"[{insert_before}]"
        targets = [
            index for index, line in enumerate(lines) if line.strip() == target
        ]
        if len(targets) > 1:
            fail(
                f"config cannot insert [{section}] before duplicate "
                f"[{insert_before}] tables"
            )
        if targets:
            lines[targets[0] : targets[0]] = replacement
        else:
            lines.extend(["", *replacement])
    return "\n".join(lines).rstrip() + "\n"


def runtime_config_text(
    source: str,
    *,
    bundle: Path,
    release_tree_sha256: str,
    genesis_public_key: str,
    genesis_expected_hash: str,
    command_private_key: str,
) -> str:
    """Render one self-contained mandatory-offline validator config."""

    release_tree_sha256 = require_sha256(
        release_tree_sha256, "Kagemusha release tree SHA-256"
    )
    release_root = TAIRA_RELEASE_INSTALL_ROOT / release_tree_sha256
    policy = release_root / RELEASE_POLICY_FILE_NAME
    catalog = release_root / RELEASE_CATALOG_DIRECTORY_NAME
    catalog_seal = qualification_seal_path(release_tree_sha256)
    signed_genesis = bundle / "genesis.signed.nrt"
    text = replace_top_level_assignment(
        source, "chain", quote_toml(PUBLIC_TAIRA_CHAIN_ID)
    )
    text = replace_top_level_assignment(
        text, "chain_discriminant", str(PUBLIC_TAIRA_CHAIN_DISCRIMINANT)
    )
    text = replace_or_insert_section_assignment(
        text,
        "nexus.storage",
        "local_budget_bytes",
        str(PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES),
        insert_before="nexus.registry",
    )
    text = replace_or_insert_section(
        text,
        "nexus.storage.disk_budget_weights",
        [
            f"{key} = {value}"
            for key, value in PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS.items()
        ],
        insert_before="nexus.registry",
        append_if_insert_target_missing=True,
    )
    text = replace_or_insert_section(
        text,
        "torii.kagemusha_commands",
        [
            "enabled = true",
            f"private_key = {quote_toml(command_private_key)}",
            'minimum_xor_balance = "1"',
            'max_tx_value = "1000000000"',
            "operation_registry_max_entries = 4096",
            "operation_registry_max_bytes = 524288",
        ],
        insert_before="settlement.offline",
    )
    text = replace_or_insert_section(
        text,
        "settlement.offline",
        [
            "enabled = true",
            "escrow_required = true",
            (
                "escrow_accounts = { "
                f"{quote_toml(PUBLIC_TAIRA_OFFLINE_ASSET_ID)} = "
                f"{quote_toml(PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT)} }}"
            ),
            f"kagemusha_release_policy_path = {quote_toml(str(policy))}",
            f"kagemusha_artifact_dir = {quote_toml(str(catalog))}",
            (
                "kagemusha_catalog_qualification_seal_path = "
                f"{quote_toml(str(catalog_seal))}"
            ),
            "kagemusha_max_decoded_bytes = 268435456",
        ],
    )
    text = replace_section_assignment(
        text,
        "genesis",
        "public_key",
        quote_toml(genesis_public_key),
    )
    text = replace_section_assignment(
        text,
        "genesis",
        "file",
        quote_toml(str(signed_genesis)),
        insert_if_missing=True,
    )
    if genesis_expected_hash != GENESIS_EXPECTED_HASH_PLACEHOLDER:
        require_genesis_expected_hash(genesis_expected_hash)
    text = replace_section_assignment(
        text,
        "genesis",
        "expected_hash",
        quote_toml(genesis_expected_hash),
        insert_if_missing=True,
    )
    return text


def base_config_text(
    source: str,
    *,
    bundle: Path,
    release_tree_sha256: str,
    genesis_public_key: str,
    genesis_expected_hash: str,
    command_private_key: str,
) -> str:
    """Render the bundle's sealed base config with the same runtime identity."""

    return runtime_config_text(
        source,
        bundle=bundle,
        release_tree_sha256=release_tree_sha256,
        genesis_public_key=genesis_public_key,
        genesis_expected_hash=genesis_expected_hash,
        command_private_key=command_private_key,
    )


def bind_runtime_genesis_expected_hash(source: str, expected_hash: str) -> str:
    """Replace only the explicit pre-signing placeholder with Kagami's hash."""

    expected_hash = require_genesis_expected_hash(expected_hash)
    try:
        payload = tomllib.loads(source)
    except tomllib.TOMLDecodeError as error:
        raise RuntimeError("staged validator config is invalid TOML") from error
    genesis = payload.get("genesis")
    if not isinstance(genesis, dict):
        fail("staged validator config lacks a genesis table")
    current = genesis.get("expected_hash")
    if current not in (GENESIS_EXPECTED_HASH_PLACEHOLDER, expected_hash):
        fail("staged validator config has an unexpected genesis trust root")
    return replace_section_assignment(
        source,
        "genesis",
        "expected_hash",
        quote_toml(expected_hash),
    )


def patch_runtime_secrets(
    source: str, *, command_private_key: str
) -> str:
    """Keep sealed renderer inputs consistent with the runtime config."""

    updates: tuple[tuple[str, str], ...] = (
        ("kagemusha_commands_private_key", quote_toml(command_private_key)),
        ("offline_asset_alias", quote_toml(PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS)),
        ("offline_asset_definition_id", quote_toml(PUBLIC_TAIRA_OFFLINE_ASSET_ID)),
        ("offline_asset_scale", str(PUBLIC_TAIRA_OFFLINE_ASSET_SCALE)),
        (
            "offline_escrow_account",
            quote_toml(PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT),
        ),
    )
    text = source
    for key, value in updates:
        text = replace_section_assignment(text, "shared", key, value)
    return text


def release_bundle_binding(bundle: Path) -> dict[str, object]:
    """Authenticate the raw hash projection of one single-release catalog."""

    if not bundle.is_absolute() or bundle.resolve(strict=True) != bundle:
        fail("release bundle must be an absolute canonical path")
    require_private_directory(bundle)
    _, policy_sha256 = stream_private_file_sha256(
        bundle / RELEASE_POLICY_FILE_NAME, 64 * 1024
    )
    catalog = bundle / RELEASE_CATALOG_DIRECTORY_NAME
    require_private_directory(catalog)
    releases = sorted(catalog.iterdir(), key=lambda path: path.name)
    if len(releases) != 1:
        fail("release catalog must contain exactly one release directory")
    release = releases[0]
    if SHA256_RE.fullmatch(release.name) is None:
        fail("release catalog directory must be one lowercase manifest SHA-256")
    require_private_directory(release)
    entries = sorted(release.iterdir(), key=lambda path: path.name)
    if len(entries) != EXPECTED_RELEASE_FILE_COUNT or any(
        path.is_dir() for path in entries
    ):
        fail(
            "release catalog directory must contain exactly "
            f"{EXPECTED_RELEASE_FILE_COUNT} files"
        )
    for path in entries:
        require_private_file_metadata(path, KAGEMUSHA_ARTIFACT_MAX_BYTES)

    manifest_norito = require_private_file(
        release / "manifest.norito", 1024 * 1024
    )
    manifest_sha256 = hashlib.sha256(manifest_norito).hexdigest()
    if manifest_sha256 != release.name:
        fail("release manifest.norito digest does not match its catalog directory")
    if require_private_file(release / "manifest.norito.sha256", 65) != (
        f"{manifest_sha256}\n".encode()
    ):
        fail("release manifest digest file does not match manifest.norito")
    manifest_payload, _ = norito_frame_payload(
        manifest_norito, "release manifest.norito"
    )

    manifest = decode_json_object(
        require_private_file(release / "manifest.json", 1024 * 1024),
        "release manifest.json",
    )
    if (
        manifest.get("chain_id") != PUBLIC_TAIRA_CHAIN_ID
        or manifest.get("asset") != PUBLIC_TAIRA_OFFLINE_ASSET_ID
        or manifest.get("asset_scale") != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
        or manifest.get("activation_height")
        != PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT
        or manifest.get("generation") != KAGEMUSHA_RELEASE_GENERATION
        or manifest.get("bridge_abi_version") != KAGEMUSHA_BRIDGE_ABI_VERSION
        or not is_positive_integer(manifest.get("withdrawal_height"))
        or manifest["withdrawal_height"]
        < PUBLIC_TAIRA_RELEASE_MINIMUM_WITHDRAWAL_HEIGHT
        or not is_positive_integer(manifest.get("max_proof_bytes"))
    ):
        fail("release manifest does not target canonical height-2 Taira offline cash")

    profiles = manifest.get("profiles")
    if not isinstance(profiles, list) or len(profiles) != 2:
        fail("release manifest must contain exact Eq and Ep profiles")
    artifacts: list[dict[str, object]] = []
    for profile in profiles:
        if not isinstance(profile, dict):
            fail("release manifest contains an invalid proof profile")
        profile_artifacts = profile.get("artifacts")
        if not isinstance(profile_artifacts, list):
            fail("release manifest proof profile lacks artifacts")
        for artifact in profile_artifacts:
            if not isinstance(artifact, dict):
                fail("release manifest contains an invalid artifact descriptor")
            artifacts.append(artifact)
    artifact_kinds = tuple(artifact.get("kind") for artifact in artifacts)
    if (
        artifact_kinds
        != tuple(
            {"kind": label, "value": None}
            for label in KAGEMUSHA_ARTIFACT_KIND_LABELS
        )
        or tuple(artifact.get("file_name") for artifact in artifacts)
        != KAGEMUSHA_ARTIFACT_FILE_NAMES
    ):
        fail("release manifest does not contain the exact ordered eight-role inventory")

    expected_names = {
        "manifest.norito",
        "manifest.json",
        "manifest.norito.sha256",
        RELEASE_ATTESTATION_FILE_NAME,
        BENCHMARK_EVIDENCE_FILE_NAME,
        CRYPTOGRAPHIC_REVIEW_FILE_NAME,
        PROMOTION_RECORD_FILE_NAME,
    }
    for artifact in artifacts:
        name = artifact["file_name"]
        assert isinstance(name, str)
        expected_names.add(name)
        path = release / name
        artifact_size, artifact_sha256 = stream_private_file_sha256(
            path, KAGEMUSHA_ARTIFACT_MAX_BYTES
        )
        if (
            artifact.get("size_bytes") != artifact_size
            or fixed_sha256_hex(
                artifact.get("sha256"),
                f"release artifact `{name}` digest",
            )
            != artifact_sha256
        ):
            fail(f"release artifact `{name}` does not match its manifest descriptor")

    roster = manifest.get("topup_finality_roster_artifact")
    if not isinstance(roster, dict):
        fail("release manifest lacks the top-up finality roster")
    roster_name = roster.get("file_name")
    if roster_name != "topup-finality-roster-v4.norito":
        fail("release manifest has the wrong top-up finality roster file")
    expected_names.add(roster_name)
    roster_size, roster_sha256 = stream_private_file_sha256(
        release / roster_name, 2 * 1024 * 1024
    )
    if (
        roster.get("size_bytes") != roster_size
        or fixed_sha256_hex(
            roster.get("sha256"), "release top-up finality roster digest"
        )
        != roster_sha256
        or roster.get("artifact_generation") != KAGEMUSHA_RELEASE_GENERATION
        or roster.get("required_bridge_abi_version")
        != KAGEMUSHA_BRIDGE_ABI_VERSION
    ):
        fail("release top-up finality roster does not match its manifest descriptor")
    if {path.name for path in entries} != expected_names:
        fail("release catalog file inventory is not exact")

    attestation_raw = require_private_file(
        release / RELEASE_ATTESTATION_FILE_NAME, 2 * 1024 * 1024
    )
    attestation_sha256 = hashlib.sha256(attestation_raw).hexdigest()
    attestation_payload, _ = norito_frame_payload(
        attestation_raw, "release attestation"
    )
    _, benchmark_sha256 = stream_private_file_sha256(
        release / BENCHMARK_EVIDENCE_FILE_NAME, 2 * 1024 * 1024
    )
    _, review_sha256 = stream_private_file_sha256(
        release / CRYPTOGRAPHIC_REVIEW_FILE_NAME, 2 * 1024 * 1024
    )
    if (
        fixed_sha256_hex(
            manifest.get("release_attestation_sha256"),
            "release attestation digest",
        )
        != attestation_sha256
        or fixed_sha256_hex(
            manifest.get("benchmark_evidence_sha256"),
            "release benchmark evidence digest",
        )
        != benchmark_sha256
        or fixed_sha256_hex(
            manifest.get("cryptographic_review_sha256"),
            "release cryptographic review digest",
        )
        != review_sha256
    ):
        fail("release evidence hashes do not match the manifest")
    promotion_raw = require_private_file(
        release / PROMOTION_RECORD_FILE_NAME, 2 * 1024 * 1024
    )
    promotion_payload, _ = norito_frame_payload(
        promotion_raw, "release promotion record"
    )
    return {
        "manifest": manifest,
        "manifest_sha256": manifest_sha256,
        "manifest_payload_sha256": hashlib.sha256(
            manifest_payload
        ).hexdigest(),
        "release_policy_sha256": policy_sha256,
        "release_attestation_sha256": attestation_sha256,
        "release_attestation_payload_sha256": hashlib.sha256(
            attestation_payload
        ).hexdigest(),
        "benchmark_evidence_sha256": benchmark_sha256,
        "cryptographic_review_sha256": review_sha256,
        "promotion_record_sha256": hashlib.sha256(promotion_raw).hexdigest(),
        "promotion_record_payload_sha256": hashlib.sha256(
            promotion_payload
        ).hexdigest(),
    }


def operator_identity_binding(
    raw: bytes, release: dict[str, object]
) -> dict[str, object]:
    """Validate the external operator identity against one exact release."""

    identity = decode_json_object(raw, "operator release identity")
    expected_keys = {
        "cash_handoff_capability",
        "required_bridge_abi_version",
        "max_hops",
        "asset_definition_id",
        "asset_scale",
        "artifact_set",
        "verifiers",
    }
    if (
        set(identity) != expected_keys
        or identity.get("cash_handoff_capability") != "cash_handoff_v1"
        or identity.get("required_bridge_abi_version")
        != KAGEMUSHA_BRIDGE_ABI_VERSION
        or identity.get("max_hops") != KAGEMUSHA_MAX_HOPS
        or identity.get("asset_definition_id") != PUBLIC_TAIRA_OFFLINE_ASSET_ID
        or identity.get("asset_scale") != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
    ):
        fail("operator release identity has the wrong Taira capability projection")
    artifact = identity.get("artifact_set")
    artifact_keys = {
        "generation",
        "manifest_sha256",
        "release_policy_sha256",
        "release_attestation_sha256",
        "activation_height",
        "withdrawal_height",
        "max_proof_bytes",
        "asset_scale",
    }
    manifest = release["manifest"]
    assert isinstance(manifest, dict)
    if (
        not isinstance(artifact, dict)
        or set(artifact) != artifact_keys
        or artifact.get("generation") != KAGEMUSHA_RELEASE_GENERATION
        or artifact.get("manifest_sha256") != release["manifest_sha256"]
        or artifact.get("release_policy_sha256")
        != release["release_policy_sha256"]
        or artifact.get("release_attestation_sha256")
        != release["release_attestation_sha256"]
        or artifact.get("activation_height")
        != PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT
        or artifact.get("withdrawal_height") != manifest["withdrawal_height"]
        or artifact.get("max_proof_bytes") != manifest["max_proof_bytes"]
        or artifact.get("asset_scale") != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
    ):
        fail("operator release identity is not bound to the exact release catalog")
    digests = (
        artifact["manifest_sha256"],
        artifact["release_policy_sha256"],
        artifact["release_attestation_sha256"],
    )
    if not all(is_nonzero_sha256(value) for value in digests) or len(
        set(digests)
    ) != 3:
        fail("operator release identity contains invalid or repeated digests")

    verifiers = identity.get("verifiers")
    verifier_keys = {
        "backend",
        "name",
        "version",
        "circuit_id",
        "commitment",
        "public_inputs_schema_hash",
        "max_proof_bytes",
        "activation_height",
        "withdrawal_height",
    }
    if not isinstance(verifiers, dict) or set(verifiers) != set(
        KAGEMUSHA_VERIFIER_ROLES
    ):
        fail("operator release identity must contain all five exact verifier roles")
    commitments: set[str] = set()
    schemas: set[str] = set()
    for field, expected_role in KAGEMUSHA_VERIFIER_ROLES.items():
        verifier = verifiers.get(field)
        if (
            not isinstance(verifier, dict)
            or set(verifier) != verifier_keys
            or (
                verifier.get("backend"),
                verifier.get("name"),
                verifier.get("circuit_id"),
            )
            != expected_role
            or verifier.get("version") != 1
            or not is_nonzero_sha256(verifier.get("commitment"))
            or not is_nonzero_sha256(
                verifier.get("public_inputs_schema_hash")
            )
            or not is_positive_integer(verifier.get("max_proof_bytes"))
            or verifier.get("activation_height")
            != PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT
        ):
            fail(f"operator release identity has an invalid `{field}` role")
        if field.startswith("active_recursive_"):
            if (
                verifier.get("withdrawal_height")
                != artifact["withdrawal_height"]
                or verifier.get("max_proof_bytes") != artifact["max_proof_bytes"]
            ):
                fail(f"operator release identity `{field}` is not release-bound")
        elif verifier.get("withdrawal_height") is not None:
            fail(f"operator release identity `{field}` must remain unscheduled")
        commitments.add(verifier["commitment"])
        schemas.add(verifier["public_inputs_schema_hash"])
    if len(commitments) != 5 or len(schemas) != 5:
        fail("operator release identity verifier roles are not distinct")
    return identity


def validate_genesis_release_binding(
    instructions: list[object],
    release: dict[str, object],
    operator_identity: dict[str, object],
) -> None:
    """Bind canonical base64 genesis instructions to release file bytes."""

    decoded: list[tuple[str, bytes, int]] = []
    for index, instruction in enumerate(instructions):
        if isinstance(instruction, str):
            decoded.append(
                decode_genesis_instruction(
                    instruction, f"offline genesis instruction {index}"
                )
            )
        elif isinstance(instruction, dict) and (
            "ActivateKagemushaRecursiveReleaseV4" in instruction
            or "RegisterVerifyingKey" in instruction
        ):
            fail(
                "release-bound genesis instructions must use canonical "
                "base64 Norito encoding"
            )

    activations = [
        (payload, flags)
        for name, payload, flags in decoded
        if instruction_short_name(name)
        == "ActivateKagemushaRecursiveReleaseV4"
    ]
    if len(activations) != 1:
        fail("offline genesis must contain exactly one complete V4 activation")
    activation_payload, activation_flags = activations[0]
    activation, device_policy = norito_struct_fields(
        activation_payload,
        2,
        activation_flags,
        "offline genesis V4 activation instruction",
    )
    if not device_policy:
        fail("offline genesis V4 activation has an empty device policy")
    activation_fields = norito_struct_fields(
        activation,
        6,
        activation_flags,
        "offline genesis V4 activation",
    )
    release_record_fields = norito_struct_fields(
        activation_fields[0],
        5,
        activation_flags,
        "offline genesis V4 release record",
    )
    embedded_release_hashes = (
        hashlib.sha256(release_record_fields[0]).hexdigest(),
        hashlib.sha256(release_record_fields[1]).hexdigest(),
        hashlib.sha256(release_record_fields[4]).hexdigest(),
    )
    expected_release_hashes = (
        release.get("manifest_payload_sha256"),
        release.get("release_attestation_payload_sha256"),
        release.get("promotion_record_payload_sha256"),
    )
    if embedded_release_hashes != expected_release_hashes:
        fail(
            "offline genesis activation release record differs from the "
            "authenticated catalog bytes"
        )
    embedded_evidence_hashes = (
        hashlib.sha256(
            norito_byte_vector(
                release_record_fields[2],
                "offline genesis physical-device evidence",
            )
        ).hexdigest(),
        hashlib.sha256(
            norito_byte_vector(
                release_record_fields[3],
                "offline genesis cryptographic-review evidence",
            )
        ).hexdigest(),
    )
    expected_evidence_hashes = (
        release.get("benchmark_evidence_sha256"),
        release.get("cryptographic_review_sha256"),
    )
    if embedded_evidence_hashes != expected_evidence_hashes:
        fail(
            "offline genesis activation evidence differs from the "
            "authenticated catalog bytes"
        )
    try:
        expected_policy = bytes.fromhex(str(release["release_policy_sha256"]))
    except (KeyError, ValueError) as error:
        raise RuntimeError("release policy identity is invalid") from error
    if activation_fields[1] != expected_policy:
        fail("offline genesis activation uses the wrong release policy")

    expected_verifiers = operator_identity["verifiers"]
    assert isinstance(expected_verifiers, dict)
    for parity, identifier_index, record_index in (
        ("eq", 2, 3),
        ("ep", 4, 5),
    ):
        field = f"active_recursive_step_{parity}_verifier"
        identifier = decode_verifier_id(
            activation_fields[identifier_index],
            activation_flags,
            f"offline genesis recursive {parity} verifier id",
        )
        expected_role = KAGEMUSHA_VERIFIER_ROLES[field]
        if identifier != expected_role[:2]:
            fail(
                f"offline genesis recursive {parity} verifier has the wrong id"
            )
        record = decode_verifier_record(
            identifier,
            activation_fields[record_index],
            activation_flags,
            f"offline genesis recursive {parity} verifier",
        )
        if record != expected_verifiers[field]:
            fail(
                f"offline genesis recursive {parity} verifier differs from "
                "the operator identity"
            )

    base_fields = tuple(KAGEMUSHA_VERIFIER_ROLES)[:3]
    base_records: dict[tuple[str, str], dict[str, object]] = {}
    for name, payload, flags in decoded:
        if instruction_short_name(name) != "RegisterVerifyingKey":
            continue
        identifier_payload, record_payload = norito_struct_fields(
            payload, 2, flags, "offline genesis verifying-key registration"
        )
        identifier = decode_verifier_id(
            identifier_payload,
            flags,
            "offline genesis verifying-key identifier",
        )
        if identifier in base_records:
            fail("offline genesis repeats a verifying-key registration")
        base_records[identifier] = decode_verifier_record(
            identifier,
            record_payload,
            flags,
            "offline genesis verifying-key record",
        )
    expected_base_keys = {
        (
            KAGEMUSHA_VERIFIER_ROLES[field][0],
            KAGEMUSHA_VERIFIER_ROLES[field][1],
        )
        for field in base_fields
    }
    if set(base_records) != expected_base_keys:
        fail("offline genesis does not register the exact three base verifiers")
    for field in base_fields:
        expected_role = KAGEMUSHA_VERIFIER_ROLES[field]
        if (
            base_records[(expected_role[0], expected_role[1])]
            != expected_verifiers[field]
        ):
            fail(f"offline genesis `{field}` differs from the operator identity")


def genesis_summary(
    path: Path,
    *,
    command_authority: str,
    genesis_public_key: str,
    release: dict[str, object] | None = None,
    operator_identity: dict[str, object] | None = None,
) -> dict[str, object]:
    """Decode and validate the fixed public Taira genesis envelope."""

    command_authority = require_command_authority(
        command_authority,
        genesis_public_key=genesis_public_key,
    )
    raw = require_private_file(path, MAX_GENESIS_BYTES)
    payload = decode_json_object(raw, "offline genesis")
    if payload.get("chain") != PUBLIC_TAIRA_CHAIN_ID:
        fail("offline genesis does not target canonical public Taira")
    if payload.get("chain_discriminant") != PUBLIC_TAIRA_CHAIN_DISCRIMINANT:
        fail("offline genesis has the wrong Taira chain discriminant")
    transactions = payload.get("transactions")
    if not isinstance(transactions, list) or not transactions:
        fail("offline genesis has no transactions")
    first = transactions[0]
    if not isinstance(first, dict):
        fail("offline genesis first transaction is invalid")
    parameters = first.get("parameters")
    if not isinstance(parameters, dict):
        fail("offline genesis lacks consensus parameters")
    sumeragi = parameters.get("sumeragi")
    if (
        not isinstance(sumeragi, dict)
        or sumeragi.get("block_cadence_ms") != PUBLIC_TAIRA_BLOCK_CADENCE_MS
    ):
        fail(
            "offline genesis must set transactions[0].parameters.sumeragi."
            f"block_cadence_ms={PUBLIC_TAIRA_BLOCK_CADENCE_MS}"
        )

    instructions: list[object] = []
    for transaction_index, transaction in enumerate(transactions):
        if not isinstance(transaction, dict):
            fail(f"offline genesis transaction {transaction_index} is invalid")
        transaction_instructions = transaction.get("instructions")
        if not isinstance(transaction_instructions, list):
            fail(
                "offline genesis transaction "
                f"{transaction_index} lacks an instructions array"
            )
        for instruction_index, instruction in enumerate(transaction_instructions):
            if not isinstance(instruction, (dict, str)):
                fail(
                    "offline genesis instruction "
                    f"{transaction_index}:{instruction_index} is invalid"
                )
            instructions.append(instruction)

    registrations: list[dict[str, object]] = []
    aliases: list[dict[str, object]] = []
    asset_mints: list[dict[str, object]] = []
    fee_mints: list[dict[str, object]] = []
    explicit_command_authority_registrations = 0
    for instruction in instructions:
        if not isinstance(instruction, dict):
            continue
        register = instruction.get("Register")
        if isinstance(register, dict):
            definition = register.get("AssetDefinition")
            if (
                isinstance(definition, dict)
                and definition.get("id") == PUBLIC_TAIRA_OFFLINE_ASSET_ID
            ):
                registrations.append(definition)
            account = register.get("Account")
            if (
                isinstance(account, dict)
                and account.get("id") == command_authority
            ):
                explicit_command_authority_registrations += 1
        alias = instruction.get("SetAssetDefinitionAlias")
        if isinstance(alias, dict) and (
            alias.get("alias") == PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
            or alias.get("asset_definition_id") == PUBLIC_TAIRA_OFFLINE_ASSET_ID
        ):
            aliases.append(alias)
        mint = instruction.get("Mint")
        if isinstance(mint, dict):
            asset = mint.get("Asset")
            if (
                isinstance(asset, dict)
                and isinstance(asset.get("destination"), str)
            ):
                destination = asset["destination"]
                if destination.startswith(f"{PUBLIC_TAIRA_OFFLINE_ASSET_ID}#"):
                    asset_mints.append(asset)
                if destination.startswith(f"{PUBLIC_TAIRA_FEE_ASSET_ID}#"):
                    fee_mints.append(asset)

    if len(registrations) != 1:
        fail(
            "offline genesis must contain exactly one canonical Taira offline "
            "asset registration"
        )
    registration = registrations[0]
    if (
        registration.get("name") != PUBLIC_TAIRA_OFFLINE_ASSET_NAME
        or registration.get("metadata") != PUBLIC_TAIRA_OFFLINE_ASSET_METADATA
        or not isinstance(registration.get("spec"), dict)
        or registration["spec"].get("scale") != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
    ):
        fail(
            "offline genesis lacks the exact Digital Shekel asset projection "
            "(ds, DS/ILS/₪ metadata, scale 2)"
        )
    if len(aliases) != 1 or aliases[0] != {
        "alias": PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
        "asset_definition_id": PUBLIC_TAIRA_OFFLINE_ASSET_ID,
        "lease_expiry_ms": None,
    }:
        fail(
            "offline genesis must contain exactly one ds#boi.is binding to the "
            "canonical Taira offline asset"
        )
    if explicit_command_authority_registrations:
        fail(
            "offline genesis must not explicitly register the implicit "
            "genesis/command authority"
        )
    expected_fee_destination = (
        f"{PUBLIC_TAIRA_FEE_ASSET_ID}#{command_authority}"
    )
    command_fee_ready = False
    for mint in fee_mints:
        if mint.get("destination") != expected_fee_destination:
            continue
        raw_quantity = mint.get("object")
        if isinstance(raw_quantity, bool) or not isinstance(
            raw_quantity, (str, int, float)
        ):
            continue
        try:
            quantity = Decimal(str(raw_quantity))
            if quantity.is_finite() and quantity > 0:
                command_fee_ready = True
                break
        except InvalidOperation:
            continue
    if not command_fee_ready:
        fail(
            "offline genesis does not fund the explicitly pinned command "
            "authority with the canonical Taira fee asset"
        )
    online_backing_source_ready = False
    for mint in asset_mints:
        destination = mint["destination"]
        assert isinstance(destination, str)
        account = destination.partition("#")[2]
        if account == PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT:
            continue
        raw_quantity = mint.get("object")
        if isinstance(raw_quantity, bool) or not isinstance(
            raw_quantity, (str, int, float)
        ):
            continue
        try:
            quantity = Decimal(str(raw_quantity))
            if quantity.is_finite() and quantity > 0:
                online_backing_source_ready = True
                break
        except InvalidOperation:
            continue
    if not online_backing_source_ready:
        fail(
            "offline genesis has no non-zero non-escrow Digital Shekel "
            "liquidity"
        )

    encoded = raw.decode("utf-8")
    binary_instruction_markers: set[str] = set()
    for index, instruction in enumerate(instructions):
        if isinstance(instruction, str):
            name, _, _ = decode_genesis_instruction(
                instruction, f"offline genesis instruction {index}"
            )
            binary_instruction_markers.add(instruction_short_name(name))
    required_markers = (
        PUBLIC_TAIRA_OFFLINE_ASSET_ID,
        PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
        command_authority,
        "offline.enabled",
        "ActivateKagemushaRecursiveReleaseV4",
        "RegisterVerifyingKey",
        "RegisterZkAsset",
        "CanManageOfflineEscrow",
        "CanActivateKagemushaRecursiveReleaseV4",
        "CanManageOfflineDeviceAttestationPolicy",
    )
    missing = [
        marker
        for marker in required_markers
        if marker not in encoded and marker not in binary_instruction_markers
    ]
    if missing:
        fail(
            "offline genesis lacks mandatory bootstrap markers: "
            + ", ".join(missing)
        )
    if (release is None) != (operator_identity is None):
        fail("genesis release validation requires both bound identities")
    if release is not None and operator_identity is not None:
        validate_genesis_release_binding(
            instructions, release, operator_identity
        )
    return {
        "chain": PUBLIC_TAIRA_CHAIN_ID,
        "chain_discriminant": PUBLIC_TAIRA_CHAIN_DISCRIMINANT,
        "block_cadence_ms": PUBLIC_TAIRA_BLOCK_CADENCE_MS,
        "transaction_count": len(transactions),
        "offline_asset_name": PUBLIC_TAIRA_OFFLINE_ASSET_NAME,
        "offline_asset_metadata": PUBLIC_TAIRA_OFFLINE_ASSET_METADATA,
        "offline_asset_alias": PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
        "command_authority": command_authority,
        "online_backing_source_ready": True,
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


def copy_release_bundle(source: Path, destination: Path) -> str:
    """Copy and validate one authenticated single-release V4 catalog."""

    if source.resolve(strict=True) != source:
        fail("release bundle must be an absolute canonical path")
    require_private_directory(source)
    if destination.exists() or destination.is_symlink():
        fail("destination Kagemusha directory already exists")
    destination.mkdir(mode=0o700)
    policy_source = source / RELEASE_POLICY_FILE_NAME
    copy_private_file_streaming(
        policy_source,
        destination / RELEASE_POLICY_FILE_NAME,
        64 * 1024,
    )
    catalog_source = source / RELEASE_CATALOG_DIRECTORY_NAME
    require_private_directory(catalog_source)
    catalog_entries = sorted(catalog_source.iterdir(), key=lambda path: path.name)
    if len(catalog_entries) != 1:
        fail("release catalog must contain exactly one release directory")
    release_source = catalog_entries[0]
    if SHA256_RE.fullmatch(release_source.name) is None:
        fail("release catalog directory must be one lowercase manifest SHA-256")
    require_private_directory(release_source)
    release_entries = sorted(release_source.iterdir(), key=lambda path: path.name)
    if len(release_entries) != EXPECTED_RELEASE_FILE_COUNT:
        fail(
            "release catalog directory must contain exactly "
            f"{EXPECTED_RELEASE_FILE_COUNT} files"
        )
    if any(path.is_dir() for path in release_entries):
        fail("release catalog may not contain nested directories")
    catalog_destination = destination / RELEASE_CATALOG_DIRECTORY_NAME
    catalog_destination.mkdir(mode=0o700)
    release_destination = catalog_destination / release_source.name
    release_destination.mkdir(mode=0o700)
    for source_file in release_entries:
        # Core admits only direct single-link regular files.  Check that shape
        # before copying as well as producing a new single-link destination.
        copy_private_file_streaming(
            source_file,
            release_destination / source_file.name,
            KAGEMUSHA_ARTIFACT_MAX_BYTES,
        )
    manifest_norito = require_private_file(
        release_destination / "manifest.norito", 1024 * 1024
    )
    actual_manifest_sha256 = hashlib.sha256(manifest_norito).hexdigest()
    if actual_manifest_sha256 != release_source.name:
        fail("release manifest.norito digest does not match its catalog directory")
    digest_file = release_destination / "manifest.norito.sha256"
    digest_payload = require_private_file(digest_file, 65)
    if digest_payload != f"{actual_manifest_sha256}\n".encode():
        fail("release manifest digest file does not match manifest.norito")
    manifest_path = release_destination / "manifest.json"
    manifest_raw = require_private_file(manifest_path, 1024 * 1024)
    try:
        manifest = json.loads(manifest_raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError("release manifest.json is invalid") from error
    if (
        not isinstance(manifest, dict)
        or manifest.get("chain_id") != PUBLIC_TAIRA_CHAIN_ID
        or manifest.get("asset") != PUBLIC_TAIRA_OFFLINE_ASSET_ID
        or manifest.get("asset_scale") != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
        or manifest.get("activation_height")
        != PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT
    ):
        fail("release manifest does not target canonical height-2 Taira offline cash")
    return release_source.name


def release_tree_sha256(root: Path) -> str:
    """Hash a symlink-free release tree including its canonical relative names."""

    digest = hashlib.sha256()
    for path in sorted(root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()):
        relative = path.relative_to(root).as_posix().encode()
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            fail(f"release tree contains a symlink: {path}")
        if stat.S_ISDIR(metadata.st_mode):
            digest.update(b"d\0" + relative + b"\0")
            continue
        size, file_sha256 = stream_private_file_sha256(
            path, KAGEMUSHA_ARTIFACT_MAX_BYTES
        )
        digest.update(b"f\0" + relative + b"\0")
        digest.update(size.to_bytes(8, "big"))
        digest.update(bytes.fromhex(file_sha256))
    return digest.hexdigest()


def run_checked(command: list[str], *, cwd: Path | None = None) -> None:
    """Run one non-interactive preparation command."""

    subprocess.run(
        command,
        cwd=cwd,
        check=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
    )


def parse_config(path: Path) -> dict[str, object]:
    """Read one bounded private TOML config."""

    raw = require_private_file(path, MAX_CONFIG_BYTES)
    try:
        payload = tomllib.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise RuntimeError(f"validator config is invalid TOML: {path}") from error
    if not isinstance(payload, dict):
        fail(f"validator config is not a TOML table: {path}")
    return payload


def validate_runtime_config(
    config: dict[str, object],
    bundle: Path,
    release_tree_sha256: str,
    genesis_public_key: str,
    genesis_expected_hash: str,
) -> None:
    """Enforce the exact mandatory-offline runtime projection."""

    if GENESIS_PUBLIC_KEY_RE.fullmatch(genesis_public_key) is None:
        fail("expected genesis public key is not canonical Ed25519")
    genesis_expected_hash = require_genesis_expected_hash(
        genesis_expected_hash
    )
    if (
        config.get("chain") != PUBLIC_TAIRA_CHAIN_ID
        or config.get("chain_discriminant") != PUBLIC_TAIRA_CHAIN_DISCRIMINANT
    ):
        fail("validator config does not target canonical public Taira")
    torii = config.get("torii")
    settlement = config.get("settlement")
    genesis = config.get("genesis")
    nexus = config.get("nexus")
    if not all(
        isinstance(value, dict) for value in (torii, settlement, genesis, nexus)
    ):
        fail("validator config lacks Torii, settlement, genesis, or Nexus tables")
    assert isinstance(torii, dict)
    assert isinstance(settlement, dict)
    assert isinstance(genesis, dict)
    assert isinstance(nexus, dict)
    nexus_storage = nexus.get("storage")
    if (
        not isinstance(nexus_storage, dict)
        or nexus_storage.get("local_budget_bytes")
        != PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES
        or nexus_storage.get("disk_budget_weights")
        != PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS
    ):
        fail(
            "validator config lacks the exact bounded Taira storage budget "
            "and component weights"
        )
    commands = torii.get("kagemusha_commands")
    offline = settlement.get("offline")
    if (
        not isinstance(commands, dict)
        or commands.get("enabled") is not True
        or not isinstance(commands.get("private_key"), str)
        or GENESIS_PRIVATE_KEY_RE.fullmatch(commands["private_key"]) is None
        or commands.get("minimum_xor_balance") != "1"
    ):
        fail("validator config lacks the exact Kagemusha command issuer")
    release_tree_sha256 = require_sha256(
        release_tree_sha256, "Kagemusha release tree SHA-256"
    )
    release_root = TAIRA_RELEASE_INSTALL_ROOT / release_tree_sha256
    expected_policy = release_root / RELEASE_POLICY_FILE_NAME
    expected_catalog = release_root / RELEASE_CATALOG_DIRECTORY_NAME
    expected_seal = qualification_seal_path(release_tree_sha256)
    if (
        not isinstance(offline, dict)
        or offline.get("enabled") is not True
        or offline.get("escrow_required") is not True
        or offline.get("escrow_accounts")
        != {
            PUBLIC_TAIRA_OFFLINE_ASSET_ID: PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT
        }
        or offline.get("kagemusha_release_policy_path") != str(expected_policy)
        or offline.get("kagemusha_artifact_dir") != str(expected_catalog)
        or offline.get("kagemusha_catalog_qualification_seal_path")
        != str(expected_seal)
    ):
        fail("validator config lacks the exact mandatory offline settlement projection")
    if (
        genesis.get("file") != str(bundle / "genesis.signed.nrt")
        or genesis.get("public_key") != genesis_public_key
        or genesis.get("expected_hash") != genesis_expected_hash
    ):
        fail(
            "validator config does not use the bundle's signed genesis "
            "with its fresh public key and exact expected hash"
        )


def staged_check_config_text(
    source: str, bundle: Path, release_tree_sha256: str
) -> str:
    """Retarget only a temporary ``--check-config`` copy to staged artifacts."""

    release_tree_sha256 = require_sha256(
        release_tree_sha256, "Kagemusha release tree SHA-256"
    )
    installed_root = TAIRA_RELEASE_INSTALL_ROOT / release_tree_sha256
    expected_policy = str(installed_root / RELEASE_POLICY_FILE_NAME)
    expected_catalog = str(installed_root / RELEASE_CATALOG_DIRECTORY_NAME)
    expected_seal = str(qualification_seal_path(release_tree_sha256))
    if (
        source.count(expected_policy) != 1
        or source.count(expected_catalog) != 1
        or source.count(expected_seal) != 1
    ):
        fail("final runtime config lacks one exact installed release-path binding")
    text = replace_section_assignment(
        source,
        "settlement.offline",
        "kagemusha_release_policy_path",
        quote_toml(str(bundle / "kagemusha" / RELEASE_POLICY_FILE_NAME)),
    )
    text = replace_section_assignment(
        text,
        "settlement.offline",
        "kagemusha_artifact_dir",
        quote_toml(
            str(bundle / "kagemusha" / RELEASE_CATALOG_DIRECTORY_NAME)
        ),
    )
    return remove_section_assignment(
        text,
        "settlement.offline",
        "kagemusha_catalog_qualification_seal_path",
    )


def update_manifest(
    bundle: Path,
    *,
    irohad: Path,
    kagami: Path,
    source_commit: str,
    genesis_public_key: str,
    genesis_expected_hash: str,
    command_authority: str,
    manifest_sha256: str,
    release_attestation_sha256: str,
    release_tree_sha256_value: str,
    operator_identity_sha256: str,
    free_bytes_after_materialization: int,
) -> None:
    """Seal all v21 identities into the reset manifest."""

    command_authority = require_command_authority(
        command_authority,
        genesis_public_key=genesis_public_key,
    )
    genesis_expected_hash = require_genesis_expected_hash(
        genesis_expected_hash
    )
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(
        require_private_file(manifest_path, 1024 * 1024).decode("utf-8")
    )
    if (
        not isinstance(manifest, dict)
        or manifest.get("schema") != MANIFEST_SCHEMA
        or manifest.get("peer_count") != PEER_COUNT
    ):
        fail("source reset manifest identity is invalid")
    manifest.update(
        {
            "source_commit": source_commit,
            "irohad_sha256": sha256(irohad),
            "kagami_sha256": sha256(kagami),
            "genesis_public_key": genesis_public_key,
            "genesis_expected_hash": genesis_expected_hash,
            "signed_genesis_sha256": sha256(bundle / "genesis.signed.nrt"),
            "unsigned_genesis_sha256": sha256(bundle / "genesis.json"),
            "base_config_sha256": sha256(bundle / "base-config.toml"),
            "chain_id": PUBLIC_TAIRA_CHAIN_ID,
            "chain_discriminant": PUBLIC_TAIRA_CHAIN_DISCRIMINANT,
            "block_cadence_ms": PUBLIC_TAIRA_BLOCK_CADENCE_MS,
            "node_storage_budget_bytes": (
                PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES
            ),
            "node_storage_budget_weights": PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS,
            "nexus_storage_budget_policy": (
                PUBLIC_TAIRA_NODE_STORAGE_BUDGET_POLICY
            ),
            "free_bytes_after_materialization": free_bytes_after_materialization,
            "offline_release_policy": (
                "mandatory-authenticated-kagemusha-v4-activation-height-2"
            ),
            "offline_asset_definition_id": PUBLIC_TAIRA_OFFLINE_ASSET_ID,
            "offline_asset_alias": PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
            "offline_asset_name": PUBLIC_TAIRA_OFFLINE_ASSET_NAME,
            "offline_asset_metadata": PUBLIC_TAIRA_OFFLINE_ASSET_METADATA,
            "offline_asset_scale": PUBLIC_TAIRA_OFFLINE_ASSET_SCALE,
            "offline_escrow_account": PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT,
            "command_authority": command_authority,
            "kagemusha_manifest_sha256": manifest_sha256,
            "kagemusha_release_attestation_sha256": (
                release_attestation_sha256
            ),
            "kagemusha_release_policy_sha256": sha256(
                bundle / "kagemusha" / RELEASE_POLICY_FILE_NAME
            ),
            "kagemusha_release_tree_sha256": release_tree_sha256_value,
            "operator_identity_sha256": operator_identity_sha256,
            "configs": {
                slug: sha256(bundle / "rendered" / slug / "config.toml")
                for slug in VALIDATOR_SLUGS
            },
            "prewarmed_storage_sha256": {
                slug: hashlib.sha256().hexdigest() for slug in VALIDATOR_SLUGS
            },
            "genesis_bootstrap_policy": (
                "canonical-taira-offline-cash-complete-activation-height-2"
            ),
        }
    )
    empty_reset.atomic_write_json(manifest_path, manifest)


def check_bundle(
    bundle: Path,
    *,
    irohad: Path,
    expected_irohad_sha256: str,
) -> dict[str, object]:
    """Perform the complete non-mutating v21 bundle preflight.

    This seals and rechecks both genesis files but does not replace the
    post-check root-signature/semantic receipt produced by the dedicated
    signed-genesis binding verifier.
    """

    if not bundle.is_absolute() or bundle.resolve(strict=True) != bundle:
        fail("bundle must be an absolute canonical path")
    require_private_directory(bundle)
    manifest_path = bundle / "reset-manifest.json"
    manifest = json.loads(
        require_private_file(manifest_path, 1024 * 1024).decode("utf-8")
    )
    if (
        not isinstance(manifest, dict)
        or manifest.get("schema") != MANIFEST_SCHEMA
        or manifest.get("peer_count") != PEER_COUNT
        or manifest.get("chain_id") != PUBLIC_TAIRA_CHAIN_ID
        or manifest.get("chain_discriminant")
        != PUBLIC_TAIRA_CHAIN_DISCRIMINANT
        or manifest.get("block_cadence_ms") != PUBLIC_TAIRA_BLOCK_CADENCE_MS
        or manifest.get("node_storage_budget_bytes")
        != PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES
        or manifest.get("node_storage_budget_weights")
        != PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS
        or manifest.get("nexus_storage_budget_policy")
        != PUBLIC_TAIRA_NODE_STORAGE_BUDGET_POLICY
        or manifest.get("offline_release_policy")
        != "mandatory-authenticated-kagemusha-v4-activation-height-2"
        or manifest.get("offline_asset_definition_id")
        != PUBLIC_TAIRA_OFFLINE_ASSET_ID
        or manifest.get("offline_asset_alias") != PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS
        or manifest.get("offline_asset_name") != PUBLIC_TAIRA_OFFLINE_ASSET_NAME
        or manifest.get("offline_asset_metadata")
        != PUBLIC_TAIRA_OFFLINE_ASSET_METADATA
        or manifest.get("offline_asset_scale") != PUBLIC_TAIRA_OFFLINE_ASSET_SCALE
        or manifest.get("offline_escrow_account")
        != PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT
    ):
        fail("v21 reset manifest projection is invalid")
    genesis_public_key = manifest.get("genesis_public_key")
    if (
        not isinstance(genesis_public_key, str)
        or GENESIS_PUBLIC_KEY_RE.fullmatch(genesis_public_key) is None
    ):
        fail("v21 reset manifest lacks one canonical genesis public key")
    genesis_expected_hash = require_genesis_expected_hash(
        manifest.get("genesis_expected_hash")
    )
    command_authority = require_command_authority(
        manifest.get("command_authority"),
        genesis_public_key=genesis_public_key,
    )
    expected_binary = require_sha256(
        expected_irohad_sha256, "expected irohad SHA-256"
    )
    require_regular_file(irohad, 2 * 1024 * 1024 * 1024, executable=True)
    if (
        sha256(irohad) != expected_binary
        or manifest.get("irohad_sha256") != expected_binary
    ):
        fail("irohad binary does not match the sealed v21 manifest")
    release_root = bundle / "kagemusha"
    if release_tree_sha256(release_root) != manifest.get(
        "kagemusha_release_tree_sha256"
    ):
        fail("v21 Kagemusha release tree does not match the reset manifest")
    release = release_bundle_binding(release_root)
    if (
        release["manifest_sha256"]
        != manifest.get("kagemusha_manifest_sha256")
        or release["release_policy_sha256"]
        != manifest.get("kagemusha_release_policy_sha256")
        or release["release_attestation_sha256"]
        != manifest.get("kagemusha_release_attestation_sha256")
    ):
        fail("v21 Kagemusha catalog identity is invalid")
    operator_raw = require_private_file(
        bundle / OPERATOR_IDENTITY_FILE_NAME, 64 * 1024
    )
    operator_sha256 = hashlib.sha256(operator_raw).hexdigest()
    if operator_sha256 != manifest.get("operator_identity_sha256"):
        fail("v21 operator identity does not match the reset manifest")
    operator_identity = operator_identity_binding(operator_raw, release)
    genesis = genesis_summary(
        bundle / "genesis.json",
        command_authority=command_authority,
        genesis_public_key=genesis_public_key,
        release=release,
        operator_identity=operator_identity,
    )
    signed_genesis = bundle / "genesis.signed.nrt"
    require_private_file(signed_genesis, MAX_GENESIS_BYTES)
    # `prepare` creates this file with `kagami genesis sign`.  The hashes below
    # prevent either side from changing inside the sealed bundle.  Deployment
    # still runs the typed signed-genesis verifier after `check` to prove that
    # the canonical SignedBlock has the same instruction semantics and expected
    # root signer as this independently catalog-bound JSON genesis.
    if (
        sha256(signed_genesis) != manifest.get("signed_genesis_sha256")
        or genesis["sha256"] != manifest.get("unsigned_genesis_sha256")
    ):
        fail("v21 genesis identity does not match the reset manifest")
    base_config_path = bundle / "base-config.toml"
    if sha256(base_config_path) != manifest.get("base_config_sha256"):
        fail("v21 base config does not match the reset manifest")
    release_tree_digest = manifest.get("kagemusha_release_tree_sha256")
    if not isinstance(release_tree_digest, str):
        fail("v21 manifest lacks the Kagemusha release tree identity")
    validate_runtime_config(
        parse_config(base_config_path),
        bundle,
        release_tree_digest,
        genesis_public_key,
        genesis_expected_hash,
    )
    config_hashes = manifest.get("configs")
    if not isinstance(config_hashes, dict) or set(config_hashes) != set(
        VALIDATOR_SLUGS
    ):
        fail("v21 manifest lacks exact validator config identities")
    for slug in VALIDATOR_SLUGS:
        workdir = bundle / "rendered" / slug
        config_path = workdir / "config.toml"
        if sha256(config_path) != config_hashes.get(slug):
            fail(f"v21 config identity mismatch for {slug}")
        validate_runtime_config(
            parse_config(config_path),
            bundle,
            release_tree_digest,
            genesis_public_key,
            genesis_expected_hash,
        )
        storage = workdir / "storage"
        require_private_directory(storage)
        if any(storage.iterdir()):
            fail(f"v21 storage is not empty for {slug}")
        staged_config = workdir / f".deploy-check-config-{os.getpid()}.toml"
        if staged_config.exists() or staged_config.is_symlink():
            fail(f"staged config path already exists: {staged_config}")
        try:
            final_text = require_private_file(
                config_path, MAX_CONFIG_BYTES
            ).decode("utf-8")
            empty_reset.write_private_file(
                staged_config,
                staged_check_config_text(
                    final_text, bundle, release_tree_digest
                ).encode("utf-8"),
            )
            run_checked(
                [
                    str(irohad),
                    "--sora",
                    "--config",
                    str(staged_config),
                    "--check-config",
                ],
                cwd=workdir,
            )
        finally:
            staged_config.unlink(missing_ok=True)
    return {
        "bundle": str(bundle),
        "chain": PUBLIC_TAIRA_CHAIN_ID,
        "block_cadence_ms": PUBLIC_TAIRA_BLOCK_CADENCE_MS,
        "node_storage_budget_bytes": PUBLIC_TAIRA_NODE_STORAGE_BUDGET_BYTES,
        "node_storage_budget_weights": PUBLIC_TAIRA_STORAGE_BUDGET_WEIGHTS,
        "mandatory_offline": True,
        "offline_asset_definition_id": PUBLIC_TAIRA_OFFLINE_ASSET_ID,
        "offline_asset_alias": PUBLIC_TAIRA_OFFLINE_ASSET_ALIAS,
        "offline_asset_name": PUBLIC_TAIRA_OFFLINE_ASSET_NAME,
        "offline_escrow_account": PUBLIC_TAIRA_OFFLINE_ESCROW_ACCOUNT,
        "command_authority": command_authority,
        "genesis_expected_hash": genesis_expected_hash,
        "online_backing_source_ready": True,
        "activation_height": PUBLIC_TAIRA_RELEASE_ACTIVATION_HEIGHT,
        "manifest_sha256": manifest["kagemusha_manifest_sha256"],
        "operator_identity_sha256": operator_sha256,
        "irohad_sha256": expected_binary,
        "peer_count": PEER_COUNT,
        "storage": "empty",
        "status": "preflight-passed",
    }


def prepare(args: argparse.Namespace) -> dict[str, object]:
    """Prepare, sign, seal, and preflight one new v21 bundle."""

    command_authority = require_command_authority(
        args.command_authority,
        genesis_public_key=args.genesis_public_key,
    )
    if (
        GENESIS_PUBLIC_KEY_RE.fullmatch(args.genesis_public_key) is None
    ):
        fail("genesis public key is not canonical Ed25519")
    if re.fullmatch(r"[0-9a-f]{40}", args.source_commit) is None:
        fail("source commit must be one full lowercase Git object id")
    output: Path = args.output_bundle
    if not output.is_absolute():
        fail("output bundle must be absolute")
    command_private_key = require_private_key(args.genesis_private_key_file)
    previous_private_key_sha256 = source_command_private_key_sha256(
        args.source_bundle
    )
    if (
        hashlib.sha256(command_private_key.encode("ascii")).digest()
        == previous_private_key_sha256
    ):
        fail("fresh reset must rotate the archived Kagemusha command key")
    operator_identity_raw = require_private_file(
        args.operator_identity, 64 * 1024
    )
    genesis_summary(
        args.offline_genesis,
        command_authority=command_authority,
        genesis_public_key=args.genesis_public_key,
    )
    require_regular_file(args.irohad, 2 * 1024 * 1024 * 1024, executable=True)
    require_regular_file(args.kagami, 2 * 1024 * 1024 * 1024, executable=True)
    irohad_sha256 = sha256(args.irohad)
    release_move = None
    if getattr(args, "move_release_bundle", False):
        release_move = ReleaseBundleMove.preflight(
            args.release_bundle, output
        )
    run_checked(
        [
            sys.executable,
            str(Path(__file__).with_name("prepare_taira_empty_reset_bundle.py")),
            "--source-bundle",
            str(args.source_bundle),
            "--output-bundle",
            str(output),
            "--irohad-sha256",
            irohad_sha256,
            "--source-commit",
            args.source_commit,
            "--minimum-free-bytes",
            str(args.minimum_free_bytes),
        ]
    )
    try:
        if release_move is None:
            manifest_sha256 = copy_release_bundle(
                args.release_bundle, output / "kagemusha"
            )
            release = release_bundle_binding(output / "kagemusha")
            if release["manifest_sha256"] != manifest_sha256:
                fail("copied release identity changed during materialization")
        else:
            manifest_sha256 = release_move.move_into_output()
            release = release_move.binding
        release_tree_digest = release_tree_sha256(output / "kagemusha")
        operator_identity = operator_identity_binding(
            operator_identity_raw, release
        )
        operator_identity_sha256 = hashlib.sha256(
            operator_identity_raw
        ).hexdigest()
        empty_reset.write_private_file(
            output / OPERATOR_IDENTITY_FILE_NAME,
            operator_identity_raw,
        )
        base_path = output / "base-config.toml"
        empty_reset.write_private_file(
            base_path,
            base_config_text(
                require_private_file(base_path, MAX_CONFIG_BYTES).decode("utf-8"),
                bundle=output,
                release_tree_sha256=release_tree_digest,
                genesis_public_key=args.genesis_public_key,
                genesis_expected_hash=GENESIS_EXPECTED_HASH_PLACEHOLDER,
                command_private_key=command_private_key,
            ).encode("utf-8"),
        )
        secrets_path = output / "validator-secrets.toml"
        empty_reset.write_private_file(
            secrets_path,
            patch_runtime_secrets(
                require_private_file(secrets_path, MAX_CONFIG_BYTES).decode(
                    "utf-8"
                ),
                command_private_key=command_private_key,
            ).encode("utf-8"),
        )
        offline_genesis = require_private_file(
            args.offline_genesis, MAX_GENESIS_BYTES
        )
        empty_reset.write_private_file(output / "genesis.json", offline_genesis)
        empty_reset.write_private_file(
            output / "rendered" / "genesis.json", offline_genesis
        )
        genesis_summary(
            output / "genesis.json",
            command_authority=command_authority,
            genesis_public_key=args.genesis_public_key,
            release=release,
            operator_identity=operator_identity,
        )
        for slug in VALIDATOR_SLUGS:
            config_path = output / "rendered" / slug / "config.toml"
            empty_reset.write_private_file(
                config_path,
                runtime_config_text(
                    require_private_file(config_path, MAX_CONFIG_BYTES).decode(
                        "utf-8"
                    ),
                    bundle=output,
                    release_tree_sha256=release_tree_digest,
                    genesis_public_key=args.genesis_public_key,
                    genesis_expected_hash=GENESIS_EXPECTED_HASH_PLACEHOLDER,
                    command_private_key=command_private_key,
                ).encode("utf-8"),
            )
        require_rotated_command_key_projection(
            output,
            command_private_key=command_private_key,
            previous_private_key_sha256=previous_private_key_sha256,
        )
        signed_genesis = output / "genesis.signed.nrt"
        signed_genesis.unlink()
        expected_hash_output = output / ".genesis.expected_hash"
        if expected_hash_output.exists() or expected_hash_output.is_symlink():
            fail("temporary genesis expected-hash output already exists")
        validator_one = output / "rendered" / VALIDATOR_SLUGS[0]
        run_checked(
            [
                str(args.kagami),
                "genesis",
                "sign",
                str(output / "genesis.json"),
                "--config",
                str(validator_one / "config.toml"),
                "--private-key-file",
                str(args.genesis_private_key_file),
                "--out-file",
                str(signed_genesis),
                "--expected-hash-out",
                str(expected_hash_output),
            ],
            cwd=validator_one,
        )
        os.chmod(signed_genesis, 0o600)
        try:
            os.chmod(expected_hash_output, 0o600)
            genesis_expected_hash = read_genesis_expected_hash(
                expected_hash_output
            )
        finally:
            expected_hash_output.unlink(missing_ok=True)
        for config_path in [
            base_path,
            *(
                output / "rendered" / slug / "config.toml"
                for slug in VALIDATOR_SLUGS
            ),
        ]:
            empty_reset.write_private_file(
                config_path,
                bind_runtime_genesis_expected_hash(
                    require_private_file(
                        config_path, MAX_CONFIG_BYTES
                    ).decode("utf-8"),
                    genesis_expected_hash,
                ).encode("utf-8"),
            )
        free_bytes_after_materialization = empty_reset.require_minimum_free_space(
            output, args.minimum_free_bytes
        )
        update_manifest(
            output,
            irohad=args.irohad,
            kagami=args.kagami,
            source_commit=args.source_commit,
            genesis_public_key=args.genesis_public_key,
            genesis_expected_hash=genesis_expected_hash,
            command_authority=command_authority,
            manifest_sha256=manifest_sha256,
            release_attestation_sha256=str(
                release["release_attestation_sha256"]
            ),
            release_tree_sha256_value=release_tree_digest,
            operator_identity_sha256=operator_identity_sha256,
            free_bytes_after_materialization=free_bytes_after_materialization,
        )
        empty_reset.require_minimum_free_space(output, args.minimum_free_bytes)
        return check_bundle(
            output,
            irohad=args.irohad,
            expected_irohad_sha256=irohad_sha256,
        )
    except BaseException as preparation_error:
        if release_move is not None and release_move.moved:
            try:
                release_move.restore()
            except BaseException as restore_error:
                recovery_error = RuntimeError(
                    "offline reset preparation failed and the moved release "
                    "could not be durably restored; output preserved for "
                    f"recovery: {output}"
                )
                if hasattr(recovery_error, "add_note"):
                    recovery_error.add_note(
                        "original preparation failure: "
                        f"{type(preparation_error).__name__}: "
                        f"{preparation_error}"
                    )
                raise recovery_error from restore_error
        shutil.rmtree(output)
        raise


def parser() -> argparse.ArgumentParser:
    """Build the command-line parser."""

    result = argparse.ArgumentParser(allow_abbrev=False)
    commands = result.add_subparsers(dest="command", required=True)
    prepare_parser = commands.add_parser("prepare", allow_abbrev=False)
    prepare_parser.add_argument("--source-bundle", type=Path, required=True)
    prepare_parser.add_argument("--offline-genesis", type=Path, required=True)
    prepare_parser.add_argument("--release-bundle", type=Path, required=True)
    prepare_parser.add_argument(
        "--move-release-bundle",
        action="store_true",
        help=(
            "atomically move the authenticated release tree into the output "
            "instead of copying it; requires one filesystem and restores the "
            "source on later failure"
        ),
    )
    prepare_parser.add_argument("--operator-identity", type=Path, required=True)
    prepare_parser.add_argument(
        "--genesis-private-key-file", type=Path, required=True
    )
    prepare_parser.add_argument("--genesis-public-key", required=True)
    prepare_parser.add_argument("--command-authority", required=True)
    prepare_parser.add_argument("--kagami", type=Path, required=True)
    prepare_parser.add_argument("--irohad", type=Path, required=True)
    prepare_parser.add_argument("--source-commit", required=True)
    prepare_parser.add_argument("--output-bundle", type=Path, required=True)
    prepare_parser.add_argument(
        "--minimum-free-bytes",
        type=int,
        default=empty_reset.DEFAULT_MINIMUM_FREE_BYTES,
    )

    check_parser = commands.add_parser("check", allow_abbrev=False)
    check_parser.add_argument("--bundle", type=Path, required=True)
    check_parser.add_argument("--irohad", type=Path, required=True)
    check_parser.add_argument("--expected-irohad-sha256", required=True)
    return result


def main() -> int:
    """Run preparation or a non-mutating preflight."""

    args = parser().parse_args()
    if args.command == "prepare":
        report = prepare(args)
    else:
        report = check_bundle(
            args.bundle,
            irohad=args.irohad,
            expected_irohad_sha256=args.expected_irohad_sha256,
        )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
