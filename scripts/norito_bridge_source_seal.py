#!/usr/bin/env python3
"""Compute and verify source seals for production mobile SDK artifacts.

The seal follows the transitive local-package dependency closure of
``connect_norito_bridge`` for every packaged target on the selected mobile
platform.  Platform inputs also bind the SDK sources compiled into the shipping
application: Swift on Apple, and Kotlin/Java on Android.  This keeps the native
artifact and its directly paired SDK source on one authenticated snapshot
without pulling in unrelated workspace tools such as Kagami or test-network
helpers.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import shutil
import stat
import subprocess
import sys
from collections.abc import Iterable


APPLE_TARGETS = (
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios",
    "aarch64-apple-darwin",
)
ANDROID_TARGETS = (
    "aarch64-linux-android",
    "x86_64-linux-android",
)
COMMON_ROOT_INPUTS = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "rust-toolchain",
    ".cargo",
    "vendor",
    "scripts/check_mobile_sdk_artifact_pin_commit.py",
    "codec",
    "scripts/check_mobile_sdk_artifacts.sh",
    "scripts/norito_bridge_source_seal.py",
    "scripts/run_mobile_hermetic_command.py",
)
APPLE_ROOT_INPUTS = (
    "IrohaSwift/Package.swift",
    "IrohaSwift/Package.resolved",
    "IrohaSwift/Sources/IrohaSwift",
    "IrohaSwift/Sources/IrohaSwiftMobileTransports",
    "scripts/archive_norito_xcframework.py",
    "scripts/build_norito_xcframework.sh",
    "scripts/exec_with_file_lock.py",
    "scripts/update_norito_bridge_swift_pins.py",
    "scripts/validate_norito_bridge_xcframework.py",
)
APPLE_REQUIRED_ROOT_INPUTS = ("IrohaSwift/Package.resolved",)
# CBSI consumes these Gradle builds directly through composite substitution, so
# their shipping JVM sources must be bound alongside the native `.so` closure.
ANDROID_ROOT_INPUTS = (
    "kotlin/settings.gradle.kts",
    "kotlin/build.gradle.kts",
    "kotlin/gradle.properties",
    "kotlin/gradle/libs.versions.toml",
    "kotlin/gradle/wrapper/gradle-wrapper.jar",
    "kotlin/gradle/wrapper/gradle-wrapper.properties",
    "kotlin/gradlew",
    "kotlin/gradlew.bat",
    "kotlin/core-jvm/build.gradle.kts",
    "kotlin/core-jvm/src/main",
    "kotlin/client-android/build.gradle.kts",
    "kotlin/client-android/src/main",
    "kotlin/offline-wallet-android/build.gradle.kts",
    "java/norito_java/settings.gradle.kts",
    "java/norito_java/build.gradle.kts",
    "java/norito_java/gradle.properties",
    "java/norito_java/src/main",
    "java/iroha_android/settings.gradle.kts",
    "java/iroha_android/build.gradle.kts",
    "java/iroha_android/gradle.properties",
    "java/iroha_android/schemas/norito_schema_manifest.json",
    "java/iroha_android/core/build.gradle.kts",
    "java/iroha_android/core/src/main",
    "java/iroha_android/src/main",
    "java/iroha_android/android/build.gradle.kts",
    "java/iroha_android/android/src/main",
    "scripts/package_mobile_sdk_artifacts.sh",
)
PLATFORM_TARGETS = {
    "apple": APPLE_TARGETS,
    "android": ANDROID_TARGETS,
}
PLATFORM_ROOT_INPUTS = {
    "apple": APPLE_ROOT_INPUTS,
    "android": ANDROID_ROOT_INPUTS,
}
# Kept as a public union for callers/tests which construct their own input set.
ROOT_INPUTS = tuple(
    dict.fromkeys(COMMON_ROOT_INPUTS + APPLE_ROOT_INPUTS + ANDROID_ROOT_INPUTS)
)
SNAPSHOT_SCHEMA = "iroha.norito-bridge-source-seal.v1"
SWIFT_NATIVE_BRIDGE_PATH = "IrohaSwift/Sources/IrohaSwift/NativeBridge.swift"
SWIFT_NATIVE_BRIDGE_HASH_KEYS = frozenset(
    {
        "macos-arm64",
        "ios-arm64",
        "ios-arm64_x86_64-simulator",
    }
)
SWIFT_NATIVE_BRIDGE_HASH_PIN = re.compile(
    rb'^(?P<prefix>[ \t]+)"(?P<key>macos-arm64|ios-arm64|ios-arm64_x86_64-simulator)"'
    rb': "(?P<digest>[0-9a-f]{64})"(?P<suffix>,?)$',
    re.MULTILINE,
)
SWIFT_NATIVE_BRIDGE_HASH_BLOCK = re.compile(
    rb'^    private static let expectedHashes: \[String: String\] = \[\n'
    rb'(?P<body>(?:[ \t]+"(?:macos-arm64|ios-arm64|ios-arm64_x86_64-simulator)"'
    rb': "[0-9a-f]{64}",\n){2}'
    rb'[ \t]+"(?:macos-arm64|ios-arm64|ios-arm64_x86_64-simulator)"'
    rb': "[0-9a-f]{64}"\n)'
    rb'^    \]$',
    re.MULTILINE,
)


class AuthenticatedTool:
    """One tool's proxy invocation and authenticated canonical executable."""

    __slots__ = ("invocation", "canonical", "canonical_identity")

    def __init__(
        self,
        *,
        invocation: pathlib.Path,
        canonical: pathlib.Path,
        canonical_identity: tuple[int, int, int, int, int],
    ) -> None:
        self.invocation = invocation
        self.canonical = canonical
        self.canonical_identity = canonical_identity

    def authenticate(self) -> None:
        try:
            canonical = self.invocation.resolve(strict=True)
            stat_result = canonical.stat()
        except OSError as error:
            raise RuntimeError(
                f"source-seal tool became unavailable: {self.invocation}"
            ) from error
        identity = (
            stat_result.st_dev,
            stat_result.st_ino,
            stat_result.st_mode,
            stat_result.st_size,
            stat_result.st_mtime_ns,
        )
        if canonical != self.canonical or identity != self.canonical_identity:
            raise RuntimeError(
                f"source-seal tool changed after authentication: {self.invocation}"
            )


def required_tool(environment_name: str, fallback_name: str) -> AuthenticatedTool:
    configured = os.environ.get(environment_name)
    candidate = pathlib.Path(configured) if configured else None
    if candidate is None:
        discovered = shutil.which(fallback_name)
        if discovered is None:
            raise RuntimeError(f"required source-seal tool is unavailable: {fallback_name}")
        candidate = pathlib.Path(discovered)
    if not candidate.is_absolute():
        raise RuntimeError(f"{environment_name} must name an absolute executable")
    invocation = pathlib.Path(os.path.abspath(candidate))
    canonical = invocation.resolve(strict=True)
    if not canonical.is_file() or not os.access(canonical, os.X_OK):
        raise RuntimeError(f"source-seal tool is not a regular executable: {canonical}")
    stat_result = canonical.stat()
    return AuthenticatedTool(
        invocation=invocation,
        canonical=canonical,
        canonical_identity=(
            stat_result.st_dev,
            stat_result.st_ino,
            stat_result.st_mode,
            stat_result.st_size,
            stat_result.st_mtime_ns,
        ),
    )


def source_seal_home() -> pathlib.Path:
    configured = os.environ.get("NORITO_BRIDGE_SEAL_HOME")
    if configured:
        candidate = pathlib.Path(configured)
        if not candidate.is_absolute():
            raise RuntimeError("NORITO_BRIDGE_SEAL_HOME must be absolute")
        return candidate.resolve(strict=True)
    if os.name == "posix":
        import pwd

        return pathlib.Path(pwd.getpwuid(os.getuid()).pw_dir).resolve(strict=True)
    return pathlib.Path.home().resolve(strict=True)


def selected_lockfile_path(
    root: pathlib.Path, configured: pathlib.Path | None = None
) -> pathlib.Path:
    """Return the canonical, non-symbolic root Cargo lock used by the build."""

    candidate = root / "Cargo.lock"
    if configured is not None and configured != candidate:
        raise RuntimeError(
            f"source sealing requires the explicit root Cargo lock: {candidate}"
        )
    if not candidate.is_absolute():
        raise RuntimeError("selected Cargo lock path must be absolute")
    canonical_spelling = pathlib.Path(os.path.abspath(candidate))
    if candidate != canonical_spelling:
        raise RuntimeError("selected Cargo lock path must be canonical")
    try:
        metadata = candidate.lstat()
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        raise RuntimeError(
            "selected Cargo lock must be a non-symbolic regular file"
        ) from error
    if (
        resolved != candidate
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
    ):
        raise RuntimeError("selected Cargo lock must be a non-symbolic regular file")
    return candidate


def source_seal_environment(
    *,
    cargo: AuthenticatedTool,
    rustc: AuthenticatedTool,
    rustdoc: AuthenticatedTool,
    git: pathlib.Path,
) -> dict[str, str]:
    home = source_seal_home()
    cargo_home = pathlib.Path(
        os.environ.get("NORITO_BRIDGE_SEAL_CARGO_HOME", str(home / ".cargo"))
    )
    rustup_home = pathlib.Path(
        os.environ.get("NORITO_BRIDGE_SEAL_RUSTUP_HOME", str(home / ".rustup"))
    )
    temporary_directory = pathlib.Path(
        os.environ.get("NORITO_BRIDGE_SEAL_TMPDIR", "/tmp")
    )
    for label, path in (
        ("NORITO_BRIDGE_SEAL_CARGO_HOME", cargo_home),
        ("NORITO_BRIDGE_SEAL_RUSTUP_HOME", rustup_home),
        ("NORITO_BRIDGE_SEAL_TMPDIR", temporary_directory),
    ):
        if not path.is_absolute():
            raise RuntimeError(f"{label} must be absolute")
    path_entries = tuple(
        dict.fromkeys(
            (
                str(cargo.invocation.parent),
                str(rustc.invocation.parent),
                str(rustdoc.invocation.parent),
                str(git.parent),
                "/usr/bin",
                "/bin",
            )
        )
    )
    environment = {
        "CARGO": str(cargo.invocation),
        "CARGO_BUILD_JOBS": "1",
        "CARGO_HOME": str(cargo_home),
        "CARGO_INCREMENTAL": "0",
        "CARGO_NET_OFFLINE": "true",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_OPTIONAL_LOCKS": "0",
        "HOME": str(home),
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
        "PATH": os.pathsep.join(path_entries),
        "RUSTC": str(rustc.invocation),
        "RUSTC_BOOTSTRAP": "1",
        "RUSTDOC": str(rustdoc.invocation),
        "RUSTUP_HOME": str(rustup_home),
        "TMPDIR": str(temporary_directory),
    }
    configured_target = os.environ.get("NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR")
    if not configured_target:
        raise RuntimeError("NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR is required")
    target = pathlib.Path(configured_target)
    if not target.is_absolute() or target != pathlib.Path(os.path.abspath(target)):
        raise RuntimeError(
            "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR must be an absolute canonical directory"
        )
    try:
        metadata = target.lstat()
        resolved = target.resolve(strict=True)
    except OSError as error:
        raise RuntimeError(
            "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR is unavailable"
        ) from error
    if (
        resolved != target
        or stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
    ):
        raise RuntimeError(
            "NORITO_BRIDGE_SEAL_CARGO_TARGET_DIR must be a non-symbolic "
            "canonical directory"
        )
    environment["CARGO_TARGET_DIR"] = str(target)
    return environment


def source_seal_tools() -> tuple[
    AuthenticatedTool, AuthenticatedTool, AuthenticatedTool, pathlib.Path
]:
    git = pathlib.Path("/usr/bin/git").resolve(strict=True)
    if not git.is_file() or not os.access(git, os.X_OK):
        raise RuntimeError("pinned source-seal Git executable is unavailable")
    return (
        required_tool("NORITO_BRIDGE_SEAL_CARGO", "cargo"),
        required_tool("NORITO_BRIDGE_SEAL_RUSTC", "rustc"),
        required_tool("NORITO_BRIDGE_SEAL_RUSTDOC", "rustdoc"),
        git,
    )


def run(
    root: pathlib.Path,
    executable: AuthenticatedTool | pathlib.Path,
    args: list[str],
    environment: dict[str, str],
) -> bytes:
    if isinstance(executable, AuthenticatedTool):
        executable.authenticate()
        invocation = executable.invocation
        canonical = executable.canonical
    else:
        invocation = executable
        canonical = executable
    try:
        result = subprocess.run(
            [str(invocation), *args],
            executable=str(canonical),
            cwd=root,
            env=environment,
            check=True,
            stdout=subprocess.PIPE,
        ).stdout
    finally:
        if isinstance(executable, AuthenticatedTool):
            executable.authenticate()
    return result


def metadata(
    root: pathlib.Path,
    target: str,
    lockfile_path: pathlib.Path | None = None,
) -> dict[str, object]:
    lockfile = selected_lockfile_path(root, lockfile_path)
    cargo, rustc, rustdoc, git = source_seal_tools()
    rustc.authenticate()
    rustdoc.authenticate()
    try:
        output = run(
            root,
            cargo,
            [
                "metadata",
                "--locked",
                "--offline",
                "-Z",
                "unstable-options",
                "--lockfile-path",
                str(lockfile),
                "--format-version",
                "1",
                "--features",
                "connect_norito_bridge/privacy-production-enabled",
                "--filter-platform",
                target,
            ],
            source_seal_environment(
                cargo=cargo,
                rustc=rustc,
                rustdoc=rustdoc,
                git=git,
            ),
        )
    finally:
        rustc.authenticate()
        rustdoc.authenticate()
    return json.loads(output)


def local_dependency_roots(
    root: pathlib.Path,
    targets: Iterable[str] = APPLE_TARGETS,
    lockfile_path: pathlib.Path | None = None,
) -> set[str]:
    package_roots: set[pathlib.Path] = set()
    for target in targets:
        document = metadata(root, target, lockfile_path)
        packages = {
            package["id"]: package
            for package in document["packages"]
            if isinstance(package, dict)
        }
        resolve = document.get("resolve")
        if not isinstance(resolve, dict):
            raise RuntimeError("cargo metadata did not return a resolve graph")
        nodes = {
            node["id"]: node
            for node in resolve.get("nodes", [])
            if isinstance(node, dict)
        }
        roots = [
            package_id
            for package_id, package in packages.items()
            if package.get("name") == "connect_norito_bridge"
            and pathlib.Path(str(package["manifest_path"])).resolve()
            == (root / "crates/connect_norito_bridge/Cargo.toml").resolve()
        ]
        if len(roots) != 1:
            raise RuntimeError(
                f"expected one connect_norito_bridge package for {target}, found {len(roots)}"
            )

        pending = roots
        visited: set[str] = set()
        while pending:
            package_id = pending.pop()
            if package_id in visited:
                continue
            visited.add(package_id)
            node = nodes.get(package_id)
            if node is None:
                raise RuntimeError(f"missing resolve node for {package_id}")
            for dependency in node.get("deps", []):
                if isinstance(dependency, dict) and isinstance(dependency.get("pkg"), str):
                    pending.append(dependency["pkg"])

        for package_id in visited:
            package = packages.get(package_id)
            if package is None:
                continue
            manifest = pathlib.Path(str(package["manifest_path"])).resolve()
            try:
                relative = manifest.parent.relative_to(root)
            except ValueError:
                continue
            package_roots.add(relative)

    return {path.as_posix() for path in package_roots}


def seal_inputs(
    root: pathlib.Path,
    platform: str = "apple",
    lockfile_path: pathlib.Path | None = None,
) -> list[str]:
    lockfile = selected_lockfile_path(root, lockfile_path)
    try:
        targets = PLATFORM_TARGETS[platform]
        platform_inputs = PLATFORM_ROOT_INPUTS[platform]
    except KeyError as error:
        raise RuntimeError(f"unsupported source-seal platform: {platform}") from error
    if platform == "apple":
        for value in APPLE_REQUIRED_ROOT_INPUTS:
            required = root / value
            try:
                metadata = required.lstat()
            except OSError as error:
                raise RuntimeError(f"required Apple source-seal input is missing: {value}") from error
            if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
                raise RuntimeError(
                    f"required Apple source-seal input is not a regular file: {value}"
                )
    candidates = set(COMMON_ROOT_INPUTS)
    candidates.update(platform_inputs)
    candidates.update(local_dependency_roots(root, targets, lockfile))
    existing = [
        value
        for value in candidates
        if (value == "Cargo.lock" and lockfile.is_file()) or (root / value).exists()
    ]
    return sorted(existing)


def listed_files(
    root: pathlib.Path,
    inputs: Iterable[str],
    lockfile_path: pathlib.Path | None = None,
) -> list[str]:
    lockfile = selected_lockfile_path(root, lockfile_path)
    input_set = set(inputs)
    cargo, rustc, rustdoc, git = source_seal_tools()
    output = run(
        root,
        git,
        [
            "ls-files",
            "-z",
            "-co",
            "--exclude-standard",
            "--",
            *inputs,
        ],
        source_seal_environment(cargo=cargo, rustc=rustc, rustdoc=rustdoc, git=git),
    )
    listed = {
        value.decode("utf-8")
        for value in output.split(b"\0")
        if value
    }

    # Some workspace-wide build inputs are intentionally ignored by repository
    # policy (notably the selected Cargo.lock). They still affect the bridge binary,
    # so an explicit ROOT_INPUT must be sealed even when `git ls-files -co
    # --exclude-standard` omits it. Do not recursively include arbitrary ignored
    # files below directory inputs: build outputs and local corpora remain outside
    # the production source seal unless named explicitly above.
    for relative in ROOT_INPUTS:
        if relative not in input_set:
            continue
        source = lockfile if relative == "Cargo.lock" else root / relative
        if source.is_symlink():
            raise RuntimeError(f"explicit source-seal input is symlinked: {relative}")
        if source.is_file():
            listed.add(relative)

    return sorted(listed)


def _swift_native_bridge_hash_block(
    contents: bytes,
) -> tuple[re.Match[bytes], list[re.Match[bytes]]]:
    blocks = list(SWIFT_NATIVE_BRIDGE_HASH_BLOCK.finditer(contents))
    if len(blocks) != 1:
        raise RuntimeError(
            "NativeBridge.swift must contain exactly one canonical expectedHashes block"
        )
    block = blocks[0]
    matches = list(SWIFT_NATIVE_BRIDGE_HASH_PIN.finditer(block.group("body")))
    keys = [match.group("key").decode("ascii") for match in matches]
    suffixes = [match.group("suffix") for match in matches]
    if (
        len(matches) != len(SWIFT_NATIVE_BRIDGE_HASH_KEYS)
        or set(keys) != set(SWIFT_NATIVE_BRIDGE_HASH_KEYS)
        or suffixes != [b",", b",", b""]
    ):
        raise RuntimeError(
            "NativeBridge.swift must contain exactly one canonical fallback hash "
            "for every Apple artifact slice"
        )
    return block, matches


def swift_native_bridge_hash_pins(contents: bytes) -> dict[str, str]:
    """Return pins from the sole executable ``expectedHashes`` declaration."""

    _block, matches = _swift_native_bridge_hash_block(contents)
    return {
        match.group("key").decode("ascii"): match.group("digest").decode("ascii")
        for match in matches
    }


def rewrite_swift_native_bridge_hash_pins(
    contents: bytes,
    hashes: dict[str, str],
) -> bytes:
    """Rewrite only the canonical ``expectedHashes`` declaration."""

    if set(hashes) != set(SWIFT_NATIVE_BRIDGE_HASH_KEYS) or any(
        not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None
        for value in hashes.values()
    ):
        raise RuntimeError("Swift native bridge replacement hashes are not canonical")
    block, _matches = _swift_native_bridge_hash_block(contents)
    body = block.group("body")

    def replace(match: re.Match[bytes]) -> bytes:
        key = match.group("key").decode("ascii")
        return (
            match.group("prefix")
            + b'"'
            + match.group("key")
            + b'": "'
            + hashes[key].encode("ascii")
            + b'"'
            + match.group("suffix")
        )

    rewritten = SWIFT_NATIVE_BRIDGE_HASH_PIN.sub(replace, body)
    body_start, body_end = block.span("body")
    return contents[:body_start] + rewritten + contents[body_end:]


def normalize_swift_native_bridge_hash_pins(contents: bytes) -> bytes:
    """Normalize the three manifestless fallback digests for source sealing.

    The artifact checker authenticates these values independently against every
    slice in the embedded manifest. Normalizing only the digest literals keeps
    a mechanical pin-only child commit on the exact source fingerprint of the
    artifact-producing parent without excluding any executable loader logic.
    """

    return rewrite_swift_native_bridge_hash_pins(
        contents,
        {key: "0" * 64 for key in SWIFT_NATIVE_BRIDGE_HASH_KEYS},
    )


def fingerprint(
    root: pathlib.Path,
    inputs: list[str],
    lockfile_path: pathlib.Path | None = None,
) -> str:
    lockfile = selected_lockfile_path(root, lockfile_path)
    digest = hashlib.sha256()
    for relative in listed_files(root, inputs, lockfile):
        source = lockfile if relative == "Cargo.lock" else root / relative
        if source.is_symlink():
            raise RuntimeError(f"source-seal input is symlinked: {relative}")
        if not source.is_file():
            raise RuntimeError(f"source-seal input is not a regular file: {relative}")
        contents = source.read_bytes()
        if relative == SWIFT_NATIVE_BRIDGE_PATH:
            contents = normalize_swift_native_bridge_hash_pins(contents)
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update(contents)
        digest.update(b"\0")
    return digest.hexdigest()


def status(
    root: pathlib.Path,
    inputs: list[str],
    lockfile_path: pathlib.Path | None = None,
) -> str:
    lockfile = selected_lockfile_path(root, lockfile_path)
    status_inputs = [
        relative
        for relative in inputs
        if relative != "Cargo.lock" or lockfile == root / "Cargo.lock"
    ]
    cargo, rustc, rustdoc, git = source_seal_tools()
    output = run(
        root,
        git,
        [
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--",
            *status_inputs,
        ],
        source_seal_environment(cargo=cargo, rustc=rustc, rustdoc=rustdoc, git=git),
    )
    return output.decode("utf-8").rstrip("\n")


def source_commit(root: pathlib.Path) -> str:
    cargo, rustc, rustdoc, git = source_seal_tools()
    value = run(
        root,
        git,
        ["rev-parse", "--verify", "HEAD"],
        source_seal_environment(cargo=cargo, rustc=rustc, rustdoc=rustdoc, git=git),
    ).decode("ascii").strip()
    if len(value) != 40 or any(character not in "0123456789abcdef" for character in value):
        raise RuntimeError("source commit is not a canonical lowercase Git SHA-1")
    return value


def snapshot(
    root: pathlib.Path,
    platform: str,
    lockfile_path: pathlib.Path | None = None,
) -> dict[str, object]:
    """Return the canonical source state consumed by one platform build."""

    lockfile = selected_lockfile_path(root, lockfile_path)
    inputs = seal_inputs(root, platform, lockfile)
    source_commit_before = source_commit(root)
    source_status_before = status(root, inputs, lockfile)
    source_fingerprint_before = fingerprint(root, inputs, lockfile)
    source_fingerprint_after = fingerprint(root, inputs, lockfile)
    source_status_after = status(root, inputs, lockfile)
    source_commit_after = source_commit(root)
    if source_commit_before != source_commit_after:
        raise RuntimeError(
            f"{platform} NoritoBridge source commit changed while authenticating "
            "the selected-source fingerprint"
        )
    if (
        source_status_before != source_status_after
        or source_fingerprint_before != source_fingerprint_after
    ):
        raise RuntimeError(
            f"{platform} NoritoBridge selected source changed while authenticating "
            "the build snapshot"
        )
    return {
        "schema": SNAPSHOT_SCHEMA,
        "platform": platform,
        "targets": list(PLATFORM_TARGETS[platform]),
        "source_commit": source_commit_before,
        "source_tree_dirty": bool(source_status_before),
        "source_status": source_status_before,
        "source_fingerprint_sha256": source_fingerprint_before,
    }


def snapshot_bytes(
    root: pathlib.Path,
    platform: str,
    lockfile_path: pathlib.Path | None = None,
) -> bytes:
    return (
        json.dumps(
            snapshot(root, platform, lockfile_path),
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def verify_snapshot(
    root: pathlib.Path,
    platform: str,
    snapshot_path: pathlib.Path,
    lockfile_path: pathlib.Path | None = None,
) -> None:
    """Reject a missing, tampered, stale, or mixed-source build snapshot."""

    expected = snapshot_path.read_bytes()
    current = snapshot_bytes(root, platform, lockfile_path)
    if expected != current:
        raise RuntimeError(
            f"{platform} NoritoBridge source changed after the build started"
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "mode", choices=("fingerprint", "paths", "snapshot", "status", "verify")
    )
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument(
        "--platform", choices=tuple(PLATFORM_TARGETS), default="apple"
    )
    parser.add_argument(
        "--snapshot",
        type=pathlib.Path,
        help="Build-start snapshot to authenticate in verify mode.",
    )
    parser.add_argument(
        "--lockfile-path",
        type=pathlib.Path,
        help="Absolute canonical Cargo lock consumed by metadata and fingerprinting.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    lockfile = selected_lockfile_path(root, args.lockfile_path)
    inputs = seal_inputs(root, args.platform, lockfile)
    if args.mode == "fingerprint":
        print(fingerprint(root, inputs, lockfile))
    elif args.mode == "paths":
        print("\n".join(inputs))
    elif args.mode == "status":
        value = status(root, inputs, lockfile)
        if value:
            print(value)
    elif args.mode == "snapshot":
        sys.stdout.buffer.write(snapshot_bytes(root, args.platform, lockfile))
    else:
        if args.snapshot is None:
            raise RuntimeError("verify mode requires --snapshot")
        verify_snapshot(root, args.platform, args.snapshot.resolve(), lockfile)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError) as exc:
        print(f"norito bridge source seal failed: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
