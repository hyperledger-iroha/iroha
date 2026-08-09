#!/usr/bin/env python3
"""Check a staged or live runtime-provider broker installation.

Prerequisites: Python 3.11+ and an exported, secret-free canonical provider
catalog when runtime providers are enabled. The checker is read-only. It never
loads provider plugins, credentials, private keys, or backend configuration.
"""

from __future__ import annotations

import argparse
import grp
import hashlib
import hmac
import os
import pwd
import re
import stat
import sys
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Sequence


CATALOG_MAX_BYTES_V1 = 256 * 1024
BROKER_EXECUTABLE_MAX_BYTES_V1 = 512 * 1024 * 1024
SUPERVISOR_ASSET_MAX_BYTES_V1 = 128 * 1024
SERVICE_USER = "iroha"
SERVICE_GROUP = "iroha"
REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
SHA256_HEX_RE = re.compile(r"[0-9a-f]{64}")


class InstallCheckError(RuntimeError):
    """A fail-closed runtime-provider broker installation error."""


@dataclass(frozen=True)
class InstallLayout:
    """Fixed V1 deployment paths for one supported operating system."""

    platform: str
    executable: PurePosixPath
    catalog: PurePosixPath
    runtime_directory: PurePosixPath
    socket: PurePosixPath
    supervisor_asset: PurePosixPath
    supervisor_template: PurePosixPath
    consumer_assets: tuple[tuple[PurePosixPath, PurePosixPath], ...]


LAYOUTS = {
    "linux": InstallLayout(
        platform="linux",
        executable=PurePosixPath(
            "/usr/local/libexec/iroha-runtime-provider-broker-v1"
        ),
        catalog=PurePosixPath(
            "/etc/iroha/runtime-provider-broker/catalog.norito"
        ),
        runtime_directory=PurePosixPath(
            "/run/iroha-runtime-provider-broker-v1"
        ),
        socket=PurePosixPath(
            "/run/iroha-runtime-provider-broker-v1/"
            "runtime-provider-broker-v1.sock"
        ),
        supervisor_asset=PurePosixPath(
            "/etc/systemd/system/iroha-runtime-provider-broker-v1.service"
        ),
        supervisor_template=PurePosixPath(
            "configs/sorafs/runtime_provider_broker/systemd/"
            "iroha-runtime-provider-broker-v1.service"
        ),
        consumer_assets=(
            (
                PurePosixPath(
                    "/etc/systemd/system/taira-irohad.service.d/"
                    "20-runtime-provider-broker-v1.conf"
                ),
                PurePosixPath(
                    "configs/sorafs/runtime_provider_broker/systemd/"
                    "taira-irohad.service.d/20-runtime-provider-broker-v1.conf"
                ),
            ),
            (
                PurePosixPath(
                    "/etc/systemd/system/sorafs-governance-dag@.service.d/"
                    "20-runtime-provider-broker-v1.conf"
                ),
                PurePosixPath(
                    "configs/sorafs/runtime_provider_broker/systemd/"
                    "sorafs-governance-dag@.service.d/"
                    "20-runtime-provider-broker-v1.conf"
                ),
            ),
        ),
    ),
    "macos": InstallLayout(
        platform="macos",
        executable=PurePosixPath(
            "/usr/local/libexec/iroha-runtime-provider-broker-v1"
        ),
        # /etc is a symlink on macOS. The Rust loader rejects symlink path
        # components, so the canonical path names /private/etc explicitly.
        catalog=PurePosixPath(
            "/private/etc/iroha/runtime-provider-broker/catalog.norito"
        ),
        runtime_directory=PurePosixPath("/private/var/iroha/run"),
        socket=PurePosixPath(
            "/private/var/iroha/run/runtime-provider-broker-v1.sock"
        ),
        supervisor_asset=PurePosixPath(
            "/Library/LaunchDaemons/"
            "org.hyperledger.iroha.runtime-provider-broker-v1.plist"
        ),
        supervisor_template=PurePosixPath(
            "configs/sorafs/runtime_provider_broker/launchd/"
            "org.hyperledger.iroha.runtime-provider-broker-v1.plist"
        ),
        consumer_assets=(),
    ),
}


def _under_root(root: Path, absolute_path: PurePosixPath) -> Path:
    if not absolute_path.is_absolute():
        raise AssertionError("fixed deployment path must be absolute")
    return root.joinpath(*absolute_path.parts[1:])


def _expected_supervisor_template(
    layout: InstallLayout, repository_root: Path = REPOSITORY_ROOT
) -> Path:
    template = layout.supervisor_template
    if template.is_absolute() or any(
        part in {"", ".", ".."} for part in template.parts
    ):
        raise AssertionError("supervisor template path must be repository-relative")
    return repository_root.joinpath(*template.parts)


def _expected_repository_asset(
    relative: PurePosixPath, repository_root: Path = REPOSITORY_ROOT
) -> Path:
    if relative.is_absolute() or any(
        part in {"", ".", ".."} for part in relative.parts
    ):
        raise AssertionError("deployment template path must be repository-relative")
    return repository_root.joinpath(*relative.parts)


def _require_install_root(install_root: Path) -> None:
    if not install_root.is_absolute() or ".." in install_root.parts:
        raise InstallCheckError("--install-root must be an absolute normal path")
    try:
        root_info = install_root.lstat()
    except OSError as error:
        raise InstallCheckError("--install-root does not exist") from error
    if stat.S_ISLNK(root_info.st_mode) or not stat.S_ISDIR(root_info.st_mode):
        raise InstallCheckError("--install-root is not a non-symlink directory")


def _identity(
    info: os.stat_result,
) -> tuple[int, int, int, int, int, int, int, int, int]:
    return (
        info.st_dev,
        info.st_ino,
        info.st_size,
        info.st_mode,
        info.st_uid,
        info.st_gid,
        info.st_nlink,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )


def _read_regular_bounded(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
    require_single_link: bool,
) -> tuple[bytes, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise InstallCheckError(f"{label} is not installed") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise InstallCheckError(f"{label} is not a non-symlink regular file")
    if require_single_link and before.st_nlink != 1:
        raise InstallCheckError(f"{label} must have exactly one hard link")
    if before.st_size == 0:
        raise InstallCheckError(f"{label} is empty")
    if before.st_size > maximum_bytes:
        raise InstallCheckError(f"{label} exceeds the V1 byte limit")

    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise InstallCheckError(f"{label} cannot be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode):
            raise InstallCheckError(f"{label} changed before it was opened")
        chunks: list[bytes] = []
        remaining = maximum_bytes + 1
        while remaining:
            chunk = os.read(descriptor, min(remaining, 64 * 1024))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        payload = b"".join(chunks)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    if _identity(before) != _identity(opened) or _identity(opened) != _identity(after):
        raise InstallCheckError(f"{label} changed while it was checked")
    if len(payload) != opened.st_size:
        raise InstallCheckError(f"{label} changed while it was read")
    if len(payload) > maximum_bytes:
        raise InstallCheckError(f"{label} exceeds the V1 byte limit")
    return payload, opened


def _sha256_regular_bounded(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
) -> tuple[str, os.stat_result]:
    try:
        before = path.lstat()
    except OSError as error:
        raise InstallCheckError(f"{label} is not installed") from error
    if stat.S_ISLNK(before.st_mode) or not stat.S_ISREG(before.st_mode):
        raise InstallCheckError(f"{label} is not a non-symlink regular file")
    if before.st_nlink != 1:
        raise InstallCheckError(f"{label} must have exactly one hard link")
    if before.st_size == 0:
        raise InstallCheckError(f"{label} is empty")
    if before.st_size > maximum_bytes:
        raise InstallCheckError(f"{label} exceeds the V1 byte limit")

    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise InstallCheckError(f"{label} cannot be opened safely") from error
    try:
        opened = os.fstat(descriptor)
        if not stat.S_ISREG(opened.st_mode):
            raise InstallCheckError(f"{label} changed before it was opened")
        digest = hashlib.sha256()
        measured = 0
        while measured <= maximum_bytes:
            chunk = os.read(descriptor, min(64 * 1024, maximum_bytes + 1 - measured))
            if not chunk:
                break
            digest.update(chunk)
            measured += len(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    if _identity(before) != _identity(opened) or _identity(opened) != _identity(after):
        raise InstallCheckError(f"{label} changed while it was checked")
    if measured != opened.st_size:
        raise InstallCheckError(f"{label} changed while it was read")
    if measured > maximum_bytes:
        raise InstallCheckError(f"{label} exceeds the V1 byte limit")
    return digest.hexdigest(), opened


def _require_canonical_sha256(value: str) -> None:
    if SHA256_HEX_RE.fullmatch(value) is None or value == "0" * 64:
        raise InstallCheckError(
            "expected broker executable SHA-256 is not canonical non-zero lowercase hex"
        )


def _require_secure_directories(
    path: Path,
    *,
    install_root: Path,
    trusted_owner_uids: frozenset[int],
) -> None:
    current = path.parent
    while True:
        try:
            info = current.lstat()
        except OSError as error:
            raise InstallCheckError("an installation directory is missing") from error
        if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
            raise InstallCheckError(
                "an installation path component is not a non-symlink directory"
            )
        if info.st_uid not in trusted_owner_uids:
            raise InstallCheckError(
                "an installation directory has an untrusted owner"
            )
        if stat.S_IMODE(info.st_mode) & 0o022:
            raise InstallCheckError(
                "an installation directory is writable by group or other"
            )
        if current == install_root:
            return
        parent = current.parent
        if parent == current or install_root not in current.parents:
            raise InstallCheckError("fixed installation path escaped the install root")
        current = parent


def _service_can_access(
    info: os.stat_result,
    *,
    service_uid: int,
    service_gid: int,
    owner_mask: int,
    group_mask: int,
    other_mask: int,
) -> bool:
    mode = stat.S_IMODE(info.st_mode)
    if info.st_uid == service_uid:
        return bool(mode & owner_mask)
    if info.st_gid == service_gid:
        return bool(mode & group_mask)
    return bool(mode & other_mask)


def _check_executable(
    path: Path,
    *,
    install_root: Path,
    service_uid: int,
    service_gid: int,
    trusted_artifact_owner_uid: int,
    expected_sha256: str,
) -> None:
    _require_canonical_sha256(expected_sha256)
    _require_secure_directories(
        path,
        install_root=install_root,
        trusted_owner_uids=frozenset({trusted_artifact_owner_uid}),
    )
    measured_sha256, info = _sha256_regular_bounded(
        path,
        label="runtime-provider broker executable",
        maximum_bytes=BROKER_EXECUTABLE_MAX_BYTES_V1,
    )
    if info.st_uid != trusted_artifact_owner_uid:
        raise InstallCheckError(
            "runtime-provider broker executable has an untrusted owner"
        )
    if stat.S_IMODE(info.st_mode) & 0o7222:
        raise InstallCheckError(
            "runtime-provider broker executable has unsafe mode bits"
        )
    if not _service_can_access(
        info,
        service_uid=service_uid,
        service_gid=service_gid,
        owner_mask=stat.S_IXUSR,
        group_mask=stat.S_IXGRP,
        other_mask=stat.S_IXOTH,
    ):
        raise InstallCheckError(
            "runtime-provider broker executable is not executable by the service UID"
        )
    if not hmac.compare_digest(measured_sha256, expected_sha256):
        raise InstallCheckError(
            "installed runtime-provider broker executable SHA-256 differs from the "
            "externally verified release digest"
        )


def _check_runtime_directory(
    path: Path,
    *,
    install_root: Path,
    service_uid: int,
    service_gid: int,
) -> None:
    _require_secure_directories(
        path / "runtime-provider-broker-v1.sock",
        install_root=install_root,
        trusted_owner_uids=frozenset({0, service_uid}),
    )
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
        raise InstallCheckError(
            "runtime-provider broker runtime directory is not a non-symlink directory"
        )
    if info.st_uid != service_uid or info.st_gid != service_gid:
        raise InstallCheckError(
            "runtime-provider broker runtime directory is not owned by the service UID/GID"
        )
    if stat.S_IMODE(info.st_mode) != 0o700:
        raise InstallCheckError(
            "runtime-provider broker runtime directory mode is not 0700"
        )


def _check_supervisor_asset(
    installed_path: Path,
    *,
    expected_path: Path,
    install_root: Path,
    trusted_artifact_owner_uid: int,
    label: str = "installed runtime-provider supervisor asset",
) -> None:
    expected_bytes, _ = _read_regular_bounded(
        expected_path,
        label="checked-in runtime-provider supervisor template",
        maximum_bytes=SUPERVISOR_ASSET_MAX_BYTES_V1,
        require_single_link=True,
    )
    _require_secure_directories(
        installed_path,
        install_root=install_root,
        trusted_owner_uids=frozenset({trusted_artifact_owner_uid}),
    )
    installed_bytes, installed_info = _read_regular_bounded(
        installed_path,
        label=label,
        maximum_bytes=SUPERVISOR_ASSET_MAX_BYTES_V1,
        require_single_link=True,
    )
    if installed_info.st_uid != trusted_artifact_owner_uid:
        raise InstallCheckError(
            f"{label} has an untrusted owner"
        )
    if stat.S_IMODE(installed_info.st_mode) & 0o7222:
        raise InstallCheckError(
            f"{label} has unsafe mode bits"
        )
    if not hmac.compare_digest(expected_bytes, installed_bytes):
        raise InstallCheckError(
            f"{label} differs from the checked-in platform template"
        )


def validate_install(
    *,
    layout: InstallLayout,
    install_root: Path,
    expected_catalog: Path,
    expected_executable_sha256: str,
    service_uid: int,
    service_gid: int,
    trusted_artifact_owner_uid: int,
    check_runtime_directory: bool,
) -> None:
    """Validate one enabled broker installation against its exported catalog."""

    _require_install_root(install_root)

    expected_bytes, _ = _read_regular_bounded(
        expected_catalog,
        label="expected runtime-provider catalog",
        maximum_bytes=CATALOG_MAX_BYTES_V1,
        require_single_link=False,
    )
    executable_path = _under_root(install_root, layout.executable)
    installed_catalog_path = _under_root(install_root, layout.catalog)
    _check_executable(
        executable_path,
        install_root=install_root,
        service_uid=service_uid,
        service_gid=service_gid,
        trusted_artifact_owner_uid=trusted_artifact_owner_uid,
        expected_sha256=expected_executable_sha256,
    )
    _check_supervisor_asset(
        _under_root(install_root, layout.supervisor_asset),
        expected_path=_expected_supervisor_template(layout),
        install_root=install_root,
        trusted_artifact_owner_uid=trusted_artifact_owner_uid,
    )
    for installed_asset, template_asset in layout.consumer_assets:
        _check_supervisor_asset(
            _under_root(install_root, installed_asset),
            expected_path=_expected_repository_asset(template_asset),
            install_root=install_root,
            trusted_artifact_owner_uid=trusted_artifact_owner_uid,
            label="installed runtime-provider consumer drop-in",
        )
    _require_secure_directories(
        installed_catalog_path,
        install_root=install_root,
        trusted_owner_uids=frozenset({trusted_artifact_owner_uid}),
    )
    installed_bytes, installed_info = _read_regular_bounded(
        installed_catalog_path,
        label="installed runtime-provider catalog",
        maximum_bytes=CATALOG_MAX_BYTES_V1,
        require_single_link=True,
    )
    if installed_info.st_uid != trusted_artifact_owner_uid:
        raise InstallCheckError(
            "installed runtime-provider catalog has an untrusted owner"
        )
    if stat.S_IMODE(installed_info.st_mode) & 0o7222:
        raise InstallCheckError(
            "installed runtime-provider catalog has unsafe mode bits"
        )
    if not _service_can_access(
        installed_info,
        service_uid=service_uid,
        service_gid=service_gid,
        owner_mask=stat.S_IRUSR,
        group_mask=stat.S_IRGRP,
        other_mask=stat.S_IROTH,
    ):
        raise InstallCheckError(
            "installed runtime-provider catalog is not readable by the service UID"
        )
    if not hmac.compare_digest(expected_bytes, installed_bytes):
        raise InstallCheckError(
            "installed runtime-provider catalog differs from the expected canonical bytes"
        )
    if check_runtime_directory or layout.platform == "macos":
        _check_runtime_directory(
            _under_root(install_root, layout.runtime_directory),
            install_root=install_root,
            service_uid=service_uid,
            service_gid=service_gid,
        )


def validate_disabled_install(*, layout: InstallLayout, install_root: Path) -> None:
    """Reject a disabled declaration when a non-empty fixed catalog is present."""

    _require_install_root(install_root)
    installed_catalog = _under_root(install_root, layout.catalog)
    try:
        info = installed_catalog.lstat()
    except FileNotFoundError:
        return
    except OSError as error:
        raise InstallCheckError(
            "disabled runtime-provider catalog path cannot be inspected"
        ) from error
    if stat.S_ISREG(info.st_mode) and info.st_size == 0:
        raise InstallCheckError(
            "disabled runtime-provider package contains an empty catalog artifact"
        )
    raise InstallCheckError(
        "disabled runtime-provider package contains a fixed catalog artifact"
    )


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Fail closed when an enabled runtime-provider catalog lacks the "
            "fixed broker executable or exact installed public catalog."
        )
    )
    parser.add_argument(
        "--platform",
        required=True,
        choices=sorted(LAYOUTS),
        help="deployment platform whose fixed V1 paths must be checked",
    )
    parser.add_argument(
        "--install-root",
        type=Path,
        default=Path("/"),
        help="absolute live or staged filesystem root (default: /)",
    )
    providers = parser.add_mutually_exclusive_group(required=True)
    providers.add_argument(
        "--expected-catalog",
        type=Path,
        help=(
            "exported secret-free non-empty catalog whose exact bytes must be "
            "installed at the fixed V1 path"
        ),
    )
    providers.add_argument(
        "--runtime-providers-disabled",
        action="store_true",
        help="record that this package intentionally has no runtime-provider catalog",
    )
    parser.add_argument(
        "--check-runtime-directory",
        action="store_true",
        help=(
            "also require the fixed Linux 0700 runtime directory to exist under "
            "the install root with the service UID/GID; macOS always requires it"
        ),
    )
    parser.add_argument(
        "--expected-executable-sha256",
        help=(
            "canonical lowercase SHA-256 of the broker executable from the "
            "externally verified signed release provenance"
        ),
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.runtime_providers_disabled:
        try:
            validate_disabled_install(
                layout=LAYOUTS[args.platform], install_root=args.install_root
            )
        except InstallCheckError as error:
            print(
                f"runtime-provider broker install check failed: {error}",
                file=sys.stderr,
            )
            return 1
        print("runtime-provider broker: disabled; no broker package required")
        return 0
    if args.expected_executable_sha256 is None:
        print(
            "runtime-provider broker install check failed: "
            "--expected-executable-sha256 is required when providers are enabled",
            file=sys.stderr,
        )
        return 1
    try:
        service_uid = pwd.getpwnam(SERVICE_USER).pw_uid
        service_gid = grp.getgrnam(SERVICE_GROUP).gr_gid
    except KeyError:
        print(
            "runtime-provider broker install check failed: "
            "required service identity iroha:iroha is not installed",
            file=sys.stderr,
        )
        return 1
    try:
        validate_install(
            layout=LAYOUTS[args.platform],
            install_root=args.install_root,
            expected_catalog=args.expected_catalog,
            expected_executable_sha256=args.expected_executable_sha256,
            service_uid=service_uid,
            service_gid=service_gid,
            trusted_artifact_owner_uid=0,
            check_runtime_directory=args.check_runtime_directory,
        )
    except InstallCheckError as error:
        print(f"runtime-provider broker install check failed: {error}", file=sys.stderr)
        return 1
    layout = LAYOUTS[args.platform]
    print(
        "runtime-provider broker install check passed: "
        f"catalog={layout.catalog} executable={layout.executable} "
        f"socket={layout.socket}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
