#!/usr/bin/env python3
"""Measure one verified, production-entitled App Attest capture bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import stat
import subprocess
import sys
import tempfile


SCHEMA = "iroha.kagemusha.ios.app_attest_capture_code_sign_measurements.v1"
TEAM_RE = re.compile(r"[A-Z0-9]{10}")
BUNDLE_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9.-]{0,254}")
CDHASH_RE = re.compile(r"[0-9a-f]{40}")
MAX_PLIST_BYTES = 1024 * 1024
MAX_EXECUTABLE_BYTES = 64 * 1024 * 1024
MAX_CODESIGN_DIAGNOSTIC_BYTES = 64 * 1024


class MeasurementError(RuntimeError):
    """Raised when the bundle measurement cannot be trusted."""


class MeasurementPublicationUncertain(MeasurementError):
    """Raised after an output may have reached its final name."""


def _identity(metadata: os.stat_result) -> tuple[int, ...]:
    """Return metadata fields that must remain stable during measurement."""

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


def _regular_bytes(path: Path, label: str, maximum: int) -> bytes:
    try:
        before = path.lstat()
    except OSError as error:
        raise MeasurementError(f"{label} metadata is unavailable") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink != 1
        or before.st_uid != os.geteuid()
        or not 1 <= before.st_size <= maximum
        or stat.S_IMODE(before.st_mode) & 0o022
    ):
        raise MeasurementError(
            f"{label} must be a bounded, non-writable, singly linked regular file"
        )
    flags = os.O_RDONLY
    if hasattr(os, "O_CLOEXEC"):
        flags |= os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            raise MeasurementError(f"{label} changed while opening")
        chunks: list[bytes] = []
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                raise MeasurementError(f"{label} ended before its declared size")
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(descriptor)
        if (
            (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
            != (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns)
        ):
            raise MeasurementError(f"{label} changed while reading")
        return b"".join(chunks)
    finally:
        os.close(descriptor)


def _codesign_details(app: Path) -> dict[str, str]:
    try:
        completed = subprocess.run(
            ["/usr/bin/codesign", "-dvv", str(app)],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise MeasurementError("codesign inspection did not complete") from error
    if completed.returncode != 0 or len(completed.stderr) > MAX_CODESIGN_DIAGNOSTIC_BYTES:
        raise MeasurementError("codesign inspection failed or exceeded its bound")
    try:
        lines = completed.stderr.decode("utf-8", errors="strict").splitlines()
    except UnicodeDecodeError as error:
        raise MeasurementError("codesign diagnostics are not UTF-8") from error
    result: dict[str, str] = {}
    for line in lines:
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        if key in {"CDHash", "Identifier", "TeamIdentifier"}:
            if key in result:
                raise MeasurementError(f"codesign repeated {key}")
            result[key] = value
    if set(result) != {"CDHash", "Identifier", "TeamIdentifier"}:
        raise MeasurementError("codesign omitted an exact identity field")
    return result


def measure_bundle(
    app: Path,
    entitlements_path: Path,
    expected_team: str,
    expected_bundle: str,
) -> dict[str, object]:
    """Return the canonical security-relevant measurement object."""

    if TEAM_RE.fullmatch(expected_team) is None:
        raise MeasurementError("expected Team ID is not canonical")
    if (
        BUNDLE_RE.fullmatch(expected_bundle) is None
        or ".." in expected_bundle
        or expected_bundle.endswith(".")
    ):
        raise MeasurementError("expected bundle ID is not canonical")
    try:
        app_metadata = app.lstat()
    except OSError as error:
        raise MeasurementError("capture app metadata is unavailable") from error
    if (
        stat.S_ISLNK(app_metadata.st_mode)
        or not stat.S_ISDIR(app_metadata.st_mode)
        or app_metadata.st_uid != os.geteuid()
        or stat.S_IMODE(app_metadata.st_mode) & 0o022
    ):
        raise MeasurementError("capture app must be an owner-custodied non-writable directory")
    info_payload = _regular_bytes(app / "Info.plist", "capture app Info.plist", MAX_PLIST_BYTES)
    entitlements_payload = _regular_bytes(
        entitlements_path, "capture app signed entitlements", MAX_PLIST_BYTES
    )
    try:
        info = plistlib.loads(info_payload)
        entitlements = plistlib.loads(entitlements_payload)
    except (plistlib.InvalidFileException, ValueError, TypeError) as error:
        raise MeasurementError("capture app property list is invalid") from error
    if not isinstance(info, dict) or not isinstance(entitlements, dict):
        raise MeasurementError("capture app property lists must be dictionaries")
    executable_name = info.get("CFBundleExecutable")
    bundle_version = info.get("CFBundleVersion")
    if (
        not isinstance(executable_name, str)
        or not executable_name
        or "/" in executable_name
        or not isinstance(bundle_version, str)
        or not bundle_version
    ):
        raise MeasurementError("capture app Info.plist identity is invalid")
    if info.get("CFBundleIdentifier") != expected_bundle:
        raise MeasurementError("capture app bundle ID differs from the requested identity")
    application_id = f"{expected_team}.{expected_bundle}"
    if (
        entitlements.get("application-identifier") != application_id
        or entitlements.get("com.apple.developer.team-identifier") != expected_team
        or entitlements.get("com.apple.developer.devicecheck.appattest-environment")
        != "production"
    ):
        raise MeasurementError("capture app signed entitlements are not exact production identity")
    executable_payload = _regular_bytes(
        app / executable_name, "capture app executable", MAX_EXECUTABLE_BYTES
    )
    details = _codesign_details(app)
    cdhash = details["CDHash"].lower()
    if (
        details["Identifier"] != expected_bundle
        or details["TeamIdentifier"] != expected_team
        or CDHASH_RE.fullmatch(cdhash) is None
    ):
        raise MeasurementError("capture app codesign identity is not exact")
    if (
        _regular_bytes(
            app / "Info.plist", "capture app Info.plist", MAX_PLIST_BYTES
        )
        != info_payload
        or _regular_bytes(
            entitlements_path,
            "capture app signed entitlements",
            MAX_PLIST_BYTES,
        )
        != entitlements_payload
        or _regular_bytes(
            app / executable_name,
            "capture app executable",
            MAX_EXECUTABLE_BYTES,
        )
        != executable_payload
    ):
        raise MeasurementError("capture app inputs changed during measurement")
    try:
        final_app_metadata = app.lstat()
    except OSError as error:
        raise MeasurementError("capture app changed during measurement") from error
    if _identity(final_app_metadata) != _identity(app_metadata):
        raise MeasurementError("capture app changed during measurement")
    return {
        "schema": SCHEMA,
        "version": 1,
        "bundle_id": expected_bundle,
        "bundle_version": bundle_version,
        "team_id": expected_team,
        "application_identifier": application_id,
        "app_attest_environment": "production",
        "executable_sha256": hashlib.sha256(executable_payload).hexdigest(),
        "cdhash": cdhash,
    }


def _write_new_private_json(path: Path, value: dict[str, object]) -> None:
    payload = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii") + b"\n"
    descriptor, temporary_text = tempfile.mkstemp(
        prefix=f".{path.name}.", dir=os.fspath(path.parent)
    )
    temporary = Path(temporary_text)
    created_identity: tuple[int, int] | None = None
    directory_descriptor = -1
    link_attempted = False
    linked = False
    operation_error: OSError | None = None
    try:
        os.fchmod(descriptor, 0o600)
        opened = os.fstat(descriptor)
        created_identity = (opened.st_dev, opened.st_ino)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                raise OSError("short capture-app measurement write")
            offset += written
        os.fsync(descriptor)
        closing = descriptor
        descriptor = -1
        os.close(closing)
        link_attempted = True
        os.link(temporary, path, follow_symlinks=False)
        linked = True
        directory_flags = os.O_RDONLY
        if hasattr(os, "O_DIRECTORY"):
            directory_flags |= os.O_DIRECTORY
        directory_descriptor = os.open(path.parent, directory_flags)
        os.fsync(directory_descriptor)
        temporary.unlink()
        os.fsync(directory_descriptor)
        closing = directory_descriptor
        directory_descriptor = -1
        os.close(closing)
        published = path.lstat()
        if (
            (published.st_dev, published.st_ino) != created_identity
            or not stat.S_ISREG(published.st_mode)
            or published.st_nlink != 1
            or published.st_uid != os.geteuid()
            or stat.S_IMODE(published.st_mode) != 0o600
            or published.st_size != len(payload)
        ):
            raise OSError("capture-app measurement output changed while writing")
        flags = os.O_RDONLY
        if hasattr(os, "O_CLOEXEC"):
            flags |= os.O_CLOEXEC
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        published_descriptor = os.open(path, flags)
        try:
            opened = os.fstat(published_descriptor)
            if (
                _identity(opened) != _identity(published)
                or (opened.st_dev, opened.st_ino) != created_identity
            ):
                raise OSError(
                    "capture-app measurement output changed while opening"
                )
            digest = hashlib.sha256()
            chunks: list[bytes] = []
            remaining = len(payload)
            while remaining:
                chunk = os.read(published_descriptor, min(1024 * 1024, remaining))
                if not chunk:
                    raise OSError(
                        "capture-app measurement output ended before its declared size"
                    )
                digest.update(chunk)
                chunks.append(chunk)
                remaining -= len(chunk)
            if os.read(published_descriptor, 1):
                raise OSError("capture-app measurement output grew while reading")
            after_open = os.fstat(published_descriptor)
            after_path = path.lstat()
            if (
                _identity(after_open) != _identity(opened)
                or _identity(after_path) != _identity(opened)
                or b"".join(chunks) != payload
                or digest.digest() != hashlib.sha256(payload).digest()
            ):
                raise OSError(
                    "capture-app measurement output bytes or identity changed"
                )
        finally:
            os.close(published_descriptor)
    except OSError as error:
        operation_error = error

    cleanup_error: OSError | None = None
    for remaining in (descriptor, directory_descriptor):
        if remaining < 0:
            continue
        try:
            os.close(remaining)
        except OSError as error:
            if cleanup_error is None:
                cleanup_error = error
    try:
        temporary.unlink()
    except FileNotFoundError:
        pass
    except OSError as error:
        if cleanup_error is None:
            cleanup_error = error

    error = operation_error if operation_error is not None else cleanup_error
    if error is not None:
        if created_identity is not None and (
            linked or (link_attempted and not isinstance(error, FileExistsError))
        ):
            raise MeasurementPublicationUncertain(
                "measurement output publication commit state is uncertain; "
                "no final name was removed"
            ) from error
        if isinstance(error, FileExistsError):
            raise error
        raise error


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True)
    parser.add_argument("--signed-entitlements", required=True)
    parser.add_argument("--development-team", required=True)
    parser.add_argument("--bundle-id", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    try:
        value = measure_bundle(
            Path(args.app),
            Path(args.signed_entitlements),
            args.development_team,
            args.bundle_id,
        )
        _write_new_private_json(Path(args.output), value)
    except MeasurementPublicationUncertain as error:
        print(f"[kagemusha-app-attest-measure] ERROR: {error}", file=sys.stderr)
        return 75
    except (OSError, MeasurementError) as error:
        print(f"[kagemusha-app-attest-measure] ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
