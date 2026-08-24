#!/usr/bin/env python3
"""Fail-closed verifier for the ABI-22 bridge embedded in an APK or AAB."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
from typing import NoReturn
import zipfile


ABIS = ("arm64-v8a", "x86_64")
LIBRARY_NAME = "libconnect_norito_bridge.so"
PROVENANCE_NAME = "native-build-provenance-v1.json"
PROVENANCE_RELATIVE_PATH = Path("iroha") / PROVENANCE_NAME
CLIENT_BUILD_RELATIVE_PATH = (
    Path("gradle-build") / "iroha_kotlin_sdk" / "client-android"
)
MAX_PROVENANCE_BYTES = 64 * 1024
BUILD_ENVIRONMENT_SCHEMA = "iroha.mobile-native-build-environment.v1"
ANDROID_NDK_BASE_REVISION = "28.0.12674087"
ANDROID_NDK_SOURCE_PROPERTIES_SHA256 = (
    "55368a3554d27b8413b75a4b2e83ea7f6b66fef4068f7a7f71cf2910c6e3357b"
)


class VerificationError(RuntimeError):
    pass


def fail(message: str) -> NoReturn:
    raise VerificationError(message)


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def strict_object(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def canonical_existing_path(raw: str, *, directory: bool, label: str) -> Path:
    candidate = Path(raw)
    if not candidate.is_absolute() or str(candidate) != raw:
        fail(f"{label} must be an absolute normalized path")
    try:
        resolved = candidate.resolve(strict=True)
        metadata = candidate.lstat()
    except OSError as error:
        fail(f"{label} is missing or unreadable: {error}")
    if resolved != candidate or stat.S_ISLNK(metadata.st_mode):
        fail(f"{label} must not traverse a symbolic link")
    if directory and not stat.S_ISDIR(metadata.st_mode):
        fail(f"{label} must be a directory")
    if not directory and not stat.S_ISREG(metadata.st_mode):
        fail(f"{label} must be a regular file")
    return resolved


def require_inside(path: Path, root: Path, label: str) -> None:
    try:
        path.relative_to(root)
    except ValueError:
        fail(f"{label} escapes the authenticated artifact root")
    current = path
    while current != root:
        try:
            if current.is_symlink():
                fail(f"{label} traverses a symbolic link: {current}")
        except OSError as error:
            fail(f"{label} is unreadable: {error}")
        current = current.parent


def read_regular(path: Path, root: Path, label: str) -> bytes:
    require_inside(path, root, label)
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        fail(f"{label} is missing or unreadable: {error}")
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            fail(f"{label} must be a non-symbolic regular file")
        chunks: list[bytes] = []
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            chunks.append(chunk)
        payload = b"".join(chunks)
        after = os.fstat(descriptor)
        if (
            (before.st_dev, before.st_ino, before.st_size)
            != (after.st_dev, after.st_ino, after.st_size)
            or len(payload) != before.st_size
        ):
            fail(f"{label} changed while it was being authenticated")
        return payload
    finally:
        os.close(descriptor)


def validate_zip_inventory(archive: zipfile.ZipFile, label: str) -> dict[str, zipfile.ZipInfo]:
    infos = archive.infolist()
    names = [info.filename for info in infos]
    if len(names) != len(set(names)):
        fail(f"{label} contains duplicate ZIP entries")
    result: dict[str, zipfile.ZipInfo] = {}
    for info in infos:
        name = info.filename
        pure = PurePosixPath(name)
        canonical_name = pure.as_posix() + ("/" if info.is_dir() else "")
        if (
            not name
            or name.startswith("/")
            or "\\" in name
            or ".." in pure.parts
            or name != canonical_name
        ):
            fail(f"{label} contains a non-canonical ZIP path: {name!r}")
        if stat.S_ISLNK(info.external_attr >> 16):
            fail(f"{label} contains a symbolic-link ZIP entry: {name}")
        result[name] = info
    return result


def require_exact_native_library_inventory(
    inventory: dict[str, zipfile.ZipInfo],
    *,
    root: str,
    label: str,
) -> None:
    if not root.endswith("/"):
        raise AssertionError("native-library root must end with '/'")
    root_parts = PurePosixPath(root).parts
    expected_abis = set(ABIS)
    expected_files = {
        f"{root}{abi}/{LIBRARY_NAME}"
        for abi in ABIS
    }
    actual_abis: set[str] = set()
    actual_files: set[str] = set()

    for name, info in inventory.items():
        parts = PurePosixPath(name).parts
        inside_root = parts[: len(root_parts)] == root_parts
        if not inside_root:
            if not info.is_dir() and name.endswith(".so"):
                fail(
                    f"{label} contains a native library outside the exact "
                    f"{root} root: {name}"
                )
            continue

        relative_parts = parts[len(root_parts):]
        if not relative_parts:
            if not info.is_dir():
                fail(f"{label} native-library root is not a directory: {name}")
            continue

        abi = relative_parts[0]
        actual_abis.add(abi)
        if len(relative_parts) == 1:
            if not info.is_dir():
                fail(f"{label} ABI path is not a directory: {name}")
            continue
        if len(relative_parts) != 2 or info.is_dir():
            fail(f"{label} contains a malformed or nested native-library entry: {name}")
        actual_files.add(name)

    if actual_abis != expected_abis:
        fail(
            f"{label} native ABI directory inventory is not exact: "
            f"{sorted(actual_abis)}"
        )
    if actual_files != expected_files:
        fail(
            f"{label} native library inventory is not exact: "
            f"{sorted(actual_files)}"
        )


def read_zip_entry(
    archive: zipfile.ZipFile,
    inventory: dict[str, zipfile.ZipInfo],
    name: str,
    label: str,
) -> bytes:
    info = inventory.get(name)
    if info is None:
        fail(f"{label} is missing ZIP entry {name}")
    try:
        payload = archive.read(info)
    except (OSError, RuntimeError, zipfile.BadZipFile) as error:
        fail(f"{label} could not read ZIP entry {name}: {error}")
    if len(payload) != info.file_size:
        fail(f"{label} ZIP entry changed while being read: {name}")
    return payload


def strict_provenance(payload: bytes) -> dict[str, object]:
    if len(payload) < 2 or len(payload) > MAX_PROVENANCE_BYTES:
        fail("native provenance must contain 2..65536 bytes")
    try:
        decoded = json.loads(
            payload.decode("utf-8"),
            object_pairs_hook=strict_object,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"native provenance is not strict UTF-8 JSON: {error}")
    if not isinstance(decoded, dict):
        fail("native provenance root must be an object")
    expected_fields = {
        "schema",
        "native_bridge_abi_version",
        "build_profile",
        "cargo_locked",
        "privacy_production_enabled",
        "cargo_features",
        "build_environment",
        "source_commit",
        "source_tree_dirty",
        "source_fingerprint_sha256",
        "cargo_lock_sha256",
        "android_ndk_revision",
        "strip_tool_sha256",
        "libraries",
    }
    if set(decoded) != expected_fields:
        fail("native provenance field inventory is not exact")
    if (
        decoded["schema"] != "iroha.android-native-build-provenance.v1"
        or type(decoded["native_bridge_abi_version"]) is not int
        or decoded["native_bridge_abi_version"] != 23
        or decoded["build_profile"] != "release"
        or decoded["cargo_locked"] is not True
        or decoded["privacy_production_enabled"] is not True
        or decoded["cargo_features"] != ["privacy-production-enabled"]
    ):
        fail("native provenance does not bind the production ABI-23 release")
    if decoded["android_ndk_revision"] != ANDROID_NDK_BASE_REVISION:
        fail("native provenance Android NDK base revision is not exact")
    build_environment = decoded["build_environment"]
    if not isinstance(build_environment, dict):
        fail("native provenance build environment must be an object")
    if build_environment.get("schema") != BUILD_ENVIRONMENT_SCHEMA:
        fail("native provenance build environment schema is not exact")
    if (
        build_environment.get("android_ndk_revision")
        != ANDROID_NDK_BASE_REVISION
        or build_environment["android_ndk_revision"]
        != decoded["android_ndk_revision"]
    ):
        fail("native provenance build environment Android NDK base revision is not exact")
    if (
        build_environment.get("android_ndk_source_properties_sha256")
        != ANDROID_NDK_SOURCE_PROPERTIES_SHA256
    ):
        fail("native provenance Android NDK source.properties digest is not exact")
    libraries = decoded["libraries"]
    if not isinstance(libraries, dict) or set(libraries) != set(ABIS):
        fail("native provenance ABI inventory is not exact")
    return decoded


def exact_generated_libraries(
    client_build_root: Path,
    artifact_root: Path,
) -> dict[str, bytes]:
    generated_root = client_build_root / "generated/jniLibs/production"
    require_inside(generated_root, artifact_root, "generated JNI root")
    if generated_root.is_symlink() or not generated_root.is_dir():
        fail("generated JNI root must be a non-symbolic directory")
    expected_files = {
        f"{abi}/{LIBRARY_NAME}"
        for abi in ABIS
    }
    actual_files: set[str] = set()
    actual_directories: set[str] = set()
    for current, directories, files in os.walk(
        generated_root,
        topdown=True,
        followlinks=False,
    ):
        current_path = Path(current)
        if current_path.is_symlink():
            fail(f"generated JNI tree traverses a symbolic link: {current_path}")
        for name in directories:
            child = current_path / name
            if child.is_symlink():
                fail(f"generated JNI tree traverses a symbolic link: {child}")
            actual_directories.add(child.relative_to(generated_root).as_posix())
        for name in files:
            child = current_path / name
            if not stat.S_ISREG(child.lstat().st_mode):
                fail(f"generated JNI tree contains a non-regular entry: {child}")
            actual_files.add(child.relative_to(generated_root).as_posix())
    if actual_files != expected_files or actual_directories != set(ABIS):
        fail("generated JNI tree inventory is not exact")
    return {
        abi: read_regular(
            generated_root / abi / LIBRARY_NAME,
            artifact_root,
            f"generated {abi} bridge",
        )
        for abi in ABIS
    }


def verify(args: argparse.Namespace) -> None:
    artifact_root = canonical_existing_path(
        args.artifact_dir,
        directory=True,
        label="MOBILE_SDK_ANDROID_ARTIFACT_DIR",
    )
    iroha_root = canonical_existing_path(
        args.iroha_root,
        directory=True,
        label="Iroha source root",
    )
    if artifact_root == iroha_root or iroha_root in artifact_root.parents:
        fail("Android artifact root must be outside the Iroha source tree")
    archive_path = canonical_existing_path(
        args.archive,
        directory=False,
        label=f"Android {args.kind.upper()}",
    )
    client_build_root = artifact_root / CLIENT_BUILD_RELATIVE_PATH
    generated_by_abi = exact_generated_libraries(client_build_root, artifact_root)
    provenance_path = (
        client_build_root
        / "generated/nativeProvenance/production"
        / PROVENANCE_RELATIVE_PATH
    )
    provenance_bytes = read_regular(
        provenance_path,
        artifact_root,
        "generated native provenance",
    )
    provenance = strict_provenance(provenance_bytes)
    aar_path = client_build_root / "outputs/aar/client-android-release.aar"
    aar_bytes = read_regular(
        aar_path,
        artifact_root,
        "authenticated client-android Release AAR",
    )

    try:
        aar = zipfile.ZipFile(aar_path)
    except (OSError, zipfile.BadZipFile) as error:
        fail(f"client-android Release AAR is unreadable: {error}")
    with aar:
        aar_inventory = validate_zip_inventory(aar, "client-android Release AAR")
        require_exact_native_library_inventory(
            aar_inventory,
            root="jni/",
            label="client-android Release AAR",
        )
        expected_aar_bridges = {
            f"jni/{abi}/{LIBRARY_NAME}"
            for abi in ABIS
        }
        actual_aar_bridges = {
            name
            for name in aar_inventory
            if PurePosixPath(name).name == LIBRARY_NAME
        }
        if actual_aar_bridges != expected_aar_bridges:
            fail("client-android Release AAR bridge inventory is not exact")
        aar_provenance_name = "assets/iroha/" + PROVENANCE_NAME
        actual_aar_provenance = {
            name
            for name in aar_inventory
            if PurePosixPath(name).name == PROVENANCE_NAME
        }
        if actual_aar_provenance != {aar_provenance_name}:
            fail("client-android Release AAR provenance inventory is not exact")
        if read_zip_entry(
            aar,
            aar_inventory,
            aar_provenance_name,
            "client-android Release AAR",
        ) != provenance_bytes:
            fail("client-android Release AAR provenance differs from generated provenance")
        for abi in ABIS:
            if read_zip_entry(
                aar,
                aar_inventory,
                f"jni/{abi}/{LIBRARY_NAME}",
                "client-android Release AAR",
            ) != generated_by_abi[abi]:
                fail(f"client-android Release AAR {abi} bridge differs from generated bytes")

    try:
        app_archive = zipfile.ZipFile(archive_path)
    except (OSError, zipfile.BadZipFile) as error:
        fail(f"Android {args.kind.upper()} is unreadable: {error}")
    prefix = "base/" if args.kind == "aab" else ""
    with app_archive:
        inventory = validate_zip_inventory(app_archive, f"Android {args.kind.upper()}")
        require_exact_native_library_inventory(
            inventory,
            root=f"{prefix}lib/",
            label=f"Android {args.kind.upper()}",
        )
        expected_bridges = {
            f"{prefix}lib/{abi}/{LIBRARY_NAME}"
            for abi in ABIS
        }
        actual_bridges = {
            name
            for name in inventory
            if PurePosixPath(name).name == LIBRARY_NAME
        }
        if actual_bridges != expected_bridges:
            fail(
                f"Android {args.kind.upper()} bridge inventory is not exact: "
                f"{sorted(actual_bridges)}"
            )
        provenance_entry = f"{prefix}assets/iroha/{PROVENANCE_NAME}"
        actual_provenance = {
            name
            for name in inventory
            if PurePosixPath(name).name == PROVENANCE_NAME
        }
        if actual_provenance != {provenance_entry}:
            fail(f"Android {args.kind.upper()} provenance inventory is not exact")
        if read_zip_entry(
            app_archive,
            inventory,
            provenance_entry,
            f"Android {args.kind.upper()}",
        ) != provenance_bytes:
            fail(f"Android {args.kind.upper()} provenance differs from authenticated bytes")

        libraries = provenance["libraries"]
        assert isinstance(libraries, dict)
        for abi in ABIS:
            record = libraries[abi]
            if not isinstance(record, dict) or set(record) != {
                "aar_path",
                "bytes",
                "raw_bytes",
                "raw_sha256",
                "sha256",
            }:
                fail(f"native provenance {abi} library record is not exact")
            generated = generated_by_abi[abi]
            if (
                record["aar_path"] != f"jni/{abi}/{LIBRARY_NAME}"
                or type(record["bytes"]) is not int
                or record["bytes"] != len(generated)
                or record["sha256"] != sha256(generated)
            ):
                fail(f"native provenance does not authenticate generated {abi} bridge")
            embedded = read_zip_entry(
                app_archive,
                inventory,
                f"{prefix}lib/{abi}/{LIBRARY_NAME}",
                f"Android {args.kind.upper()}",
            )
            if embedded != generated:
                fail(
                    f"Android {args.kind.upper()} {abi} bridge differs from the "
                    "authenticated generated/AAR bytes"
                )

    # Retain a live reference until every ZIP comparison has completed.
    if not aar_bytes:
        fail("authenticated client-android Release AAR is empty")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--archive", required=True)
    parser.add_argument("--kind", choices=("aab", "apk"), required=True)
    parser.add_argument("--artifact-dir", required=True)
    parser.add_argument("--iroha-root", required=True)
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    try:
        verify(parse_args(argv))
    except VerificationError as error:
        print(f"[android-native-archive] ERROR: {error}", file=sys.stderr)
        return 1
    print("[android-native-archive] authenticated ABI-23 bridge bytes match SDK AAR and app archive")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
