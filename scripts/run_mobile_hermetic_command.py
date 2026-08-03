#!/usr/bin/env python3
"""Run one mobile native-build command with an exact environment.

This launcher is shared by the Apple, Android, and host-JNI build gates.  Its
profiles are deliberately closed inventories: a caller must provide every
declared variable and cannot add undeclared variables.  In particular, ambient
Cargo/Rust compiler flags and wrapper variables never reach the child process.
"""

from __future__ import annotations

import argparse
import os
import pathlib
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
    "gradle-jvm": frozenset(
        {
            "ANDROID_HOME",
            "ANDROID_SDK_ROOT",
            "DYLD_LIBRARY_PATH",
            "GRADLE_USER_HOME",
            "HOME",
            "IROHA_NATIVE_LIBRARY_PATH",
            "IROHA_REQUIRE_KAGEMUSHA_NATIVE",
            "JAVA_HOME",
            "LANG",
            "LC_ALL",
            "LD_LIBRARY_PATH",
            "PATH",
            "TMPDIR",
        }
    ),
}


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
        args.profile in AUTHENTICATED_CARGO_PROFILES
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
    return completed.returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError) as error:
        print(f"mobile hermetic command failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
