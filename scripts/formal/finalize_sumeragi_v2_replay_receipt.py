#!/usr/bin/env python3
"""Finalize one externally SSH-signed Sumeragi V2 replay V1 receipt.

The finalizer never signs and accepts no private-key argument. It publishes a
create-only evidence directory only after independently checking the receipt
and detached SSHSIG with checksum-pinned public verification inputs.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
from typing import Any


def _load_local_module(module_name: str, filename: str) -> Any:
    """Load one authenticated sibling without relying on ambient import paths."""

    path = Path(__file__).absolute().parent / filename
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"replay support module is unavailable: {path}")
    canonical_path = path.resolve(strict=True)
    if canonical_path != path:
        raise RuntimeError(f"replay support module path is not canonical: {path}")
    path = canonical_path
    loaded = sys.modules.get(module_name)
    if loaded is not None:
        loaded_path = Path(getattr(loaded, "__file__", "")).resolve()
        if loaded_path != path:
            raise RuntimeError(
                f"replay support module identity differs: {loaded_path} != {path}"
            )
        return loaded
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load replay support module: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    try:
        spec.loader.exec_module(module)
    except BaseException:
        sys.modules.pop(module_name, None)
        raise
    return module


receipt_checker = _load_local_module(
    "check_sumeragi_v2_replay_receipt",
    "check_sumeragi_v2_replay_receipt.py",
)
_REPLAY_SIGNING = receipt_checker._REPLAY_SIGNING
MAX_RECEIPT_BYTES = _REPLAY_SIGNING.MAX_RECEIPT_BYTES
SSHSIG_NAMESPACE = _REPLAY_SIGNING.SSHSIG_NAMESPACE
SIGNATURE_FORMAT = _REPLAY_SIGNING.SIGNATURE_FORMAT
SignatureInputs = _REPLAY_SIGNING.SignatureInputs
SigningError = _REPLAY_SIGNING.SigningError
read_snapshot = _REPLAY_SIGNING.read_snapshot
require_sha256 = _REPLAY_SIGNING.require_sha256
require_unchanged = _REPLAY_SIGNING.require_unchanged
verify_external_signature = _REPLAY_SIGNING.verify_external_signature
verify_exact_signature_bytes = _REPLAY_SIGNING.verify_exact_signature_bytes


ATTESTATION_SCHEMA = "iroha-sumeragi-v2-replay-release-attestation-v1"
WORKSPACE_ROOT = Path(__file__).resolve().parents[2]
FLAT_PAYLOAD_NAMES = (
    "receipt.json",
    "receipt.json.sig",
    "ssh-keygen.release-tool",
    "allowed_signers",
    "revocation.krl",
)
PROJECTION_DIRECTORY_NAME = "tlapm-projection"
PROJECTION_FILE_NAMES = ("Folds.tla", "Functions.tla")
PROJECTION_LOGICAL_NAMES = tuple(
    f"{PROJECTION_DIRECTORY_NAME}/{name}" for name in PROJECTION_FILE_NAMES
)
PAYLOAD_NAMES = (*FLAT_PAYLOAD_NAMES, *PROJECTION_LOGICAL_NAMES)
ATTESTATION_NAME = "release-attestation.json"
OUTPUT_NAMES = (*FLAT_PAYLOAD_NAMES, ATTESTATION_NAME)
MAX_ATTESTATION_BYTES = 256 * 1024
MAX_PROJECTION_FILE_BYTES = 1024 * 1024


class FinalizationError(RuntimeError):
    """The signed replay evidence could not be verified or published."""


@dataclass
class OwnedOutputRoot:
    """Held identities for one create-only release publication."""

    path: Path
    parent_path: Path
    name: str
    parent_descriptor: int
    descriptor: int
    parent_identity: tuple[int, int]
    identity: tuple[int, int]
    created: dict[str, tuple[int, int]] = field(default_factory=dict)
    projection_descriptor: int = -1
    projection_identity: tuple[int, int] | None = None
    projection_created: dict[str, tuple[int, int]] = field(default_factory=dict)
    projection_mtime_ns: int = 0
    projection_ctime_ns: int = 0
    parent_mtime_ns: int = 0
    parent_ctime_ns: int = 0
    root_mtime_ns: int = 0
    root_ctime_ns: int = 0


def canonical_json(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=True,
            allow_nan=False,
        )
        + "\n"
    ).encode("ascii")


def _record(path: str, data: bytes, mode: int) -> dict[str, Any]:
    return {
        "path": path,
        "sha256": hashlib.sha256(data).hexdigest(),
        "size_bytes": len(data),
        "mode": mode,
        "nlink": 1,
    }


def _projection_sources(
    receipt: dict[str, Any], receipt_path: Path
) -> tuple[tuple[str, Any], ...]:
    """Capture the exact signed TLAPM projection used by the replay."""

    records = {
        record.get("path"): record
        for record in receipt.get("tool_identity", {}).get("files", [])
        if isinstance(record, dict)
    }
    locations = receipt_checker._tool_locations(receipt, WORKSPACE_ROOT)
    snapshots: list[tuple[str, Any]] = []
    for logical in PROJECTION_LOGICAL_NAMES:
        path = locations.get(logical)
        if path is None:
            raise FinalizationError(
                f"signed receipt omits the {logical} projection location"
            )
        snapshot = read_snapshot(
            path,
            f"signed replay {logical}",
            maximum_bytes=MAX_PROJECTION_FILE_BYTES,
        )
        if (
            snapshot.mode != 0o444
            or snapshot.record(logical) != records.get(logical)
        ):
            raise FinalizationError(
                f"signed replay {logical} is not the exact read-only tool record"
            )
        snapshots.append((logical, snapshot))
    projection = snapshots[0][1].path.parent
    if (
        any(snapshot.path.parent != projection for _logical, snapshot in snapshots)
        or projection.resolve(strict=True) != projection
        or stat.S_IMODE(projection.lstat().st_mode) != 0o555
        or sorted(item.name for item in projection.iterdir())
        != list(PROJECTION_FILE_NAMES)
    ):
        raise FinalizationError(
            "signed replay TLAPM projection is not one exact read-only two-file root"
        )
    # The signed receipt pathname is kept in the diagnostic to distinguish this
    # publication input from an independently supplied projection.
    if receipt_path.parent == projection:
        raise FinalizationError(
            "signed replay receipt and TLAPM projection must have distinct roots"
        )
    return tuple(snapshots)


def release_attestation(
    artifacts: tuple[tuple[str, bytes, int], ...],
    *,
    principal: str,
    fingerprint: str,
    verification_stdout_sha256: str,
) -> dict[str, Any]:
    """Derive the closed release-attestation value from verified inputs."""

    if tuple(name for name, _data, _mode in artifacts) != PAYLOAD_NAMES:
        raise FinalizationError("release artifact ordering differs")
    return {
        "schema": ATTESTATION_SCHEMA,
        "schema_version": 1,
        "signature": {
            "scheme": SIGNATURE_FORMAT,
            "provider": "openssh-sshsig",
            "namespace": SSHSIG_NAMESPACE,
            "payload": "receipt.json",
            "artifact": "receipt.json.sig",
            "principal": principal,
            "fingerprint": fingerprint,
            "verification_stdout_sha256": verification_stdout_sha256,
        },
        "artifacts": [
            _record(name, data, mode) for name, data, mode in artifacts
        ],
        "publication": {
            "create_only": True,
            "marker_last": True,
            "unexpected_files_allowed": False,
            "symlinks_allowed": False,
            "hard_links_allowed": False,
            "partial": False,
            "projection": {
                "path": PROJECTION_DIRECTORY_NAME,
                "mode": 0o555,
                "read_only": True,
                "files": list(PROJECTION_LOGICAL_NAMES),
            },
        },
    }


def _directory_flags() -> int:
    flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
    )
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _same_identity(metadata: os.stat_result, identity: tuple[int, int]) -> bool:
    return (metadata.st_dev, metadata.st_ino) == identity


def _require_private_directory(
    metadata: os.stat_result, label: str
) -> tuple[int, int]:
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != os.geteuid()
    ):
        raise FinalizationError(f"{label} must be an owner-owned 0700 directory")
    return (metadata.st_dev, metadata.st_ino)


def _require_read_only_projection_directory(
    metadata: os.stat_result, label: str
) -> tuple[int, int]:
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o555
        or metadata.st_uid != os.geteuid()
    ):
        raise FinalizationError(
            f"{label} must be an owner-owned read-only 0555 directory"
        )
    return (metadata.st_dev, metadata.st_ino)


def _prepare_output_root(path: Path) -> OwnedOutputRoot:
    if not path.is_absolute() or path != Path(os.path.abspath(path)):
        raise FinalizationError("--output-root must already be an absolute clean path")
    if path == WORKSPACE_ROOT or WORKSPACE_ROOT in path.parents:
        raise FinalizationError("--output-root must be outside the workspace")
    if path.name in {"", ".", ".."} or "/" in path.name:
        raise FinalizationError("--output-root has an invalid final component")
    parent_path = path.parent
    descriptor = -1
    created_identity: tuple[int, int] | None = None
    try:
        if parent_path.resolve(strict=True) != parent_path:
            raise FinalizationError("finalizer output parent is not canonical")
        parent_link = parent_path.lstat()
        parent_descriptor = os.open(parent_path, _directory_flags())
    except OSError as error:
        raise FinalizationError("finalizer output parent is unavailable") from error
    try:
        parent_opened = os.fstat(parent_descriptor)
        parent_identity = _require_private_directory(
            parent_opened, "finalizer output parent"
        )
        if (
            not _same_identity(parent_link, parent_identity)
            or _require_private_directory(
                parent_link, "finalizer output parent"
            )
            != parent_identity
        ):
            raise FinalizationError("finalizer output parent identity differs")
        try:
            os.stat(path.name, dir_fd=parent_descriptor, follow_symlinks=False)
        except FileNotFoundError:
            pass
        except OSError as error:
            raise FinalizationError(
                "finalizer output root could not be inspected"
            ) from error
        else:
            raise FinalizationError("finalizer output root is create-only")

        os.mkdir(path.name, 0o700, dir_fd=parent_descriptor)
        created_link = os.stat(
            path.name, dir_fd=parent_descriptor, follow_symlinks=False
        )
        created_identity = _require_private_directory(
            created_link, "finalizer output root"
        )
        descriptor = os.open(
            path.name, _directory_flags(), dir_fd=parent_descriptor
        )
        opened = os.fstat(descriptor)
        if (
            _require_private_directory(opened, "finalizer output root")
            != created_identity
        ):
            raise FinalizationError("finalizer output root identity differs")
        return OwnedOutputRoot(
            path=path,
            parent_path=parent_path,
            name=path.name,
            parent_descriptor=parent_descriptor,
            descriptor=descriptor,
            parent_identity=parent_identity,
            identity=created_identity,
        )
    except Exception:
        removable = False
        if created_identity is not None:
            try:
                current = os.stat(
                    path.name,
                    dir_fd=parent_descriptor,
                    follow_symlinks=False,
                )
                removable = _same_identity(current, created_identity)
                if descriptor >= 0:
                    removable = removable and not os.listdir(descriptor)
            except OSError:
                removable = False
        if descriptor >= 0:
            os.close(descriptor)
        if removable:
            try:
                os.rmdir(path.name, dir_fd=parent_descriptor)
                os.fsync(parent_descriptor)
            except OSError:
                pass
        os.close(parent_descriptor)
        raise


def _require_owned_root(output: OwnedOutputRoot) -> None:
    try:
        parent_opened = os.fstat(output.parent_descriptor)
        root_opened = os.fstat(output.descriptor)
        parent_link = output.parent_path.lstat()
        root_link = os.stat(
            output.name,
            dir_fd=output.parent_descriptor,
            follow_symlinks=False,
        )
        lexical_root = output.path.lstat()
        canonical_parent = output.parent_path.resolve(strict=True)
    except OSError as error:
        raise FinalizationError(
            "finalizer output ownership changed during publication"
        ) from error
    if (
        canonical_parent != output.parent_path
        or _require_private_directory(
            parent_opened, "finalizer output parent"
        )
        != output.parent_identity
        or _require_private_directory(parent_link, "finalizer output parent")
        != output.parent_identity
        or _require_private_directory(root_opened, "finalizer output root")
        != output.identity
        or _require_private_directory(root_link, "finalizer output root")
        != output.identity
        or _require_private_directory(lexical_root, "finalizer output root")
        != output.identity
        or (
            output.parent_mtime_ns != 0
            and (
                parent_opened.st_mtime_ns != output.parent_mtime_ns
                or parent_opened.st_ctime_ns != output.parent_ctime_ns
            )
        )
        or (
            output.root_mtime_ns != 0
            and (
                root_opened.st_mtime_ns != output.root_mtime_ns
                or root_opened.st_ctime_ns != output.root_ctime_ns
            )
        )
    ):
        raise FinalizationError(
            "finalizer output parent or root was renamed or replaced"
        )
    if output.projection_descriptor >= 0:
        if output.projection_identity is None:
            raise FinalizationError("release TLAPM projection identity is absent")
        try:
            projection_opened = os.fstat(output.projection_descriptor)
            projection_linked = os.stat(
                PROJECTION_DIRECTORY_NAME,
                dir_fd=output.descriptor,
                follow_symlinks=False,
            )
        except OSError as error:
            raise FinalizationError(
                "release TLAPM projection ownership changed during publication"
            ) from error
        if (
            _require_read_only_projection_directory(
                projection_opened, "release TLAPM projection"
            )
            != output.projection_identity
            or _require_read_only_projection_directory(
                projection_linked, "release TLAPM projection"
            )
            != output.projection_identity
            or (
                output.projection_mtime_ns != 0
                and (
                    projection_opened.st_mtime_ns
                    != output.projection_mtime_ns
                    or projection_opened.st_ctime_ns
                    != output.projection_ctime_ns
                )
            )
        ):
            raise FinalizationError(
                "release TLAPM projection was renamed, replaced, or changed"
            )


def _seal_owned_root(output: OwnedOutputRoot) -> None:
    parent = os.fstat(output.parent_descriptor)
    root = os.fstat(output.descriptor)
    if output.parent_mtime_ns == 0:
        output.parent_mtime_ns = parent.st_mtime_ns
        output.parent_ctime_ns = parent.st_ctime_ns
    elif (
        parent.st_mtime_ns != output.parent_mtime_ns
        or parent.st_ctime_ns != output.parent_ctime_ns
    ):
        raise FinalizationError("finalizer output parent changed after sealing")
    output.root_mtime_ns = root.st_mtime_ns
    output.root_ctime_ns = root.st_ctime_ns
    if output.projection_descriptor >= 0:
        projection = os.fstat(output.projection_descriptor)
        output.projection_mtime_ns = projection.st_mtime_ns
        output.projection_ctime_ns = projection.st_ctime_ns
    _require_owned_root(output)


def _write_create_only(
    output: OwnedOutputRoot, name: str, data: bytes, mode: int
) -> None:
    _require_owned_root(output)
    if name not in OUTPUT_NAMES or name in output.created:
        raise FinalizationError("release artifact name or ordering differs")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(
            name, flags, mode, dir_fd=output.descriptor
        )
    except OSError as error:
        raise FinalizationError(f"{name} could not be created exclusively") from error
    try:
        offset = 0
        while offset < len(data):
            written = os.write(descriptor, data[offset:])
            if written <= 0:
                raise FinalizationError(f"{name} could not be written completely")
            offset += written
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
        metadata = os.fstat(descriptor)
        if (
            not stat.S_ISREG(metadata.st_mode)
            or stat.S_IMODE(metadata.st_mode) != mode
            or metadata.st_uid != os.geteuid()
            or metadata.st_nlink != 1
            or metadata.st_size != len(data)
        ):
            raise FinalizationError(f"{name} publication metadata differs")
        output.created[name] = (metadata.st_dev, metadata.st_ino)
    finally:
        os.close(descriptor)


def _create_projection(
    output: OwnedOutputRoot,
    sources: tuple[tuple[str, Any], ...],
) -> None:
    """Publish the receipt-authenticated TLAPM modules under a held descriptor."""

    _require_owned_root(output)
    if output.projection_descriptor >= 0 or output.projection_identity is not None:
        raise FinalizationError("TLAPM projection may only be created once")
    directory_created = False
    descriptor = -1
    try:
        os.mkdir(PROJECTION_DIRECTORY_NAME, 0o700, dir_fd=output.descriptor)
        directory_created = True
        linked = os.stat(
            PROJECTION_DIRECTORY_NAME,
            dir_fd=output.descriptor,
            follow_symlinks=False,
        )
        identity = _require_private_directory(linked, "release TLAPM projection")
        descriptor = os.open(
            PROJECTION_DIRECTORY_NAME,
            _directory_flags(),
            dir_fd=output.descriptor,
        )
    except BaseException as error:
        if descriptor >= 0:
            os.close(descriptor)
        if directory_created:
            try:
                os.rmdir(PROJECTION_DIRECTORY_NAME, dir_fd=output.descriptor)
            except OSError:
                pass
        if isinstance(error, FinalizationError):
            raise
        raise FinalizationError(
            "release TLAPM projection could not be created"
        ) from error
    output.projection_descriptor = descriptor
    output.projection_identity = identity
    try:
        if (
            _require_private_directory(
                os.fstat(descriptor), "release TLAPM projection"
            )
            != identity
        ):
            raise FinalizationError("release TLAPM projection identity differs")
        for logical, snapshot in sources:
            name = logical.rsplit("/", 1)[-1]
            if (
                logical not in PROJECTION_LOGICAL_NAMES
                or name in output.projection_created
            ):
                raise FinalizationError("release TLAPM projection ordering differs")
            flags = (
                os.O_WRONLY
                | os.O_CREAT
                | os.O_EXCL
                | getattr(os, "O_CLOEXEC", 0)
            )
            if hasattr(os, "O_NOFOLLOW"):
                flags |= os.O_NOFOLLOW
            file_descriptor = os.open(name, flags, 0o444, dir_fd=descriptor)
            try:
                offset = 0
                while offset < len(snapshot.data):
                    written = os.write(file_descriptor, snapshot.data[offset:])
                    if written <= 0:
                        raise FinalizationError(
                            f"{logical} could not be written completely"
                        )
                    offset += written
                os.fchmod(file_descriptor, 0o444)
                os.fsync(file_descriptor)
                metadata = os.fstat(file_descriptor)
                if (
                    not stat.S_ISREG(metadata.st_mode)
                    or stat.S_IMODE(metadata.st_mode) != 0o444
                    or metadata.st_uid != os.geteuid()
                    or metadata.st_nlink != 1
                    or metadata.st_size != len(snapshot.data)
                ):
                    raise FinalizationError(
                        f"{logical} publication metadata differs"
                    )
                output.projection_created[name] = (
                    metadata.st_dev,
                    metadata.st_ino,
                )
            finally:
                os.close(file_descriptor)
        if tuple(
            f"{PROJECTION_DIRECTORY_NAME}/{name}"
            for name in output.projection_created
        ) != PROJECTION_LOGICAL_NAMES:
            raise FinalizationError("release TLAPM projection file set differs")
        os.fchmod(descriptor, 0o555)
        os.fsync(descriptor)
        opened = os.fstat(descriptor)
        linked = os.stat(
            PROJECTION_DIRECTORY_NAME,
            dir_fd=output.descriptor,
            follow_symlinks=False,
        )
        if (
            _require_read_only_projection_directory(
                opened, "release TLAPM projection"
            )
            != identity
            or _require_read_only_projection_directory(
                linked, "release TLAPM projection"
            )
            != identity
        ):
            raise FinalizationError("release TLAPM projection identity differs")
        output.projection_mtime_ns = opened.st_mtime_ns
        output.projection_ctime_ns = opened.st_ctime_ns
        os.fsync(output.descriptor)
    except Exception:
        raise


def _read_held_file(
    output: OwnedOutputRoot,
    name: str,
    *,
    maximum_bytes: int,
    directory_descriptor: int | None = None,
    label: str | None = None,
) -> tuple[bytes, int, tuple[int, int]]:
    directory_descriptor = (
        output.descriptor
        if directory_descriptor is None
        else directory_descriptor
    )
    label = name if label is None else label
    try:
        before = os.stat(
            name, dir_fd=directory_descriptor, follow_symlinks=False
        )
    except OSError as error:
        raise FinalizationError(f"{label} is unavailable") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or before.st_uid != os.geteuid()
        or before.st_nlink != 1
        or before.st_size > maximum_bytes
    ):
        raise FinalizationError(f"{label} is not a bounded single-link file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(name, flags, dir_fd=directory_descriptor)
    try:
        opened = os.fstat(descriptor)
        if not _same_identity(opened, (before.st_dev, before.st_ino)):
            raise FinalizationError(f"{label} changed while opening")
        chunks: list[bytes] = []
        total = 0
        while True:
            chunk = os.read(
                descriptor, min(1024 * 1024, maximum_bytes + 1 - total)
            )
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            if total > maximum_bytes:
                raise FinalizationError(f"{label} exceeds its closed byte limit")
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    linked = os.stat(name, dir_fd=directory_descriptor, follow_symlinks=False)
    stable_fields = (
        "st_dev",
        "st_ino",
        "st_size",
        "st_mtime_ns",
        "st_ctime_ns",
        "st_mode",
        "st_nlink",
        "st_uid",
    )
    if (
        total != opened.st_size
        or any(getattr(opened, key) != getattr(after, key) for key in stable_fields)
        or any(getattr(after, key) != getattr(linked, key) for key in stable_fields)
    ):
        raise FinalizationError(f"{label} changed while reading")
    return (
        b"".join(chunks),
        stat.S_IMODE(after.st_mode),
        (after.st_dev, after.st_ino),
    )


def _validate_exact_output(
    output: OwnedOutputRoot,
    expected: dict[str, tuple[bytes, int]],
) -> dict[str, tuple[bytes, int, tuple[int, int]]]:
    _require_owned_root(output)
    try:
        names = os.listdir(output.descriptor)
    except OSError as error:
        raise FinalizationError("finalizer output could not be enumerated") from error
    flat_expected = {
        name for name in expected if name not in PROJECTION_LOGICAL_NAMES
    }
    expected_root_names = {*flat_expected, PROJECTION_DIRECTORY_NAME}
    if (
        len(names) != len(set(names))
        or set(names) != expected_root_names
        or output.projection_descriptor < 0
        or output.projection_identity is None
    ):
        raise FinalizationError("finalizer output path set differs")
    observed: dict[str, tuple[bytes, int, tuple[int, int]]] = {}
    for name in sorted(flat_expected):
        data, mode, identity = _read_held_file(
            output,
            name,
            maximum_bytes=(
                MAX_ATTESTATION_BYTES
                if name == ATTESTATION_NAME
                else max(len(expected[name][0]), 1)
            ),
        )
        expected_data, expected_mode = expected[name]
        if (
            data != expected_data
            or mode != expected_mode
            or output.created.get(name) != identity
        ):
            raise FinalizationError(f"{name} final bytes or identity differ")
        observed[name] = (data, mode, identity)
    try:
        projection_names = os.listdir(output.projection_descriptor)
        projection_opened = os.fstat(output.projection_descriptor)
        projection_linked = os.stat(
            PROJECTION_DIRECTORY_NAME,
            dir_fd=output.descriptor,
            follow_symlinks=False,
        )
    except OSError as error:
        raise FinalizationError(
            "finalizer TLAPM projection could not be enumerated"
        ) from error
    if (
        len(projection_names) != len(set(projection_names))
        or set(projection_names) != set(PROJECTION_FILE_NAMES)
        or _require_read_only_projection_directory(
            projection_opened, "release TLAPM projection"
        )
        != output.projection_identity
        or _require_read_only_projection_directory(
            projection_linked, "release TLAPM projection"
        )
        != output.projection_identity
    ):
        raise FinalizationError("finalizer TLAPM projection path set differs")
    for logical in PROJECTION_LOGICAL_NAMES:
        name = logical.rsplit("/", 1)[-1]
        data, mode, identity = _read_held_file(
            output,
            name,
            maximum_bytes=MAX_PROJECTION_FILE_BYTES,
            directory_descriptor=output.projection_descriptor,
            label=logical,
        )
        expected_data, expected_mode = expected[logical]
        if (
            data != expected_data
            or mode != expected_mode
            or output.projection_created.get(name) != identity
        ):
            raise FinalizationError(
                f"{logical} final bytes or identity differ"
            )
        observed[logical] = (data, mode, identity)
    _require_owned_root(output)
    return observed


def _cleanup_owned_root(output: OwnedOutputRoot) -> None:
    failures: list[str] = []
    try:
        if output.projection_descriptor >= 0:
            projection_remaining: list[str] = []
            projection_owned = False
            try:
                projection_opened = os.fstat(output.projection_descriptor)
                projection_linked = os.stat(
                    PROJECTION_DIRECTORY_NAME,
                    dir_fd=output.descriptor,
                    follow_symlinks=False,
                )
                projection_owned = (
                    output.projection_identity is not None
                    and _same_identity(
                        projection_opened, output.projection_identity
                    )
                    and _same_identity(
                        projection_linked, output.projection_identity
                    )
                    and stat.S_ISDIR(projection_opened.st_mode)
                    and projection_opened.st_uid == os.geteuid()
                )
            except OSError:
                projection_owned = False
            if projection_owned:
                try:
                    os.fchmod(output.projection_descriptor, 0o700)
                except OSError:
                    failures.append("<projection-mode>")
                else:
                    for name in reversed(tuple(output.projection_created)):
                        try:
                            metadata = os.stat(
                                name,
                                dir_fd=output.projection_descriptor,
                                follow_symlinks=False,
                            )
                            if (
                                not stat.S_ISREG(metadata.st_mode)
                                or metadata.st_nlink != 1
                                or not _same_identity(
                                    metadata,
                                    output.projection_created[name],
                                )
                            ):
                                failures.append(
                                    f"{PROJECTION_DIRECTORY_NAME}/{name}"
                                )
                                continue
                            os.unlink(
                                name, dir_fd=output.projection_descriptor
                            )
                        except FileNotFoundError:
                            continue
                        except OSError:
                            failures.append(
                                f"{PROJECTION_DIRECTORY_NAME}/{name}"
                            )
                    try:
                        os.fsync(output.projection_descriptor)
                    except OSError:
                        failures.append("<projection-fsync>")
                    try:
                        projection_remaining = os.listdir(
                            output.projection_descriptor
                        )
                    except OSError:
                        projection_remaining = ["<unreadable>"]
                    if projection_remaining:
                        failures.extend(
                            f"{PROJECTION_DIRECTORY_NAME}/{name}"
                            for name in projection_remaining
                        )
                    else:
                        try:
                            os.rmdir(
                                PROJECTION_DIRECTORY_NAME,
                                dir_fd=output.descriptor,
                            )
                        except OSError:
                            failures.append("<projection-link>")
            else:
                failures.append("<projection-link>")
        for name in reversed(tuple(output.created)):
            try:
                metadata = os.stat(
                    name,
                    dir_fd=output.descriptor,
                    follow_symlinks=False,
                )
                if (
                    not stat.S_ISREG(metadata.st_mode)
                    or metadata.st_nlink != 1
                    or not _same_identity(metadata, output.created[name])
                ):
                    failures.append(name)
                    continue
                os.unlink(name, dir_fd=output.descriptor)
            except FileNotFoundError:
                continue
            except OSError:
                failures.append(name)
        try:
            os.fsync(output.descriptor)
        except OSError:
            failures.append("<root-fsync>")
        try:
            remaining = os.listdir(output.descriptor)
        except OSError:
            remaining = ["<unreadable>"]
        if remaining:
            failures.extend(str(name) for name in remaining)
        try:
            root_link = os.stat(
                output.name,
                dir_fd=output.parent_descriptor,
                follow_symlinks=False,
            )
            if not _same_identity(root_link, output.identity):
                failures.append("<root-link>")
            elif not remaining:
                os.rmdir(output.name, dir_fd=output.parent_descriptor)
                os.fsync(output.parent_descriptor)
        except OSError:
            failures.append("<root-link>")
    finally:
        if output.projection_descriptor >= 0:
            os.close(output.projection_descriptor)
        os.close(output.descriptor)
        os.close(output.parent_descriptor)
    if failures:
        raise FinalizationError(
            "owned finalizer output could not be reclaimed: "
            + ", ".join(sorted(set(failures)))
        )


def _close_success(output: OwnedOutputRoot) -> None:
    try:
        _require_owned_root(output)
    finally:
        if output.projection_descriptor >= 0:
            os.close(output.projection_descriptor)
        os.close(output.descriptor)
        os.close(output.parent_descriptor)


def finalize(args: argparse.Namespace) -> Path:
    receipt_path = Path(os.path.abspath(args.receipt))
    receipt = read_snapshot(
        receipt_path, "canonical receipt", maximum_bytes=MAX_RECEIPT_BYTES
    )
    if receipt.mode != 0o600:
        raise FinalizationError("receipt.json mode must be 0600")
    receipt_value = receipt_checker._check_structure(receipt_path)
    require_unchanged(
        receipt, "canonical receipt", maximum_bytes=MAX_RECEIPT_BYTES
    )
    projection_sources = _projection_sources(receipt_value, receipt_path)

    expected = {
        "signature": require_sha256(
            args.expected_signature_sha256,
            "expected detached SSHSIG digest",
        ),
        "ssh_keygen": require_sha256(
            args.expected_ssh_keygen_sha256,
            "expected ssh-keygen digest",
        ),
        "allowed": require_sha256(
            args.expected_allowed_signers_sha256,
            "expected allowed-signers digest",
        ),
        "revocation": require_sha256(
            args.expected_revocation_sha256,
            "expected revocation digest",
        ),
    }
    source_inputs = SignatureInputs(
        signature=args.signature,
        expected_signature_sha256=expected["signature"],
        ssh_keygen=args.ssh_keygen_bin,
        expected_ssh_keygen_sha256=expected["ssh_keygen"],
        allowed_signers=args.allowed_signers,
        expected_allowed_signers_sha256=expected["allowed"],
        revocation_file=args.revocation_file,
        expected_revocation_sha256=expected["revocation"],
        principal=args.principal,
        expected_signer_fingerprint=args.expected_signer_fingerprint,
    )
    verified_sources = verify_external_signature(receipt, source_inputs)
    for logical, snapshot in projection_sources:
        require_unchanged(
            snapshot,
            f"signed replay {logical}",
            maximum_bytes=MAX_PROJECTION_FILE_BYTES,
        )
    artifacts = (
        ("receipt.json", receipt.data, 0o400),
        ("receipt.json.sig", verified_sources.signature.data, 0o400),
        (
            "ssh-keygen.release-tool",
            verified_sources.ssh_keygen.data,
            0o500,
        ),
        ("allowed_signers", verified_sources.allowed_signers.data, 0o400),
        ("revocation.krl", verified_sources.revocation_file.data, 0o400),
        *(
            (logical, snapshot.data, 0o444)
            for logical, snapshot in projection_sources
        ),
    )

    output = _prepare_output_root(args.output_root)
    try:
        expected_files: dict[str, tuple[bytes, int]] = {}
        for name, data, mode in artifacts[: len(FLAT_PAYLOAD_NAMES)]:
            _write_create_only(output, name, data, mode)
            expected_files[name] = (data, mode)
        _create_projection(output, projection_sources)
        for name, data, mode in artifacts[len(FLAT_PAYLOAD_NAMES) :]:
            expected_files[name] = (data, mode)
        os.fsync(output.descriptor)
        _seal_owned_root(output)
        archived_files = _validate_exact_output(output, expected_files)

        archived_verification = verify_exact_signature_bytes(
            receipt=archived_files["receipt.json"][0],
            signature=archived_files["receipt.json.sig"][0],
            ssh_keygen=archived_files["ssh-keygen.release-tool"][0],
            allowed_signers=archived_files["allowed_signers"][0],
            revocation_file=archived_files["revocation.krl"][0],
            expected_signature_sha256=expected["signature"],
            expected_ssh_keygen_sha256=expected["ssh_keygen"],
            expected_allowed_signers_sha256=expected["allowed"],
            expected_revocation_sha256=expected["revocation"],
            principal=args.principal,
            expected_signer_fingerprint=args.expected_signer_fingerprint,
        )
        if _validate_exact_output(output, expected_files) != archived_files:
            raise FinalizationError(
                "archived release inputs changed during verification"
            )

        attestation = release_attestation(
            artifacts,
            principal=args.principal,
            fingerprint=archived_verification.signer_fingerprint,
            verification_stdout_sha256=archived_verification.stdout_sha256,
        )
        attestation_bytes = canonical_json(attestation)
        _write_create_only(
            output, ATTESTATION_NAME, attestation_bytes, 0o400
        )
        expected_files[ATTESTATION_NAME] = (attestation_bytes, 0o400)
        os.fsync(output.descriptor)
        os.fsync(output.parent_descriptor)
        _seal_owned_root(output)
        _validate_exact_output(output, expected_files)
        # This identity check is intentionally the final filesystem operation
        # before success; the attestation marker was the final created node.
        _close_success(output)
        return output.path / ATTESTATION_NAME
    except BaseException as error:
        try:
            _cleanup_owned_root(output)
        except FinalizationError as cleanup_error:
            raise cleanup_error from error
        raise


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("receipt", type=Path)
    parser.add_argument("--signature", type=Path, required=True)
    parser.add_argument("--expected-signature-sha256", required=True)
    parser.add_argument("--ssh-keygen-bin", type=Path, required=True)
    parser.add_argument("--expected-ssh-keygen-sha256", required=True)
    parser.add_argument("--allowed-signers", type=Path, required=True)
    parser.add_argument("--expected-allowed-signers-sha256", required=True)
    parser.add_argument("--revocation-file", type=Path, required=True)
    parser.add_argument("--expected-revocation-sha256", required=True)
    parser.add_argument("--principal", required=True)
    parser.add_argument("--expected-signer-fingerprint", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    return parser


def main() -> int:
    try:
        result = finalize(_parser().parse_args())
    except (
        FinalizationError,
        SigningError,
        receipt_checker.ReceiptError,
        OSError,
        UnicodeError,
        ValueError,
    ) as error:
        print(f"Sumeragi V2 replay finalization failed: {error}", file=sys.stderr)
        return 2
    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
