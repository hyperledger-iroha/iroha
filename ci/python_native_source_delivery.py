#!/usr/bin/env python3
"""Pin clean build source and deliver the same verified wheel's native bytes.

This tool never loads an SDK or native extension. The maintained caller owns the
actual before-build pin, fresh build and ABI/installed-wheel verification. A
delivery receipt proves the file relation, not an independent build or ABI claim.
"""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import uuid
import zipfile

SOURCE_TOOL_ROOT = Path(__file__).resolve().parents[1]
# Isolated Python omits the script directory. Admit only this tool's own
# maintained owners, never an ambient module or an SDK/native import.
sys.path.insert(0, str(SOURCE_TOOL_ROOT / "ci"))
import verify_privacy_python_wheel as wheel

sys.path.insert(0, str(SOURCE_TOOL_ROOT / "scripts"))
import check_native_sdk_artifact as artifact
from compute_workspace_source_manifest import release_source_identity


STATE_SCHEMA = "iroha.python.native-source-state.v1"
DELIVERY_SCHEMA = "iroha.python.native-source-delivery.v1"
MAX_CONTROL_BYTES = 64 * 1024


class DeliveryError(RuntimeError):
    """The original source, wheel, artifact evidence or file owner changed."""


def _json(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def source_pin(root: Path) -> str:
    """Use the existing complete clean-source/Cargo.lock identity owner."""
    root = wheel._canonical_directory(root, "source root")
    return _json(release_source_identity(root))


def assert_source_pin(root: Path, expected: str) -> dict[str, object]:
    """Refuse a dirty or different source before and throughout the build."""
    if type(expected) is not str or not expected or len(expected) > MAX_CONTROL_BYTES:
        raise DeliveryError("source pin must be one bounded original string")
    actual = source_pin(root)
    if actual != expected:
        raise DeliveryError("Python native source changed since the before-build pin")
    return json.loads(actual)


def _source_directory(root: Path) -> Path:
    wheel._canonical_directory(root, "source root")
    return wheel._canonical_directory(
        root / "python/iroha_native/src/iroha_native", "canonical native source owner"
    )


def inspect_source(root: Path) -> dict[str, object]:
    """Seal actual source-owner files and its sole existing native artifact."""
    directory = _source_directory(root)
    files: list[dict[str, str]] = []
    natives: list[dict[str, str]] = []
    for path in sorted(directory.rglob("*")):
        relative = path.relative_to(directory)
        # Python's generated cache is not a package source. Source qualification
        # uses a fresh cache prefix and -B, so these files are never its inputs.
        if "__pycache__" in relative.parts:
            continue
        if path.is_symlink():
            raise DeliveryError("native source owner contains a symbolic link")
        if path.is_dir():
            continue
        raw, seal = wheel._read_stable_regular_file(
            path, label="original native source member", max_bytes=wheel.MAX_MEMBER_BYTES
        )
        entry = {"name": relative.as_posix(), "seal": seal.render()}
        if path.suffix.lower() in (".so", ".pyd", ".dylib", ".dll"):
            if len(relative.parts) != 1 or not path.name.startswith("_crypto"):
                raise DeliveryError("native source has an unexpected extension owner")
            natives.append(entry)
        else:
            files.append(entry)
        del raw
    if len(natives) > 1:
        raise DeliveryError("native source must contain at most one original extension")
    return {"schema": STATE_SCHEMA, "files": files, "native": natives[0] if natives else None}


def _control(raw: bytes, schema: str) -> dict[str, object]:
    if not raw or len(raw) > MAX_CONTROL_BYTES:
        raise DeliveryError("delivery control exceeds its original bound")
    try:
        value = json.loads(raw, object_pairs_hook=artifact._reject_duplicate_object_pairs)
    except (ValueError, UnicodeError) as error:
        raise DeliveryError("delivery control is not canonical JSON") from error
    if type(value) is not dict or value.get("schema") != schema or _json(value).encode() != raw:
        raise DeliveryError("delivery control is not its exact canonical schema")
    return value


def _state(value: object) -> dict[str, object]:
    """Admit only the exact bounded source snapshot's builtin field graph."""
    if (type(value) is not dict or set(value) != {"schema", "files", "native"}
            or value["schema"] != STATE_SCHEMA or type(value["files"]) is not list):
        raise DeliveryError("source snapshot has an unsupported field graph")
    names = []
    entries = value["files"] + ([] if value["native"] is None else [value["native"]])
    for entry in entries:
        if (type(entry) is not dict or set(entry) != {"name", "seal"}
                or type(entry["name"]) is not str or type(entry["seal"]) is not str):
            raise DeliveryError("source snapshot member graph is not exact")
        name = entry["name"]
        path = PurePosixPath(name)
        if (not name or path.is_absolute() or any(part in (".", "..") for part in name.split("/"))
                or path.as_posix() != name or "\\" in name
                or wheel.FileSeal.parse(entry["seal"]).render() != entry["seal"]):
            raise DeliveryError("source snapshot member binding is not canonical")
        names.append(name)
    if len(names) != len(set(names)) or names[:len(value["files"])] != sorted(names[:len(value["files"])]):
        raise DeliveryError("source snapshot member inventory is duplicated or unordered")
    return value


def _inputs(
    root: Path, expected_pin: str, wheel_path: Path, wheel_seal: str,
    manifest_path: Path, manifest_seal: str,
) -> tuple[bytes, wheel.WheelArchive, bytes]:
    identity = assert_source_pin(root, expected_pin)
    payload, _ = wheel._read_stable_regular_file(
        wheel_path, label="original fresh native wheel", max_bytes=wheel.MAX_WHEEL_BYTES,
        expected_seal=wheel.FileSeal.parse(wheel_seal),
    )
    # The original captured payload and the sole existing ZIP/RECORD field walk
    # select the member. Extraction below uses those same immutable bytes.
    archive = wheel.parse_wheel_bytes(payload, owner=wheel.NATIVE_OWNER)
    raw_manifest, _ = wheel._read_stable_regular_file(
        manifest_path, label="original native artifact manifest", max_bytes=MAX_CONTROL_BYTES,
        expected_seal=wheel.FileSeal.parse(manifest_seal),
    )
    manifest = artifact.validate_manifest(json.loads(
        raw_manifest, object_pairs_hook=artifact._reject_duplicate_object_pairs
    ))
    if raw_manifest != artifact.canonical_manifest_bytes(manifest):
        raise DeliveryError("native artifact manifest is not canonical")
    member = next(member for member in archive.package_members if member.name == archive.native_member)
    if (
        manifest["sdk"] != "python"
        or manifest["source_commit"] != identity["head_commit"]
        or manifest["workspace_source_manifest_sha256"] != identity["workspace_source_manifest_sha256"]
        or manifest["artifact_sha256"] != member.sha256
        or manifest["artifact_size"] != member.size
    ):
        raise DeliveryError("fresh wheel differs from the original source-bound ABI evidence")
    with zipfile.ZipFile(io.BytesIO(payload)) as captured:
        native = captured.read(archive.native_member)
    if len(native) != member.size or hashlib.sha256(native).hexdigest() != member.sha256:
        raise DeliveryError("native member differs from the sole parser's original member")
    return payload, archive, native


def _match_sources(state: dict[str, object], archive: wheel.WheelArchive) -> None:
    originals = {entry["name"]: wheel.FileSeal.parse(entry["seal"]) for entry in state["files"]}
    expected = {member.name.removeprefix("iroha_native/"): member
                for member in archive.package_members if member.name != archive.native_member}
    if set(originals) != set(expected):
        raise DeliveryError("fresh wheel and canonical Python source owner inventories differ")
    if any((originals[name].sha256, originals[name].size) != (member.sha256, member.size)
           for name, member in expected.items()):
        raise DeliveryError("fresh wheel does not contain the original canonical Python owner")


def _atomic_move_function():
    """Select only the host's descriptor-relative atomic no-overwrite rename."""
    if sys.platform == "darwin":
        symbol, flag = "renameatx_np", 0x00000004  # Darwin RENAME_EXCL.
    elif sys.platform.startswith("linux"):
        symbol, flag = "renameat2", 1  # Linux RENAME_NOREPLACE.
    else:
        raise DeliveryError("source promotion requires an atomic no-overwrite rename host")
    library = ctypes.CDLL(None, use_errno=True)
    try:
        operation = getattr(library, symbol)
    except AttributeError as error:
        raise DeliveryError("host lacks descriptor-relative atomic no-overwrite rename") from error
    operation.argtypes = (ctypes.c_int, ctypes.c_char_p, ctypes.c_int,
                          ctypes.c_char_p, ctypes.c_uint)
    operation.restype = ctypes.c_int
    return operation, flag


def _move_no_replace(source_fd: int, source_name: str, destination_fd: int, destination_name: str) -> None:
    """Move one physical object atomically; an existing destination always survives."""
    if any(not name or "/" in name or "\\" in name or name in (".", "..")
           for name in (source_name, destination_name)):
        raise DeliveryError("atomic delivery names must be exact basenames")
    operation, flag = _atomic_move_function()
    result = operation(source_fd, os.fsencode(source_name), destination_fd,
                       os.fsencode(destination_name), flag)
    if result != 0:
        number = ctypes.get_errno()
        if number in (errno.ENOSYS, errno.ENOTSUP, errno.EINVAL):
            raise DeliveryError("filesystem lacks atomic no-overwrite rename support")
        raise OSError(number, os.strerror(number), destination_name)


def _open_directory(path: Path) -> int:
    if (os.open not in os.supports_dir_fd
            or not all(getattr(os, flag, 0) for flag in ("O_DIRECTORY", "O_NOFOLLOW", "O_CLOEXEC"))):
        raise DeliveryError("source promotion requires descriptor-relative filesystem operations")
    _atomic_move_function()
    return os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)


def _write_new(directory: int, name: str, data: bytes) -> tuple[int, int]:
    """Create a private retained object; failures never unlink a competing name."""
    descriptor = os.open(name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC,
                         0o600, dir_fd=directory)
    original = os.fstat(descriptor)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as stream:
            stream.write(data)
            stream.flush()
            os.fsync(descriptor)
        return original.st_dev, original.st_ino
    finally:
        # Partial and competing private objects remain diagnostic custody. No
        # check-then-unlink operation can safely identify a shared pathname.
        os.close(descriptor)


def _same_directory(path: Path, descriptor: int) -> None:
    current = path.stat(follow_symlinks=False)
    original = os.fstat(descriptor)
    if path.resolve(strict=True) != path or (current.st_dev, current.st_ino) != (original.st_dev, original.st_ino):
        raise DeliveryError("original source or backup directory changed")


def promote_source(
    root: Path, expected_pin: str, expected_source: str, wheel_path: Path, wheel_seal: str,
    manifest_path: Path, manifest_seal: str, backup_directory: Path,
) -> wheel.FileSeal:
    """Preserve the original artifact and atomically create the exact fresh member.

    The old file is moved into a fresh durable backup before the new file is
    moved into its vacant slot with no overwrite. A competing destination is
    refused. Failed promotion retains the original in the named backup; the
    intent identifies recovery, never an automatic loader fallback.
    """
    _, archive, native = _inputs(root, expected_pin, wheel_path, wheel_seal, manifest_path, manifest_seal)
    if type(expected_source) is not str or len(expected_source) > MAX_CONTROL_BYTES:
        raise DeliveryError("source state must be the bounded original snapshot")
    source = inspect_source(root)
    if _json(source) != expected_source:
        raise DeliveryError("original source artifact changed during the native build")
    _match_sources(source, archive)
    directory = _source_directory(root)
    name = archive.native_member.removeprefix("iroha_native/")
    old = source["native"]
    if old is not None and old["name"] != name:
        raise DeliveryError("source native filename differs from the sole fresh native member")
    backup_directory = wheel._canonical_directory(backup_directory, "original artifact backup")
    if root not in backup_directory.parents or not backup_directory.relative_to(root).parts[0].startswith(".codex-"):
        raise DeliveryError("source artifact backup must be in an ignored repository owner directory")
    if tuple(backup_directory.iterdir()):
        raise DeliveryError("source artifact backup must be fresh and empty")
    if os.name == "posix" and stat.S_IMODE(backup_directory.stat().st_mode) != 0o700:
        raise DeliveryError("source artifact backup requires owner-only Unix permissions")
    source_fd = _open_directory(directory)
    try:
        backup_fd = _open_directory(backup_directory)
    except BaseException:
        os.close(source_fd)
        raise
    parent_directory = backup_directory.parent
    try:
        parent_fd = _open_directory(parent_directory)
    except BaseException:
        try:
            os.close(backup_fd)
        finally:
            os.close(source_fd)
        raise

    def assert_directories() -> None:
        _same_directory(directory, source_fd)
        _same_directory(backup_directory, backup_fd)
        _same_directory(parent_directory, parent_fd)

    temporary = f".fresh-native-{uuid.uuid4().hex}.so"
    try:
        intent = {"schema": DELIVERY_SCHEMA, "source_pin": expected_pin, "source_before": source,
                  "wheel": str(wheel_path), "wheel_seal": wheel_seal,
                  "manifest": str(manifest_path), "manifest_seal": manifest_seal,
                  "native_member": archive.native_member}
        _write_new(backup_fd, "intent.json", _json(intent).encode())
        os.fsync(backup_fd)
        # Sync the existing immediate parent so the fresh private backup entry
        # is durable before removing any source artifact. Ancestors are not
        # created or claimed here; both existing parent and backup remain pinned.
        assert_directories()
        os.fsync(parent_fd)
        temporary_identity = _write_new(backup_fd, temporary, native)
        _, fresh_seal = wheel._read_stable_regular_file(
            backup_directory / temporary, label="original fresh private native object", max_bytes=wheel.MAX_MEMBER_BYTES
        )
        if ((fresh_seal.device, fresh_seal.inode) != temporary_identity
                or (fresh_seal.sha256, fresh_seal.size) != (hashlib.sha256(native).hexdigest(), len(native))):
            raise DeliveryError("fresh private native object changed its original owner")
        os.fsync(backup_fd)
        if _json(inspect_source(root)) != expected_source:
            raise DeliveryError("original source artifact changed before promotion")
        assert_source_pin(root, expected_pin)
        assert_directories()
        original_backup = None
        if old is not None:
            _move_no_replace(source_fd, name, backup_fd, name)
            assert_directories()
            old_bytes, moved = wheel._read_stable_regular_file(
                backup_directory / name, label="original moved source artifact", max_bytes=wheel.MAX_MEMBER_BYTES
            )
            expected_old = wheel.FileSeal.parse(old["seal"])
            # A rename changes ctime. Retain and check the original object and
            # every other field, including complete bytes, before activation.
            if (moved.sha256, moved.device, moved.inode, moved.size, moved.mtime_ns, moved.mode) != (
                expected_old.sha256, expected_old.device, expected_old.inode,
                expected_old.size, expected_old.mtime_ns, expected_old.mode
            ):
                raise DeliveryError("moved artifact is not the original before-build owner")
            del old_bytes
            original_backup = {"name": name, "seal": moved.render()}
            os.fsync(backup_fd)
            os.fsync(source_fd)
        # Both transfers use kernel no-overwrite rename. Private temporary
        # objects are retained on refusal; shared source names are never unlinked.
        assert_directories()
        _move_no_replace(backup_fd, temporary, source_fd, name)
        assert_directories()
        os.fsync(source_fd)
        os.fsync(backup_fd)
        promoted = inspect_source(root)
        actual_native = wheel.FileSeal.parse(promoted["native"]["seal"])
        if (actual_native.sha256, actual_native.device, actual_native.inode, actual_native.size,
                actual_native.mtime_ns, actual_native.mode) != (
                fresh_seal.sha256, fresh_seal.device, fresh_seal.inode, fresh_seal.size,
                fresh_seal.mtime_ns, fresh_seal.mode):
            raise DeliveryError("promoted source is not the original fresh native physical object")
        _match_sources(promoted, archive)
        assert_source_pin(root, expected_pin)
        receipt = dict(intent, source_after=promoted, original_backup=original_backup)
        _write_new(backup_fd, "delivery.json", _json(receipt).encode())
        os.fsync(backup_fd)
        _, receipt_seal = wheel._read_stable_regular_file(
            backup_directory / "delivery.json", label="original source delivery receipt", max_bytes=MAX_CONTROL_BYTES
        )
        return receipt_seal
    finally:
        try:
            os.close(parent_fd)
        finally:
            try:
                os.close(backup_fd)
            finally:
                os.close(source_fd)


def verify_source(
    root: Path, expected_pin: str, wheel_path: Path, wheel_seal: str,
    manifest_path: Path, manifest_seal: str, receipt_path: Path, receipt_seal: str,
) -> None:
    """Revalidate original wheel, source, promoted artifact and preserved backup."""
    _, archive, native = _inputs(root, expected_pin, wheel_path, wheel_seal, manifest_path, manifest_seal)
    raw, _ = wheel._read_stable_regular_file(
        receipt_path, label="original source delivery receipt", max_bytes=MAX_CONTROL_BYTES,
        expected_seal=wheel.FileSeal.parse(receipt_seal),
    )
    receipt = _control(raw, DELIVERY_SCHEMA)
    if set(receipt) != {"schema", "source_pin", "source_before", "wheel", "wheel_seal",
                        "manifest", "manifest_seal", "native_member", "source_after", "original_backup"}:
        raise DeliveryError("delivery receipt field inventory is not exact")
    if (receipt["source_pin"], receipt["wheel"], receipt["wheel_seal"], receipt["manifest"], receipt["manifest_seal"], receipt["native_member"]) != (
        expected_pin, str(wheel_path), wheel_seal, str(manifest_path), manifest_seal, archive.native_member
    ):
        raise DeliveryError("delivery receipt does not bind the original inputs")
    before = _state(receipt["source_before"])
    after = _state(receipt["source_after"])
    if before["files"] != after["files"]:
        raise DeliveryError("delivery changed the original Python source owners")
    source = inspect_source(root)
    if source != after:
        raise DeliveryError("promoted source artifact or original Python owner changed")
    _match_sources(source, archive)
    current = wheel.FileSeal.parse(source["native"]["seal"])
    if (current.sha256, current.size) != (hashlib.sha256(native).hexdigest(), len(native)):
        raise DeliveryError("source artifact differs from the same original fresh native member")
    backup = receipt["original_backup"]
    if (before["native"] is None) != (backup is None):
        raise DeliveryError("delivery omitted its original source artifact custody")
    if backup is not None:
        name = archive.native_member.removeprefix("iroha_native/")
        if type(backup) is not dict or set(backup) != {"name", "seal"} or backup["name"] != name:
            raise DeliveryError("original native backup does not have its exact member name")
        original = before["native"]
        original_seal = wheel.FileSeal.parse(original["seal"])
        moved = wheel.FileSeal.parse(backup["seal"])
        if (original["name"] != name or
                (moved.sha256, moved.device, moved.inode, moved.size, moved.mtime_ns, moved.mode) !=
                (original_seal.sha256, original_seal.device, original_seal.inode, original_seal.size,
                 original_seal.mtime_ns, original_seal.mode)):
            raise DeliveryError("preserved backup differs from the original before-build owner")
        wheel.assert_expected_file_seal(
            receipt_path.parent / backup["name"], moved,
            label="preserved original native source artifact", max_bytes=wheel.MAX_MEMBER_BYTES,
        )
    assert_source_pin(root, expected_pin)


def main() -> int:
    """Execute a bounded source pin, snapshot, promotion or revalidation."""
    parser = argparse.ArgumentParser(description=__doc__, allow_abbrev=False)
    parser.add_argument("mode", choices=("pin", "assert-pin", "inspect", "promote", "verify"))
    parser.add_argument("--root", required=True, type=Path)
    parser.add_argument("--source-pin")
    parser.add_argument("--source-state")
    parser.add_argument("--wheel", type=Path)
    parser.add_argument("--wheel-seal")
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--manifest-seal")
    parser.add_argument("--backup-directory", type=Path)
    parser.add_argument("--receipt", type=Path)
    parser.add_argument("--receipt-seal")
    args = parser.parse_args()
    allowed = {
        "pin": {"root"}, "inspect": {"root"}, "assert-pin": {"root", "source_pin"},
        "promote": {"root", "source_pin", "source_state", "wheel", "wheel_seal", "manifest", "manifest_seal", "backup_directory"},
        "verify": {"root", "source_pin", "wheel", "wheel_seal", "manifest", "manifest_seal", "receipt", "receipt_seal"},
    }
    provided = {name for name, value in vars(args).items() if name != "mode" and value is not None}
    if provided != allowed[args.mode]:
        parser.error("the selected command requires its exact original field inventory")
    try:
        if args.mode == "pin":
            print(source_pin(args.root))
        elif args.mode == "inspect":
            print(_json(inspect_source(args.root)))
        elif args.mode == "assert-pin":
            assert_source_pin(args.root, args.source_pin)
        else:
            if any(value is None for value in (args.source_pin, args.wheel, args.wheel_seal, args.manifest, args.manifest_seal)):
                parser.error("promotion/verification require every original source, wheel and artifact binding")
            common = (args.root, args.source_pin, args.wheel, args.wheel_seal, args.manifest, args.manifest_seal)
            if args.mode == "promote":
                if args.source_state is None or args.backup_directory is None:
                    parser.error("promotion requires the original source state and fresh backup directory")
                print(promote_source(args.root, args.source_pin, args.source_state, args.wheel, args.wheel_seal,
                                     args.manifest, args.manifest_seal, args.backup_directory).render())
            else:
                if args.receipt is None or args.receipt_seal is None:
                    parser.error("verification requires the original delivery receipt and seal")
                verify_source(*common, args.receipt, args.receipt_seal)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
