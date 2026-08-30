#!/usr/bin/env python3
"""Run one mobile native-build command with an exact environment.

This launcher is shared by the Apple, Android, and host-JNI build gates.  Its
profiles are deliberately closed inventories: a caller must provide every
declared variable and cannot add undeclared variables.  In particular, ambient
Cargo/Rust compiler flags and wrapper variables never reach the child process.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import stat
import subprocess
import sys


COMMON_CARGO_ENVIRONMENT = frozenset(
    {
        "CARGO",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_NET_OFFLINE",
        "CARGO_TARGET_DIR",
        "HOME",
        "LANG",
        "LC_ALL",
        "NORITO_SKIP_BINDINGS_SYNC",
        "PATH",
        "RUSTC",
        "RUSTUP_HOME",
        "TMPDIR",
    }
)
APPLE_CARGO_ENVIRONMENT = COMMON_CARGO_ENVIRONMENT | {
    "CARGO_BUILD_JOBS",
    "RUSTC_BOOTSTRAP",
    "RUSTDOC",
}
ANDROID_CARGO_ENVIRONMENT = APPLE_CARGO_ENVIRONMENT | {
    "ANDROID_NDK_HOME",
    "ANDROID_NDK_ROOT",
}
PRIVACY_CARGO_CORRIDOR_ENVIRONMENT = frozenset(
    {
        "IROHA_PRIVACY_CARGO_LOCKFILE_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_LOCKFILE_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_WORKSPACE_CARGO_LOCK_STATE",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_CONFIG_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME_DIRECTORY_STATE",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_REGISTRY_LINK_STATE",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_GIT_LINK_STATE",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIRECTORY_STATE",
        "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR",
        "IROHA_PRIVACY_AUTHENTICATED_RUSTUP_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_RUSTUP_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_RUSTC_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_RUSTC_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_RUSTDOC_SEAL",
        "IROHA_PRIVACY_SDK_ROOT",
        "IROHA_PRIVACY_REAL_CARGO",
        "IROHA_PRIVACY_CARGO_AUDIT_PATH",
        "IROHA_PRIVACY_LOCKFILE_PYTHON_BIN",
        "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_PATH",
        "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_SEAL",
        "IROHA_PRIVACY_AUTHENTICATED_APPLE_CARGO_PROFILE",
        "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGET",
        "IROHA_PRIVACY_AUTHENTICATED_DEVELOPER_DIR",
        "IROHA_PRIVACY_AUTHENTICATED_SDKROOT",
    }
)
PRIVACY_WRAPPED_CARGO_ENVIRONMENT = (
    (COMMON_CARGO_ENVIRONMENT - {"RUSTC", "RUSTUP_HOME"})
    | {
        "CARGO_BUILD_JOBS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_BOOTSTRAP",
    }
    | PRIVACY_CARGO_CORRIDOR_ENVIRONMENT
)
APPLE_TARGETS = (
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
)
PRIVACY_APPLE_PROFILE_SPECS: dict[str, dict[str, object]] = {
    "privacy-apple-ios-device-arm64": {
        "target": "aarch64-apple-ios",
        "platform": "iPhoneOS",
        "environment": {"IPHONEOS_DEPLOYMENT_TARGET": "15.0"},
    },
    "privacy-apple-ios-simulator-arm64": {
        "target": "aarch64-apple-ios-sim",
        "platform": "iPhoneSimulator",
        "environment": {
            "IPHONEOS_DEPLOYMENT_TARGET": "15.0",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET": "15.0",
        },
    },
    "privacy-apple-ios-simulator-x86_64": {
        "target": "x86_64-apple-ios",
        "platform": "iPhoneSimulator",
        "environment": {
            "IPHONEOS_DEPLOYMENT_TARGET": "15.0",
            "IPHONESIMULATOR_DEPLOYMENT_TARGET": "15.0",
        },
    },
    "privacy-apple-macos-arm64": {
        "target": "aarch64-apple-darwin",
        "platform": "MacOSX",
        "environment": {"MACOSX_DEPLOYMENT_TARGET": "12.0"},
    },
    "privacy-apple-macos-x86_64": {
        "target": "x86_64-apple-darwin",
        "platform": "MacOSX",
        "environment": {"MACOSX_DEPLOYMENT_TARGET": "12.0"},
    },
}
GRADLE_JVM_ENVIRONMENT = frozenset(
    {
        "ANDROID_HOME",
        "ANDROID_SDK_ROOT",
        "DYLD_LIBRARY_PATH",
        "GRADLE_USER_HOME",
        "HOME",
        "IROHA_NATIVE_LIBRARY_PATH",
        "IROHA_REQUIRE_KAGEMUSHA_NATIVE",
        "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION",
        "JAVA_HOME",
        "LANG",
        "LC_ALL",
        "LD_LIBRARY_PATH",
        "PATH",
        "TMPDIR",
    }
)
AUTHENTICATED_CARGO_PROFILES = frozenset(
    {
        "android-cargo",
        "apple-ios-device",
        "apple-ios-simulator",
        "apple-macos",
    }
)
PROFILES = {
    "apple-ios-device": APPLE_CARGO_ENVIRONMENT
    | {
        "DEVELOPER_DIR",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "SDKROOT",
    },
    "apple-ios-simulator": APPLE_CARGO_ENVIRONMENT
    | {
        "DEVELOPER_DIR",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "IPHONESIMULATOR_DEPLOYMENT_TARGET",
        "SDKROOT",
    },
    "apple-macos": APPLE_CARGO_ENVIRONMENT
    | {
        "DEVELOPER_DIR",
        "MACOSX_DEPLOYMENT_TARGET",
        "SDKROOT",
    },
    "android-cargo": ANDROID_CARGO_ENVIRONMENT,
    "host-cargo": COMMON_CARGO_ENVIRONMENT,
    "gradle-jvm": GRADLE_JVM_ENVIRONMENT,
    "gradle-jvm-localnet": GRADLE_JVM_ENVIRONMENT
    | {
        "IROHA_LOCALNET_DIR",
        "IROHA_LOCALNET_TEST",
    },
}
for _profile_name, _profile_spec in PRIVACY_APPLE_PROFILE_SPECS.items():
    PROFILES[_profile_name] = PRIVACY_WRAPPED_CARGO_ENVIRONMENT | {
        "DEVELOPER_DIR",
        "SDKROOT",
        *dict(_profile_spec["environment"]),
    }
PRIVACY_WRAPPED_APPLE_PROFILES = frozenset(PRIVACY_APPLE_PROFILE_SPECS)


def parse_assignment(raw: str) -> tuple[str, str]:
    if "=" not in raw:
        raise argparse.ArgumentTypeError("--set requires NAME=VALUE")
    name, value = raw.split("=", 1)
    if not name or any(character not in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_" for character in name):
        raise argparse.ArgumentTypeError(f"invalid environment variable name: {name!r}")
    if "\0" in value:
        raise argparse.ArgumentTypeError(f"{name} contains a NUL byte")
    return name, value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", choices=sorted(PROFILES), required=True)
    parser.add_argument(
        "--set",
        dest="assignments",
        action="append",
        default=[],
        type=parse_assignment,
        metavar="NAME=VALUE",
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("an executable and command arguments are required after --")
    return args


def parse_apple_target_args(action: str) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog=f"{sys.argv[0]} {action}")
    parser.add_argument("--toolchain-cargo", required=True, type=pathlib.Path)
    parser.add_argument("--toolchain-selector", required=True)
    if action == "seal-apple-targets":
        parser.add_argument("--output", required=True, type=pathlib.Path)
    else:
        parser.add_argument("--manifest", required=True, type=pathlib.Path)
        parser.add_argument("--manifest-seal", required=True)
        selection = parser.add_mutually_exclusive_group(required=True)
        selection.add_argument("--target", choices=APPLE_TARGETS)
        selection.add_argument("--all", action="store_true")
    return parser.parse_args(sys.argv[2:])


def authenticate_regular_executable(name: str, raw: str) -> tuple[pathlib.Path, tuple[int, ...]]:
    candidate = pathlib.Path(raw)
    if not candidate.is_absolute() or candidate != pathlib.Path(os.path.abspath(candidate)):
        raise RuntimeError(f"{name} must name an absolute canonical executable")
    try:
        metadata = candidate.lstat()
        resolved = candidate.resolve(strict=True)
        resolved_metadata = resolved.stat()
    except OSError as error:
        raise RuntimeError(f"{name} executable is unavailable: {candidate}") from error
    if (
        resolved != candidate
        or not candidate.is_file()
        or not os.access(candidate, os.X_OK)
        or metadata.st_mode != resolved_metadata.st_mode
    ):
        raise RuntimeError(f"{name} must name a non-symbolic regular executable: {candidate}")
    identity = (
        resolved_metadata.st_dev,
        resolved_metadata.st_ino,
        resolved_metadata.st_mode,
        resolved_metadata.st_size,
        resolved_metadata.st_mtime_ns,
    )
    return resolved, identity


def authenticate_regular_file(name: str, candidate: pathlib.Path) -> tuple[pathlib.Path, tuple[int, ...]]:
    if not candidate.is_absolute() or candidate != pathlib.Path(os.path.abspath(candidate)):
        raise RuntimeError(f"{name} must name an absolute canonical regular file")
    try:
        metadata = candidate.lstat()
        resolved = candidate.resolve(strict=True)
        resolved_metadata = resolved.stat()
    except OSError as error:
        raise RuntimeError(f"{name} is unavailable: {candidate}") from error
    if (
        resolved != candidate
        or not candidate.is_file()
        or metadata.st_mode != resolved_metadata.st_mode
    ):
        raise RuntimeError(f"{name} must name a non-symbolic regular file: {candidate}")
    identity = (
        resolved_metadata.st_dev,
        resolved_metadata.st_ino,
        resolved_metadata.st_mode,
        resolved_metadata.st_size,
        resolved_metadata.st_mtime_ns,
    )
    return resolved, identity


def _canonical_directory(name: str, candidate: pathlib.Path) -> pathlib.Path:
    if not candidate.is_absolute() or candidate != pathlib.Path(os.path.abspath(candidate)):
        raise RuntimeError(f"{name} must be an absolute canonical directory")
    try:
        metadata = candidate.lstat()
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise RuntimeError(f"{name} is unavailable: {candidate}") from error
    if resolved != candidate or not stat.S_ISDIR(metadata.st_mode):
        raise RuntimeError(f"{name} must be a non-symbolic canonical directory")
    return candidate


def _toolchain_root(cargo: pathlib.Path, selector: str) -> pathlib.Path:
    if selector != "1.93.1-aarch64-apple-darwin":
        raise RuntimeError("Apple target sealing requires the exact authenticated host selector")
    authenticated, _ = authenticate_regular_executable("authenticated Cargo", str(cargo))
    toolchain = authenticated.parent.parent
    if (
        authenticated.parent.name != "bin"
        or toolchain.name != selector
        or toolchain.parent.name != "toolchains"
    ):
        raise RuntimeError("authenticated Cargo is outside the selected Rust toolchain")
    return _canonical_directory("authenticated Rust toolchain", toolchain)


def _tree_sha256(root: pathlib.Path) -> tuple[str, int, int]:
    root = _canonical_directory("Apple Rust target sysroot", root)
    digest = hashlib.sha256()
    file_count = 0
    byte_count = 0
    for directory, directories, files in os.walk(root, topdown=True, followlinks=False):
        directory_path = pathlib.Path(directory)
        directories.sort()
        files.sort()
        relative_directory = directory_path.relative_to(root).as_posix()
        directory_metadata = directory_path.lstat()
        if not stat.S_ISDIR(directory_metadata.st_mode):
            raise RuntimeError(f"Apple Rust target tree contains a non-directory: {directory_path}")
        digest.update(b"directory\0")
        digest.update(relative_directory.encode("utf-8"))
        digest.update(b"\0")
        digest.update(f"{stat.S_IMODE(directory_metadata.st_mode):04o}".encode("ascii"))
        digest.update(b"\0")
        for name in [*directories, *files]:
            child = directory_path / name
            metadata = child.lstat()
            if stat.S_ISLNK(metadata.st_mode):
                raise RuntimeError(f"Apple Rust target tree contains a symbolic link: {child}")
        for name in files:
            child = directory_path / name
            metadata = child.lstat()
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
                raise RuntimeError(
                    f"Apple Rust target tree contains a non-regular or multiply linked file: {child}"
                )
            relative = child.relative_to(root).as_posix()
            digest.update(b"file\0")
            digest.update(relative.encode("utf-8"))
            digest.update(b"\0")
            digest.update(f"{stat.S_IMODE(metadata.st_mode):04o}".encode("ascii"))
            digest.update(b"\0")
            content_digest = hashlib.sha256()
            file_bytes = 0
            with child.open("rb") as handle:
                while chunk := handle.read(1024 * 1024):
                    content_digest.update(chunk)
                    file_bytes += len(chunk)
            digest.update(content_digest.hexdigest().encode("ascii"))
            digest.update(b"\0")
            digest.update(str(file_bytes).encode("ascii"))
            digest.update(b"\0")
            byte_count += file_bytes
            file_count += 1
    if file_count == 0:
        raise RuntimeError(f"Apple Rust target sysroot is empty: {root}")
    return digest.hexdigest(), file_count, byte_count


def _manifest_file_seal(path: pathlib.Path) -> str:
    authenticated, identity = authenticate_regular_file("Apple target manifest", path)
    metadata = authenticated.lstat()
    if (
        metadata.st_nlink != 1
        or stat.S_IMODE(metadata.st_mode) != 0o400
        or metadata.st_uid != os.geteuid()
        or metadata.st_size <= 0
        or metadata.st_size > 1024 * 1024
    ):
        raise RuntimeError(
            "Apple target manifest must be one owner-read-only, singly linked regular file"
        )
    digest = hashlib.sha256(authenticated.read_bytes()).hexdigest()
    return ":".join((digest, *(str(value) for value in identity)))


def _duplicates_rejected(pairs: list[tuple[str, object]]) -> dict[str, object]:
    result: dict[str, object] = {}
    for name, value in pairs:
        if name in result:
            raise RuntimeError(f"Apple target manifest contains duplicate key: {name}")
        result[name] = value
    return result


def _expected_target_records(
    cargo: pathlib.Path, selector: str, targets: tuple[str, ...] = APPLE_TARGETS
) -> dict[str, dict[str, object]]:
    toolchain = _toolchain_root(cargo, selector)
    records: dict[str, dict[str, object]] = {}
    for target in targets:
        if target not in APPLE_TARGETS:
            raise RuntimeError(f"unsupported authenticated Apple Rust target: {target}")
        sysroot = toolchain / "lib" / "rustlib" / target
        digest, file_count, byte_count = _tree_sha256(sysroot)
        records[target] = {
            "path": str(sysroot),
            "sha256": digest,
            "file_count": file_count,
            "byte_count": byte_count,
        }
    return records


def seal_apple_targets(
    *, cargo: pathlib.Path, selector: str, output: pathlib.Path
) -> str:
    if not output.is_absolute() or output != pathlib.Path(os.path.abspath(output)):
        raise RuntimeError("Apple target manifest output must be absolute and canonical")
    parent = _canonical_directory("Apple target manifest parent", output.parent)
    if output.exists() or output.is_symlink():
        raise RuntimeError("Apple target manifest output must not already exist")
    payload = {
        "schema": "iroha.privacy-sdk.apple-rust-targets.v1",
        "toolchain_selector": selector,
        "targets": _expected_target_records(cargo, selector),
    }
    encoded = (json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    descriptor = os.open(output, flags, 0o400)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as handle:
            handle.write(encoded)
            handle.flush()
            os.fsync(handle.fileno())
    finally:
        os.close(descriptor)
    parent_descriptor = os.open(parent, os.O_RDONLY)
    try:
        os.fsync(parent_descriptor)
    finally:
        os.close(parent_descriptor)
    return _manifest_file_seal(output)


def verify_apple_targets(
    *,
    cargo: pathlib.Path,
    selector: str,
    manifest: pathlib.Path,
    manifest_seal: str,
    targets: tuple[str, ...],
) -> None:
    if _manifest_file_seal(manifest) != manifest_seal:
        raise RuntimeError("authenticated Apple target manifest changed")
    try:
        payload = json.loads(
            manifest.read_text(encoding="utf-8"), object_pairs_hook=_duplicates_rejected
        )
    except (OSError, UnicodeError, ValueError, TypeError) as error:
        raise RuntimeError(f"Apple target manifest is not canonical JSON: {error}") from error
    if not isinstance(payload, dict) or set(payload) != {
        "schema",
        "toolchain_selector",
        "targets",
    }:
        raise RuntimeError("Apple target manifest field inventory is not exact")
    if (
        payload["schema"] != "iroha.privacy-sdk.apple-rust-targets.v1"
        or payload["toolchain_selector"] != selector
        or not isinstance(payload["targets"], dict)
        or set(payload["targets"]) != set(APPLE_TARGETS)
    ):
        raise RuntimeError("Apple target manifest authority is not exact")
    expected_records = _expected_target_records(cargo, selector, targets)
    for target in targets:
        if target not in APPLE_TARGETS:
            raise RuntimeError(f"unsupported authenticated Apple Rust target: {target}")
        if payload["targets"].get(target) != expected_records[target]:
            raise RuntimeError(f"authenticated Apple Rust target changed: {target}")


def authenticate_cargo_environment(
    environment: dict[str, str],
) -> dict[str, tuple[pathlib.Path, tuple[int, ...]]]:
    exact_values = {
        "CARGO_BUILD_JOBS": "1",
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "NORITO_SKIP_BINDINGS_SYNC": "1",
        "RUSTC_BOOTSTRAP": "1",
    }
    for name, expected in exact_values.items():
        if environment[name] != expected:
            raise RuntimeError(f"{name} must be exactly {expected!r}")

    target = pathlib.Path(environment["CARGO_TARGET_DIR"])
    if not target.is_absolute() or target != pathlib.Path(os.path.abspath(target)):
        raise RuntimeError("CARGO_TARGET_DIR must be an absolute canonical directory")
    try:
        metadata = target.lstat()
        resolved = target.resolve(strict=True)
    except OSError as error:
        raise RuntimeError(f"CARGO_TARGET_DIR is unavailable: {target}") from error
    if resolved != target or not target.is_dir() or metadata.st_mode != target.stat().st_mode:
        raise RuntimeError(
            f"CARGO_TARGET_DIR must be a non-symbolic canonical directory: {target}"
        )

    return {
        name: authenticate_regular_executable(name, environment[name])
        for name in ("CARGO", "RUSTC", "RUSTDOC")
    }


def authenticate_wrapped_apple_environment(
    profile: str,
    environment: dict[str, str],
    command: list[str],
) -> dict[str, tuple[pathlib.Path, tuple[int, ...]]]:
    spec = PRIVACY_APPLE_PROFILE_SPECS[profile]
    target = str(spec["target"])
    exact_values = {
        "CARGO_BUILD_JOBS": "1",
        "CARGO_ENCODED_RUSTFLAGS": "",
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "NORITO_SKIP_BINDINGS_SYNC": "1",
        "RUSTC_BOOTSTRAP": "1",
        "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR": (
            "1.93.1-aarch64-apple-darwin"
        ),
        "IROHA_PRIVACY_AUTHENTICATED_APPLE_CARGO_PROFILE": profile,
        "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGET": target,
    }
    exact_values.update(dict(spec["environment"]))
    for name, expected in exact_values.items():
        if environment[name] != expected:
            raise RuntimeError(f"{profile} requires {name}={expected!r}")
    for name in (
        "CARGO_HOME",
        "CARGO_TARGET_DIR",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME",
        "IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR",
    ):
        _canonical_directory(name, pathlib.Path(environment[name]))
    if (
        environment["CARGO_HOME"]
        != environment["IROHA_PRIVACY_AUTHENTICATED_CARGO_HOME"]
        or environment["CARGO_TARGET_DIR"]
        != environment["IROHA_PRIVACY_AUTHENTICATED_CARGO_TARGET_DIR"]
    ):
        raise RuntimeError("wrapped Apple Cargo directories differ from the corridor")

    repository = _canonical_directory(
        "authenticated privacy SDK root",
        pathlib.Path(environment["IROHA_PRIVACY_SDK_ROOT"]),
    )
    wrapper = repository / "ci" / "privacy_sdk_cargo_wrapper.sh"
    authenticated_wrapper, wrapper_identity = authenticate_regular_executable(
        "authenticated privacy Cargo wrapper", environment["CARGO"]
    )
    if authenticated_wrapper != wrapper:
        raise RuntimeError("wrapped Apple Cargo command does not use the repository wrapper")

    developer = _canonical_directory(
        "authenticated Xcode developer directory",
        pathlib.Path(environment["DEVELOPER_DIR"]),
    )
    sdkroot = _canonical_directory(
        "authenticated Apple SDK root", pathlib.Path(environment["SDKROOT"])
    )
    if (
        environment["IROHA_PRIVACY_AUTHENTICATED_DEVELOPER_DIR"] != str(developer)
        or environment["IROHA_PRIVACY_AUTHENTICATED_SDKROOT"] != str(sdkroot)
    ):
        raise RuntimeError("Apple SDK paths differ from the authenticated profile")
    sdk_parent = (
        developer
        / "Platforms"
        / f"{spec['platform']}.platform"
        / "Developer"
        / "SDKs"
    )
    if sdkroot.parent != sdk_parent:
        raise RuntimeError(f"{profile} SDK root escaped its exact Xcode platform")

    if command[0] != str(authenticated_wrapper):
        raise RuntimeError("wrapped Apple Cargo executable differs from CARGO")
    arguments = command[1:]

    def exact_token(name: str) -> int:
        positions = [index for index, value in enumerate(arguments) if value == name]
        if len(positions) != 1:
            raise RuntimeError(f"{profile} requires exactly one {name}")
        return positions[0]

    def exact_pair(name: str, expected: str) -> int:
        position = exact_token(name)
        if position + 1 >= len(arguments) or arguments[position + 1] != expected:
            raise RuntimeError(f"{profile} requires the exact sequence {name} {expected}")
        return position

    build_position = exact_token("build")
    locked_position = exact_token("--locked")
    offline_position = exact_token("--offline")
    jobs_position = exact_pair("--jobs", "1")
    target_position = exact_pair("--target", target)
    if not (
        build_position
        < locked_position
        < offline_position
        < jobs_position
        < target_position
    ):
        raise RuntimeError(
            f"{profile} must use build --locked --offline --jobs 1 --target {target} in order"
        )
    if any(
        value == "-Z"
        or value.startswith("-Z")
        or value == "--lockfile-path"
        or value.startswith("--lockfile-path=")
        or value.startswith("--target=")
        or value.startswith("--jobs=")
        or value == "-j"
        or value.startswith("-j")
        for value in arguments
    ):
        raise RuntimeError("wrapped Apple Cargo command contains an alternate envelope form")

    verify_apple_targets(
        cargo=pathlib.Path(environment["IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH"]),
        selector=environment["IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR"],
        manifest=pathlib.Path(
            environment["IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_PATH"]
        ),
        manifest_seal=environment[
            "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_SEAL"
        ],
        targets=(target,),
    )
    return {"CARGO": (authenticated_wrapper, wrapper_identity)}


def authenticate_android_cargo_arguments(
    command: list[str],
) -> tuple[pathlib.Path, tuple[int, ...]]:
    workspace = pathlib.Path.cwd()
    canonical_workspace = workspace.resolve(strict=True)
    if workspace != canonical_workspace:
        raise RuntimeError("Android Cargo working directory must be absolute and canonical")
    root_lock, lock_identity = authenticate_regular_file(
        "Android root Cargo.lock",
        canonical_workspace / "Cargo.lock",
    )
    arguments = command[1:]

    def exact_token(name: str) -> int:
        positions = [index for index, value in enumerate(arguments) if value == name]
        if len(positions) != 1:
            raise RuntimeError(f"Android Cargo command requires exactly one {name}")
        return positions[0]

    def exact_pair(name: str, expected: str) -> int:
        position = exact_token(name)
        if position + 1 >= len(arguments) or arguments[position + 1] != expected:
            raise RuntimeError(
                f"Android Cargo command requires the exact sequence {name} {expected}"
            )
        return position

    build_position = exact_token("build")
    locked_position = exact_token("--locked")
    offline_position = exact_token("--offline")
    jobs_position = exact_pair("--jobs", "1")
    unstable_position = exact_pair("-Z", "unstable-options")
    lock_position = exact_pair("--lockfile-path", str(root_lock))
    if not (
        build_position
        < locked_position
        < offline_position
        < jobs_position
        < unstable_position
        < lock_position
    ):
        raise RuntimeError(
            "Android Cargo command must use build --locked --offline --jobs 1 "
            "-Z unstable-options --lockfile-path <root Cargo.lock> in that order"
        )
    if any(
        value == "-j"
        or (value.startswith("-j") and value != "-Z")
        or value.startswith("--jobs=")
        or value.startswith("--lockfile-path=")
        or value.startswith("-Zunstable-options")
        for value in arguments
    ):
        raise RuntimeError("Android Cargo command contains an alternate Cargo envelope form")
    return root_lock, lock_identity


def main() -> int:
    if sys.argv[1:2] == ["seal-apple-targets"]:
        seal_arguments = parse_apple_target_args("seal-apple-targets")
        print(
            seal_apple_targets(
                cargo=seal_arguments.toolchain_cargo,
                selector=seal_arguments.toolchain_selector,
                output=seal_arguments.output,
            )
        )
        return 0
    if sys.argv[1:2] == ["verify-apple-targets"]:
        verify_arguments = parse_apple_target_args("verify-apple-targets")
        verify_apple_targets(
            cargo=verify_arguments.toolchain_cargo,
            selector=verify_arguments.toolchain_selector,
            manifest=verify_arguments.manifest,
            manifest_seal=verify_arguments.manifest_seal,
            targets=(APPLE_TARGETS if verify_arguments.all else (verify_arguments.target,)),
        )
        return 0

    args = parse_args()
    expected = PROFILES[args.profile]
    environment: dict[str, str] = {}
    for name, value in args.assignments:
        if name in environment:
            raise RuntimeError(f"duplicate environment assignment: {name}")
        environment[name] = value
    actual = set(environment)
    if actual != expected:
        missing = sorted(expected - actual)
        unexpected = sorted(actual - expected)
        raise RuntimeError(
            f"{args.profile} environment inventory is not exact "
            f"(missing={missing}, unexpected={unexpected})"
        )

    authenticated_tools: dict[str, tuple[pathlib.Path, tuple[int, ...]]] = {}
    authenticated_files: dict[str, tuple[pathlib.Path, tuple[int, ...]]] = {}
    if args.profile in AUTHENTICATED_CARGO_PROFILES:
        authenticated_tools = authenticate_cargo_environment(environment)
    elif args.profile in PRIVACY_WRAPPED_APPLE_PROFILES:
        authenticated_tools = authenticate_wrapped_apple_environment(
            args.profile, environment, args.command
        )
    if args.profile == "android-cargo":
        authenticated_files["Android root Cargo.lock"] = authenticate_android_cargo_arguments(
            args.command
        )

    executable = pathlib.Path(args.command[0])
    if not executable.is_absolute():
        raise RuntimeError(f"hermetic command executable must be absolute: {executable}")
    resolved = executable.resolve(strict=True)
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise RuntimeError(f"hermetic command executable is not a regular executable: {resolved}")
    if (
        args.profile in AUTHENTICATED_CARGO_PROFILES | PRIVACY_WRAPPED_APPLE_PROFILES
        and resolved != authenticated_tools["CARGO"][0]
    ):
        raise RuntimeError(
            "Cargo command does not match the authenticated CARGO executable"
        )

    completed = subprocess.run(
        [str(resolved), *args.command[1:]],
        env=environment,
        close_fds=True,
        check=False,
    )
    for name, (path, expected_identity) in authenticated_tools.items():
        _, current_identity = authenticate_regular_executable(name, str(path))
        if current_identity != expected_identity:
            raise RuntimeError(f"{name} changed during the hermetic Cargo invocation")
    for name, (path, expected_identity) in authenticated_files.items():
        _, current_identity = authenticate_regular_file(name, path)
        if current_identity != expected_identity:
            raise RuntimeError(f"{name} changed during the hermetic Cargo invocation")
    if args.profile in PRIVACY_WRAPPED_APPLE_PROFILES:
        target = str(PRIVACY_APPLE_PROFILE_SPECS[args.profile]["target"])
        verify_apple_targets(
            cargo=pathlib.Path(environment["IROHA_PRIVACY_AUTHENTICATED_CARGO_PATH"]),
            selector=environment[
                "IROHA_PRIVACY_AUTHENTICATED_RUST_TOOLCHAIN_SELECTOR"
            ],
            manifest=pathlib.Path(
                environment[
                    "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_PATH"
                ]
            ),
            manifest_seal=environment[
                "IROHA_PRIVACY_AUTHENTICATED_APPLE_TARGETS_MANIFEST_SEAL"
            ],
            targets=(target,),
        )
    return completed.returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError) as error:
        print(f"mobile hermetic command failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
