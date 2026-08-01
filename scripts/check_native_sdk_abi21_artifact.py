#!/usr/bin/env python3
"""Record and verify fail-closed ABI-21 native SDK artifact evidence.

This checker is intentionally host-only.  It authenticates the exact native
artifact exercised by a Node, Python, C/JNI, or C# test lane, calls that
artifact's ABI probe, verifies the required SoraFS and C# privacy entrypoints,
and binds the result to one clean Git revision. Apple and Android release packages continue
to use ``check_mobile_sdk_artifacts.sh``, which additionally authenticates every
cross-compiled slice and its transitive source seal.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import NoReturn


SCHEMA = "iroha.native-sdk-abi21-artifact.v1"
REQUIRED_BRIDGE_ABI_VERSION = 21
MAX_MANIFEST_BYTES = 64 * 1024
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")
TARGET_RE = re.compile(r"[a-z0-9][a-z0-9._+-]{0,127}")
SDK_VALUES = frozenset({"c-jni", "csharp", "node", "python"})

REQUIRED_SYMBOLS: Mapping[str, tuple[str, ...]] = {
    "c-jni": (
        "connect_norito_bridge_abi_version",
        "connect_norito_free",
        "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
    ),
    "csharp": (
        "connect_norito_bridge_abi_version",
        "connect_norito_free",
        "connect_norito_sorafs_reference_validate_appeal_finance_cancel_asset_lock_json",
        "iroha_privacy_capabilities_v1",
        "iroha_privacy_validate_capabilities_v1",
        "iroha_privacy_exact12_fixture_bundle_v1",
        "iroha_privacy_validate_exact12_fixture_bundle_v1",
        "iroha_privacy_free_buffer",
    ),
    "node": (
        "connectNoritoBridgeAbiVersion",
        "sorafsValidateAppealFinanceCancelAssetLockJson",
    ),
    "python": (
        "connect_norito_bridge_abi_version",
        "sorafs_validate_appeal_finance_cancel_asset_lock_json",
    ),
}


class ArtifactContractError(RuntimeError):
    """Raised when native SDK artifact evidence is incomplete or stale."""


def fail(message: str) -> NoReturn:
    """Raise one stable checker error."""

    raise ArtifactContractError(message)


def _plain_object(value: object, label: str) -> dict[str, object]:
    if type(value) is not dict:
        fail(f"{label} must be a JSON object")
    return value


def _reject_duplicate_object_pairs(
    pairs: list[tuple[str, object]],
) -> dict[str, object]:
    result: dict[str, object] = {}
    for key, value in pairs:
        if key in result:
            fail(f"native artifact manifest contains duplicate key {key!r}")
        result[key] = value
    return result


def _run_git(root: Path, arguments: Sequence[str]) -> str:
    environment = os.environ.copy()
    environment["GIT_CONFIG_GLOBAL"] = os.devnull
    environment["GIT_CONFIG_NOSYSTEM"] = "1"
    environment["GIT_OPTIONAL_LOCKS"] = "0"
    result = subprocess.run(
        ["git", "-C", str(root), *arguments],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
        env=environment,
    )
    if result.returncode != 0:
        detail = result.stderr.strip()
        fail(
            "native artifact source state could not be authenticated"
            + (f": {detail}" if detail else "")
        )
    return result.stdout


def source_state(root: Path) -> tuple[str, bool]:
    """Return the exact Git commit and whole-tree cleanliness."""

    commit = _run_git(root, ("rev-parse", "--verify", "HEAD")).strip()
    if COMMIT_RE.fullmatch(commit) is None:
        fail("native artifact source commit is not canonical lowercase Git SHA-1")
    status = _run_git(
        root,
        ("status", "--porcelain=v1", "--untracked-files=all"),
    )
    return commit, not bool(status.strip())


def stable_artifact_identity(path: Path) -> tuple[str, int]:
    """Hash one regular, non-linked file while detecting replacement races."""

    try:
        before = path.lstat()
    except OSError as error:
        raise ArtifactContractError(
            f"native artifact is unavailable: {path}"
        ) from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
    ):
        fail("native artifact must be one non-empty regular file with one hard link")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ArtifactContractError(f"native artifact could not be opened: {path}") from error
    digest = hashlib.sha256()
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            fail("native artifact changed while it was opened")
        while True:
            chunk = os.read(descriptor, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)
    identity_before = (
        opened.st_dev,
        opened.st_ino,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_nlink,
    )
    identity_after = (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_size,
        after.st_mtime_ns,
        after.st_ctime_ns,
        after.st_nlink,
    )
    if identity_before != identity_after:
        fail("native artifact changed while it was hashed")
    return digest.hexdigest(), opened.st_size


def stable_bounded_file_bytes(
    path: Path,
    *,
    label: str,
    maximum_bytes: int,
) -> bytes:
    """Read one bounded regular file without following or racing replacements."""

    try:
        before = path.lstat()
    except OSError as error:
        raise ArtifactContractError(f"{label} is unavailable: {path}") from error
    if (
        not stat.S_ISREG(before.st_mode)
        or stat.S_ISLNK(before.st_mode)
        or before.st_nlink != 1
        or before.st_size <= 0
        or before.st_size > maximum_bytes
    ):
        fail(f"{label} must be one bounded regular file with one hard link")

    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        descriptor = os.open(path, flags)
    except OSError as error:
        raise ArtifactContractError(f"{label} could not be opened: {path}") from error
    chunks: list[bytes] = []
    try:
        opened = os.fstat(descriptor)
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino):
            fail(f"{label} changed while it was opened")
        remaining = maximum_bytes + 1
        while remaining:
            chunk = os.read(descriptor, min(64 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        after = os.fstat(descriptor)
    finally:
        os.close(descriptor)

    def identity(metadata: os.stat_result) -> tuple[int, ...]:
        return (
            metadata.st_dev,
            metadata.st_ino,
            metadata.st_mode,
            metadata.st_size,
            metadata.st_mtime_ns,
            metadata.st_ctime_ns,
            metadata.st_nlink,
        )

    try:
        current = path.lstat()
    except OSError as error:
        raise ArtifactContractError(f"{label} changed while it was read") from error
    opened_identity = identity(opened)
    if (
        opened_identity != identity(after)
        or opened_identity != identity(current)
        or not stat.S_ISREG(opened.st_mode)
        or stat.S_ISLNK(opened.st_mode)
        or opened.st_nlink != 1
    ):
        fail(f"{label} changed while it was read")
    raw = b"".join(chunks)
    if len(raw) != opened.st_size or len(raw) > maximum_bytes:
        fail(f"{label} changed size or exceeded its byte limit while it was read")
    return raw


def probe_c_abi(path: Path, required_symbols: Sequence[str]) -> int:
    """Load one C ABI library and require its exact exported inventory."""

    try:
        library = ctypes.CDLL(str(path))
    except OSError as error:
        raise ArtifactContractError(f"native C ABI artifact could not be loaded: {path}") from error
    missing = [symbol for symbol in required_symbols if not hasattr(library, symbol)]
    if missing:
        fail("native C ABI artifact is missing required symbols: " + ", ".join(missing))
    probe = getattr(library, "connect_norito_bridge_abi_version")
    probe.argtypes = []
    probe.restype = ctypes.c_uint32
    return int(probe())


def _probe_subprocess(
    command: Sequence[str],
    *,
    label: str,
) -> int:
    result = subprocess.run(
        list(command),
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        detail = result.stderr.strip()[:4096]
        fail(f"{label} failed" + (f": {detail}" if detail else ""))
    raw = result.stdout.strip()
    if not raw.isascii() or not raw.isdecimal():
        fail(f"{label} returned a noncanonical ABI version")
    return int(raw)


def probe_node_abi(
    path: Path,
    required_symbols: Sequence[str],
    *,
    node: str = "node",
) -> int:
    """Load one Node addon and call its exact ABI-21 probe."""

    source = r"""
const artifact = process.argv[1];
const required = JSON.parse(process.argv[2]);
let binding;
if (/\.(?:cjs|js)$/iu.test(artifact)) {
  binding = require(artifact);
} else {
  const nativeModule = { exports: {} };
  process.dlopen(nativeModule, artifact);
  binding = nativeModule.exports;
}
const missing = required.filter((name) => typeof binding[name] !== "function");
if (missing.length !== 0) {
  process.stderr.write("missing required exports: " + missing.join(", "));
  process.exit(2);
}
const version = binding.connectNoritoBridgeAbiVersion();
if (!Number.isSafeInteger(version) || version < 0) {
  process.stderr.write("ABI probe returned a non-integer");
  process.exit(3);
}
process.stdout.write(String(version));
"""
    return _probe_subprocess(
        (node, "--eval", source, str(path), json.dumps(list(required_symbols))),
        label="native Node ABI probe",
    )


def probe_python_abi(
    path: Path,
    required_symbols: Sequence[str],
    *,
    python: str = sys.executable,
) -> int:
    """Load one Python extension directly and call its exact ABI-21 probe."""

    source = r"""
import importlib.machinery
import importlib.util
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
required = json.loads(sys.argv[2])
if path.suffix == ".py":
    name = "_iroha_native_abi21_fixture"
    loader = importlib.machinery.SourceFileLoader(name, str(path))
else:
    name = "iroha_python._crypto"
    loader = importlib.machinery.ExtensionFileLoader(name, str(path))
spec = importlib.util.spec_from_loader(name, loader)
if spec is None:
    raise SystemExit("native extension has no import specification")
module = importlib.util.module_from_spec(spec)
loader.exec_module(module)
missing = [name for name in required if not callable(getattr(module, name, None))]
if missing:
    raise SystemExit("missing required exports: " + ", ".join(missing))
version = module.connect_norito_bridge_abi_version()
if type(version) is not int or version < 0:
    raise SystemExit("ABI probe returned a non-integer")
print(version, end="")
"""
    return _probe_subprocess(
        (python, "-I", "-c", source, str(path), json.dumps(list(required_symbols))),
        label="native Python ABI probe",
    )


def probe_artifact(
    sdk: str,
    path: Path,
    *,
    node: str = "node",
    python: str = sys.executable,
) -> int:
    """Probe the exact host artifact selected for one SDK lane."""

    required = REQUIRED_SYMBOLS[sdk]
    if sdk == "node":
        return probe_node_abi(path, required, node=node)
    if sdk == "python":
        return probe_python_abi(path, required, python=python)
    return probe_c_abi(path, required)


def _require_exact_abi(version: int) -> None:
    if type(version) is not int or version != REQUIRED_BRIDGE_ABI_VERSION:
        fail(
            "native artifact bridge ABI must be exactly "
            f"{REQUIRED_BRIDGE_ABI_VERSION}; found {version!r}"
        )


def build_manifest(
    *,
    sdk: str,
    target: str,
    artifact_path: Path,
    source_root: Path,
    probe: Callable[[str, Path], int] = probe_artifact,
) -> dict[str, object]:
    """Authenticate one artifact and return its canonical evidence manifest."""

    if sdk not in SDK_VALUES:
        fail(f"unsupported native SDK lane: {sdk!r}")
    if TARGET_RE.fullmatch(target) is None:
        fail("native SDK target must be a bounded lowercase target token")
    commit_before, clean_before = source_state(source_root)
    if not clean_before:
        fail("native SDK artifacts must be built and tested from a clean source tree")
    digest, size = stable_artifact_identity(artifact_path)
    version = probe(sdk, artifact_path)
    _require_exact_abi(version)
    digest_after, size_after = stable_artifact_identity(artifact_path)
    if (digest_after, size_after) != (digest, size):
        fail("native artifact changed while its ABI and exports were probed")
    commit_after, clean_after = source_state(source_root)
    if commit_after != commit_before or not clean_after:
        fail("native SDK source changed while artifact evidence was collected")
    return {
        "artifact_sha256": digest,
        "artifact_size": size,
        "bridge_abi_version": version,
        "required_symbols": list(REQUIRED_SYMBOLS[sdk]),
        "schema": SCHEMA,
        "sdk": sdk,
        "source_commit": commit_before,
        "source_tree_clean": True,
        "target": target,
    }


def canonical_manifest_bytes(manifest: Mapping[str, object]) -> bytes:
    """Encode one already validated evidence manifest canonically."""

    validated = validate_manifest(dict(manifest))
    return (
        json.dumps(validated, ensure_ascii=True, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("ascii")


def validate_manifest(value: object) -> dict[str, object]:
    """Validate the exact ABI-21 artifact evidence schema."""

    manifest = _plain_object(value, "native artifact manifest")
    expected_keys = {
        "artifact_sha256",
        "artifact_size",
        "bridge_abi_version",
        "required_symbols",
        "schema",
        "sdk",
        "source_commit",
        "source_tree_clean",
        "target",
    }
    if set(manifest) != expected_keys:
        fail("native artifact manifest field inventory is not exact")
    sdk = manifest["sdk"]
    if type(sdk) is not str or sdk not in SDK_VALUES:
        fail("native artifact manifest SDK lane is unsupported")
    target = manifest["target"]
    if type(target) is not str or TARGET_RE.fullmatch(target) is None:
        fail("native artifact manifest target is not canonical")
    digest = manifest["artifact_sha256"]
    if type(digest) is not str or SHA256_RE.fullmatch(digest) is None:
        fail("native artifact manifest SHA-256 is not canonical")
    size = manifest["artifact_size"]
    if type(size) is not int or size <= 0:
        fail("native artifact manifest size must be a positive integer")
    version = manifest["bridge_abi_version"]
    _require_exact_abi(version)
    commit = manifest["source_commit"]
    if type(commit) is not str or COMMIT_RE.fullmatch(commit) is None:
        fail("native artifact manifest source commit is not canonical")
    if manifest["source_tree_clean"] is not True:
        fail("native artifact manifest must attest a clean source tree")
    if manifest["schema"] != SCHEMA:
        fail("native artifact manifest schema is unsupported")
    required = manifest["required_symbols"]
    if type(required) is not list or tuple(required) != REQUIRED_SYMBOLS[sdk]:
        fail("native artifact required-symbol inventory is not exact")
    return dict(manifest)


def load_manifest(path: Path) -> dict[str, object]:
    """Read one bounded, canonical evidence manifest."""

    raw = stable_bounded_file_bytes(
        path,
        label="native artifact manifest",
        maximum_bytes=MAX_MANIFEST_BYTES,
    )
    try:
        parsed = json.loads(
            raw,
            object_pairs_hook=_reject_duplicate_object_pairs,
        )
    except (UnicodeError, json.JSONDecodeError) as error:
        raise ArtifactContractError(
            f"native artifact manifest is unreadable: {path}"
        ) from error
    validated = validate_manifest(parsed)
    if raw != canonical_manifest_bytes(validated):
        fail("native artifact manifest JSON is not canonical")
    return validated


def verify_manifest(
    manifest: Mapping[str, object],
    *,
    artifact_path: Path,
    source_root: Path,
    probe: Callable[[str, Path], int] = probe_artifact,
) -> None:
    """Re-authenticate source, artifact bytes, exports, and exact ABI."""

    expected = validate_manifest(dict(manifest))
    commit_before, clean_before = source_state(source_root)
    if (
        not clean_before
        or commit_before != expected["source_commit"]
        or expected["source_tree_clean"] is not True
    ):
        fail("native artifact manifest does not match the current clean source revision")
    digest, size = stable_artifact_identity(artifact_path)
    if digest != expected["artifact_sha256"] or size != expected["artifact_size"]:
        fail("native artifact bytes do not match the evidence manifest")
    version = probe(str(expected["sdk"]), artifact_path)
    _require_exact_abi(version)
    if version != expected["bridge_abi_version"]:
        fail("native artifact ABI probe does not match the evidence manifest")
    digest_after, size_after = stable_artifact_identity(artifact_path)
    if (digest_after, size_after) != (digest, size):
        fail("native artifact changed while its ABI and exports were verified")
    commit_after, clean_after = source_state(source_root)
    if commit_after != commit_before or not clean_after:
        fail("native SDK source changed while artifact evidence was verified")


def _exclusive_write(path: Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    flags = (
        os.O_CREAT
        | os.O_EXCL
        | os.O_WRONLY
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as error:
        raise ArtifactContractError(
            f"native artifact manifest output must be fresh: {path}"
        ) from error
    try:
        offset = 0
        while offset < len(payload):
            offset += os.write(descriptor, payload[offset:])
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def parse_args() -> argparse.Namespace:
    """Parse the record/verify command line."""

    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("record", "verify"))
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--node", default="node")
    parser.add_argument("--python", default=sys.executable)
    parser.add_argument("--sdk", choices=tuple(sorted(SDK_VALUES)))
    parser.add_argument("--target")
    return parser.parse_args()


def main() -> int:
    """Run the selected fail-closed artifact operation."""

    args = parse_args()
    # Preserve the final path component so ``stable_artifact_identity`` can
    # reject a symlink rather than silently authenticating its target.
    artifact = Path(os.path.abspath(args.artifact))
    if not artifact.exists() and not artifact.is_symlink():
        fail(f"native artifact is unavailable: {artifact}")
    source_root = args.source_root.resolve(strict=True)
    probe = lambda sdk, path: probe_artifact(
        sdk,
        path,
        node=args.node,
        python=args.python,
    )
    if args.mode == "record":
        if args.sdk is None or args.target is None:
            fail("record mode requires --sdk and --target")
        manifest = build_manifest(
            sdk=args.sdk,
            target=args.target,
            artifact_path=artifact,
            source_root=source_root,
            probe=probe,
        )
        _exclusive_write(args.manifest, canonical_manifest_bytes(manifest))
    else:
        if args.sdk is not None or args.target is not None:
            fail("verify mode reads SDK and target from the manifest")
        verify_manifest(
            load_manifest(Path(os.path.abspath(args.manifest))),
            artifact_path=artifact,
            source_root=source_root,
            probe=probe,
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ArtifactContractError as error:
        print(f"native SDK ABI-21 artifact check failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
