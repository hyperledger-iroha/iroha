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
APPLE_CARGO_ENVIRONMENT = COMMON_CARGO_ENVIRONMENT | {"RUSTC_BOOTSTRAP"}
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
    "android-cargo": COMMON_CARGO_ENVIRONMENT
    | {
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
    },
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
            "IROHA_REQUIRE_SORAFS_NATIVE_VALIDATION",
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

    executable = pathlib.Path(args.command[0])
    if not executable.is_absolute():
        raise RuntimeError(f"hermetic command executable must be absolute: {executable}")
    resolved = executable.resolve(strict=True)
    if not resolved.is_file() or not os.access(resolved, os.X_OK):
        raise RuntimeError(f"hermetic command executable is not a regular executable: {resolved}")

    completed = subprocess.run(
        [str(resolved), *args.command[1:]],
        env=environment,
        close_fds=True,
        check=False,
    )
    return completed.returncode


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError) as error:
        print(f"mobile hermetic command failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
