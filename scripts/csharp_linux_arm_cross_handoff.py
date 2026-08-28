#!/usr/bin/env python3
"""Seal and verify the authenticated C# Linux ARM cross-build handoff.

The Linux ARM release bridge is built on a GitHub-hosted x64 runner, then
loaded and ABI-checked on a short-lived native ARM runner.  This helper binds
the immutable handoff to the exact clean checkout, cross toolchain identity,
build command, ELF architecture, and shared-library bytes.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import stat
import subprocess
import sys
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import NoReturn

if __package__:
    from . import check_native_sdk_abi23_artifact as native_checker
else:
    _checker_path = Path(__file__).resolve(strict=True).with_name(
        "check_native_sdk_abi23_artifact.py"
    )
    _checker_spec = importlib.util.spec_from_file_location(
        "_iroha_native_sdk_artifact_checker",
        _checker_path,
    )
    if _checker_spec is None or _checker_spec.loader is None:
        raise ImportError(f"unable to load native artifact checker: {_checker_path}")
    native_checker = importlib.util.module_from_spec(_checker_spec)
    _checker_spec.loader.exec_module(native_checker)


SCHEMA = "iroha.csharp-linux-arm-cross-handoff.v1"
TARGET = "aarch64-unknown-linux-gnu"
BUILD_HOST = "x86_64-unknown-linux-gnu"
LINKER_MACHINE = "aarch64-linux-gnu"
ARTIFACT_NAME = "libconnect_norito_bridge.so"
MANIFEST_NAME = "csharp-linux-arm-cross-handoff.json"
MAX_MANIFEST_BYTES = 64 * 1024
MAX_ARTIFACT_BYTES = 4 * 1024 * 1024 * 1024
BUILD_COMMAND = (
    "cargo",
    "rustc",
    "--locked",
    "--release",
    "-p",
    "connect_norito_bridge",
    "--target",
    TARGET,
    "--lib",
    "--crate-type",
    "cdylib",
)
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
VERSION_TEXT_RE = re.compile(r"[\x20-\x7e]{1,512}")
BUILD_ENVIRONMENT_KEYS = {
    "cargo_version",
    "linker_dumpmachine",
    "linker_version",
    "rustc_commit_hash",
    "rustc_host",
    "rustc_release",
}
MANIFEST_KEYS = {
    "artifact_name",
    "artifact_sha256",
    "artifact_size",
    "build_command",
    "build_environment",
    "schema",
    "source_commit",
    "source_tree_clean",
    "target",
    "workspace_source_manifest_sha256",
}


class HandoffError(RuntimeError):
    """The C# Linux ARM cross-build handoff failed authentication."""


def fail(message: str) -> NoReturn:
    """Raise one stable handoff error."""

    raise HandoffError(message)


def _plain_object(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        fail(f"{label} must be a JSON object")
    return value


def _reject_duplicate_pairs(
    pairs: list[tuple[str, object]],
) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail(f"cross-build handoff contains duplicate key {key!r}")
        result[key] = value
    return result


def _require_sha256(value: object, label: str) -> str:
    if (
        not isinstance(value, str)
        or SHA256_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        fail(f"{label} must be a non-zero lowercase SHA-256 digest")
    return value


def _require_printable(value: object, label: str) -> str:
    if not isinstance(value, str) or VERSION_TEXT_RE.fullmatch(value) is None:
        fail(f"{label} must be one bounded printable line")
    return value


def validate_manifest(value: object) -> dict[str, object]:
    """Validate the exact handoff manifest schema."""

    manifest = _plain_object(value, "cross-build handoff manifest")
    if set(manifest) != MANIFEST_KEYS:
        fail("cross-build handoff manifest has a non-canonical field inventory")
    if manifest["schema"] != SCHEMA:
        fail("cross-build handoff manifest has the wrong schema")
    if manifest["target"] != TARGET:
        fail("cross-build handoff manifest has the wrong Rust target")
    if manifest["artifact_name"] != ARTIFACT_NAME:
        fail("cross-build handoff manifest has the wrong artifact name")
    _require_sha256(manifest["artifact_sha256"], "artifact SHA-256")
    size = manifest["artifact_size"]
    if type(size) is not int or size <= 0 or size > MAX_ARTIFACT_BYTES:
        fail("cross-build handoff artifact size is outside the accepted bound")
    commit = manifest["source_commit"]
    if not isinstance(commit, str) or COMMIT_RE.fullmatch(commit) is None:
        fail("cross-build handoff source commit is not canonical Git SHA-1")
    if manifest["source_tree_clean"] is not True:
        fail("cross-build handoff requires a clean source tree")
    _require_sha256(
        manifest["workspace_source_manifest_sha256"],
        "workspace source manifest SHA-256",
    )
    if manifest["build_command"] != list(BUILD_COMMAND):
        fail("cross-build handoff has the wrong cdylib-only Cargo command")

    environment = _plain_object(
        manifest["build_environment"], "cross-build environment"
    )
    if set(environment) != BUILD_ENVIRONMENT_KEYS:
        fail("cross-build environment has a non-canonical field inventory")
    _require_printable(environment["cargo_version"], "Cargo version")
    _require_printable(environment["linker_version"], "cross linker version")
    if environment["linker_dumpmachine"] != LINKER_MACHINE:
        fail("cross-build environment has the wrong linker machine")
    if environment["rustc_host"] != BUILD_HOST:
        fail("cross-build environment has the wrong rustc host")
    _require_printable(environment["rustc_release"], "rustc release")
    rustc_commit = environment["rustc_commit_hash"]
    if not isinstance(rustc_commit, str) or COMMIT_RE.fullmatch(rustc_commit) is None:
        fail("cross-build environment has a non-canonical rustc commit hash")
    return manifest


def canonical_manifest_bytes(manifest: Mapping[str, object]) -> bytes:
    """Encode a validated manifest deterministically."""

    validated = validate_manifest(dict(manifest))
    return (
        json.dumps(validated, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def _run_tool(executable: str, arguments: Sequence[str], label: str) -> str:
    try:
        result = subprocess.run(
            [executable, *arguments],
            check=False,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise HandoffError(f"{label} could not be inspected") from error
    output = result.stdout.strip()
    if result.returncode != 0 or not output:
        detail = result.stderr.strip()
        fail(f"{label} inspection failed" + (f": {detail}" if detail else ""))
    if len(output.encode("utf-8")) > 16 * 1024:
        fail(f"{label} inspection output is too large")
    return output


def capture_build_environment(
    *, cargo: str, rustc: str, linker: str
) -> dict[str, object]:
    """Capture the bounded cross-toolchain identity used by the producer."""

    cargo_version = _run_tool(cargo, ("-V",), "Cargo").splitlines()
    rustc_verbose = _run_tool(rustc, ("-vV",), "rustc")
    linker_machine = _run_tool(linker, ("-dumpmachine",), "cross linker")
    linker_version = _run_tool(linker, ("--version",), "cross linker").splitlines()
    if len(cargo_version) != 1 or not linker_version:
        fail("cross-build tool versions are not canonical")
    rustc_fields: dict[str, str] = {}
    for line in rustc_verbose.splitlines():
        key, separator, value = line.partition(": ")
        if separator and key in {"commit-hash", "host", "release"}:
            rustc_fields[key] = value
    if set(rustc_fields) != {"commit-hash", "host", "release"}:
        fail("rustc verbose version is missing required identity fields")
    environment: dict[str, object] = {
        "cargo_version": cargo_version[0],
        "linker_dumpmachine": linker_machine,
        "linker_version": linker_version[0],
        "rustc_commit_hash": rustc_fields["commit-hash"],
        "rustc_host": rustc_fields["host"],
        "rustc_release": rustc_fields["release"],
    }
    validate_manifest(
        {
            "artifact_name": ARTIFACT_NAME,
            "artifact_sha256": "1" * 64,
            "artifact_size": 1,
            "build_command": list(BUILD_COMMAND),
            "build_environment": environment,
            "schema": SCHEMA,
            "source_commit": "1" * 40,
            "source_tree_clean": True,
            "target": TARGET,
            "workspace_source_manifest_sha256": "1" * 64,
        }
    )
    return environment


def authenticate_source(source_root: Path) -> tuple[str, str]:
    """Return the exact clean Git commit and canonical source digest."""

    commit, clean = native_checker.source_state(source_root)
    if not clean:
        fail("cross-build handoff requires a clean source checkout")
    digest = native_checker.workspace_source_manifest_sha256(source_root)
    commit_after, clean_after = native_checker.source_state(source_root)
    if commit_after != commit or not clean_after:
        fail("source checkout changed while it was authenticated")
    return commit, digest


def _file_revision(metadata: os.stat_result) -> tuple[int, ...]:
    return (
        metadata.st_dev,
        metadata.st_ino,
        metadata.st_mode,
        metadata.st_nlink,
        metadata.st_uid,
        metadata.st_size,
        metadata.st_mtime_ns,
        metadata.st_ctime_ns,
    )


def validate_linux_arm_cdylib(path: Path) -> None:
    """Require a stable little-endian AArch64 ELF shared object."""

    try:
        before = path.lstat()
    except OSError as error:
        raise HandoffError(f"cross-build artifact is unavailable: {path}") from error
    if (
        stat.S_ISLNK(before.st_mode)
        or not stat.S_ISREG(before.st_mode)
        or before.st_nlink < 1
        or before.st_size < 20
    ):
        fail("cross-build artifact must be one non-empty regular ELF file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise HandoffError("cross-build artifact could not be opened") from error
    try:
        opened = os.fstat(descriptor)
        header = os.read(descriptor, 20)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    try:
        named_after = path.lstat()
    except OSError as error:
        raise HandoffError(
            "cross-build artifact changed during ELF inspection"
        ) from error
    if (
        _file_revision(before) != _file_revision(opened)
        or _file_revision(opened) != _file_revision(after)
        or _file_revision(after) != _file_revision(named_after)
    ):
        fail("cross-build artifact changed during ELF inspection")
    if (
        header[:7] != b"\x7fELF\x02\x01\x01"
        or header[16:18] != b"\x03\x00"
        or header[18:20] != b"\xb7\x00"
    ):
        fail("cross-build artifact is not a little-endian AArch64 ELF shared object")


def _require_candidate_directory(root: Path) -> tuple[int, int, int, int]:
    try:
        metadata = root.lstat()
    except OSError as error:
        raise HandoffError(
            f"cross-build candidate directory is unavailable: {root}"
        ) from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        fail("cross-build candidate must be an owner-controlled directory")
    return metadata.st_dev, metadata.st_ino, metadata.st_mode, metadata.st_uid


def _require_candidate_file(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise HandoffError(f"{label} is unavailable: {path}") from error
    if (
        stat.S_ISLNK(metadata.st_mode)
        or not stat.S_ISREG(metadata.st_mode)
        or metadata.st_nlink != 1
        or metadata.st_uid != os.geteuid()
        or stat.S_IMODE(metadata.st_mode) & 0o022
    ):
        fail(f"{label} must be an owner-controlled regular file with one link")


def _write_manifest_exclusively(root: Path, payload: bytes) -> Path:
    manifest_path = root / MANIFEST_NAME
    directory_flags = (
        os.O_RDONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_DIRECTORY", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    write_flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        directory = os.open(root, directory_flags)
    except OSError as error:
        raise HandoffError(
            "cross-build candidate directory could not be pinned"
        ) from error
    descriptor: int | None = None
    try:
        pinned = os.fstat(directory)
        descriptor = os.open(MANIFEST_NAME, write_flags, 0o600, dir_fd=directory)
        offset = 0
        while offset < len(payload):
            written = os.write(descriptor, payload[offset:])
            if written <= 0:
                fail("cross-build handoff manifest could not be written completely")
            offset += written
        os.fsync(descriptor)
        created = os.fstat(descriptor)
        named = manifest_path.lstat()
        root_after = root.lstat()
        if (
            not stat.S_ISREG(created.st_mode)
            or created.st_nlink != 1
            or created.st_size != len(payload)
            or stat.S_IMODE(created.st_mode) & 0o077
            or (created.st_dev, created.st_ino) != (named.st_dev, named.st_ino)
            or (pinned.st_dev, pinned.st_ino) != (root_after.st_dev, root_after.st_ino)
        ):
            fail("cross-build handoff manifest changed while it was created")
    except OSError as error:
        raise HandoffError(
            "cross-build handoff manifest could not be created"
        ) from error
    finally:
        if descriptor is not None:
            os.close(descriptor)
        os.close(directory)
    return manifest_path


SourceAuthenticator = Callable[[Path], tuple[str, str]]


def seal_candidate(
    *,
    artifact: Path,
    candidate_root: Path,
    source_root: Path,
    build_environment: Mapping[str, object],
    source_authenticator: SourceAuthenticator = authenticate_source,
) -> dict[str, object]:
    """Create one fresh deterministic cross-build candidate directory."""

    artifact = Path(os.path.abspath(artifact))
    candidate_root = Path(os.path.abspath(candidate_root))
    source_root = source_root.resolve(strict=True)
    if candidate_root == source_root or source_root in candidate_root.parents:
        fail("cross-build candidate must be outside the source checkout")
    if candidate_root.exists() or candidate_root.is_symlink():
        fail("cross-build candidate directory must be one fresh path")
    parent = candidate_root.parent.resolve(strict=True)
    if parent != candidate_root.parent:
        fail("cross-build candidate parent must not contain symlinks")

    source_before = source_authenticator(source_root)
    validate_linux_arm_cdylib(artifact)
    try:
        candidate_root.mkdir(mode=0o700)
    except OSError as error:
        raise HandoffError(
            "cross-build candidate directory could not be created"
        ) from error
    _require_candidate_directory(candidate_root)
    staged_path, artifact_digest, artifact_size = native_checker.stage_unique_artifact(
        artifact, candidate_root / ARTIFACT_NAME
    )
    validate_linux_arm_cdylib(staged_path)
    manifest: dict[str, object] = {
        "artifact_name": ARTIFACT_NAME,
        "artifact_sha256": artifact_digest,
        "artifact_size": artifact_size,
        "build_command": list(BUILD_COMMAND),
        "build_environment": dict(build_environment),
        "schema": SCHEMA,
        "source_commit": source_before[0],
        "source_tree_clean": True,
        "target": TARGET,
        "workspace_source_manifest_sha256": source_before[1],
    }
    _write_manifest_exclusively(candidate_root, canonical_manifest_bytes(manifest))
    if source_authenticator(source_root) != source_before:
        fail("source checkout changed while the cross-build handoff was sealed")
    verify_candidate(
        candidate_root=candidate_root,
        source_root=source_root,
        source_authenticator=source_authenticator,
    )
    return manifest


def _load_manifest(path: Path) -> dict[str, object]:
    _require_candidate_file(path, "cross-build handoff manifest")
    try:
        payload = native_checker.stable_bounded_file_bytes(
            path,
            label="cross-build handoff manifest",
            maximum_bytes=MAX_MANIFEST_BYTES,
        )
        value = json.loads(
            payload.decode("ascii"), object_pairs_hook=_reject_duplicate_pairs
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise HandoffError(
            "cross-build handoff manifest is not canonical JSON"
        ) from error
    manifest = validate_manifest(value)
    if payload != canonical_manifest_bytes(manifest):
        fail("cross-build handoff manifest bytes are not canonical")
    return manifest


def verify_candidate(
    *,
    candidate_root: Path,
    source_root: Path,
    source_authenticator: SourceAuthenticator = authenticate_source,
) -> dict[str, object]:
    """Authenticate one downloaded candidate against the local checkout."""

    candidate_root = Path(os.path.abspath(candidate_root))
    source_root = source_root.resolve(strict=True)
    directory_before = _require_candidate_directory(candidate_root)
    try:
        entries = {entry.name for entry in candidate_root.iterdir()}
    except OSError as error:
        raise HandoffError("cross-build candidate inventory is unavailable") from error
    if entries != {ARTIFACT_NAME, MANIFEST_NAME}:
        fail("cross-build candidate has a non-canonical file inventory")

    source_before = source_authenticator(source_root)
    manifest = _load_manifest(candidate_root / MANIFEST_NAME)
    expected_source = (
        manifest["source_commit"],
        manifest["workspace_source_manifest_sha256"],
    )
    if source_before != expected_source:
        fail("cross-build candidate does not match the checked-out source")
    artifact = candidate_root / ARTIFACT_NAME
    _require_candidate_file(artifact, "cross-build candidate artifact")
    identity_before = native_checker.stable_artifact_identity(artifact)
    if identity_before != (
        manifest["artifact_sha256"],
        manifest["artifact_size"],
    ):
        fail("cross-build candidate artifact digest or size does not match")
    validate_linux_arm_cdylib(artifact)
    if native_checker.stable_artifact_identity(artifact) != identity_before:
        fail("cross-build candidate artifact changed during authentication")
    if source_authenticator(source_root) != source_before:
        fail("source checkout changed while the cross-build candidate was verified")
    try:
        entries_after = {entry.name for entry in candidate_root.iterdir()}
    except OSError as error:
        raise HandoffError(
            "cross-build candidate inventory changed during authentication"
        ) from error
    if (
        entries_after != {ARTIFACT_NAME, MANIFEST_NAME}
        or _require_candidate_directory(candidate_root) != directory_before
    ):
        fail("cross-build candidate changed during authentication")
    return manifest


def verify_and_stage_candidate(
    *,
    candidate_root: Path,
    stage_artifact: Path,
    source_root: Path,
    source_authenticator: SourceAuthenticator = authenticate_source,
) -> Path:
    """Verify and copy the exact candidate into one fresh private artifact."""

    manifest = verify_candidate(
        candidate_root=candidate_root,
        source_root=source_root,
        source_authenticator=source_authenticator,
    )
    candidate_artifact = Path(os.path.abspath(candidate_root)) / ARTIFACT_NAME
    staged_artifact, digest, size = native_checker.stage_unique_artifact(
        candidate_artifact,
        Path(os.path.abspath(stage_artifact)),
    )
    if (digest, size) != (
        manifest["artifact_sha256"],
        manifest["artifact_size"],
    ):
        fail("staged Linux ARM artifact does not match the verified candidate")
    validate_linux_arm_cdylib(staged_artifact)
    expected_source = (
        manifest["source_commit"],
        manifest["workspace_source_manifest_sha256"],
    )
    if source_authenticator(source_root.resolve(strict=True)) != expected_source:
        fail("source checkout changed while the verified candidate was staged")
    return staged_artifact


def parse_args() -> argparse.Namespace:
    """Parse the seal, verify, or verify-and-stage command line."""

    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("seal", "verify", "verify-stage"))
    parser.add_argument("--candidate-root", required=True, type=Path)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--artifact", type=Path)
    parser.add_argument("--stage-artifact", type=Path)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--linker", default="aarch64-linux-gnu-gcc")
    return parser.parse_args()


def main() -> int:
    """Run one fail-closed handoff operation."""

    args = parse_args()
    if args.mode == "seal":
        if args.artifact is None:
            fail("seal mode requires --artifact")
        if args.stage_artifact is not None:
            fail("seal mode does not accept --stage-artifact")
        seal_candidate(
            artifact=args.artifact,
            candidate_root=args.candidate_root,
            source_root=args.source_root,
            build_environment=capture_build_environment(
                cargo=args.cargo,
                rustc=args.rustc,
                linker=args.linker,
            ),
        )
    elif args.mode == "verify":
        if args.artifact is not None or args.stage_artifact is not None:
            fail("verify mode does not accept artifact staging arguments")
        verify_candidate(
            candidate_root=args.candidate_root,
            source_root=args.source_root,
        )
    else:
        if args.artifact is not None:
            fail("verify-stage mode does not accept --artifact")
        if args.stage_artifact is None:
            fail("verify-stage mode requires --stage-artifact")
        verify_and_stage_candidate(
            candidate_root=args.candidate_root,
            stage_artifact=args.stage_artifact,
            source_root=args.source_root,
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (HandoffError, native_checker.ArtifactContractError) as error:
        print(f"C# Linux ARM cross-build handoff failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
