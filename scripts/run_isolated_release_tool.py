#!/usr/bin/env -S python3 -I -S
"""Run one reviewed release helper without exposing ``scripts/`` on sys.path."""

from __future__ import annotations

import hashlib
import os
import stat
import sys
import types
from pathlib import Path


ALLOWED_TOOLS = frozenset(
    {
        "build_release_oci_archive.py",
        "build_release_tar_zst.py",
        "capture_release_command.py",
        "copy_release_file.py",
        "copy_release_tree.py",
        "fastpq/rollout_manifest_summary.py",
        "generate_release_manifest.py",
        "verify_release_prebuilt_provenance.py",
        "write_release_checksum.py",
        "write_release_sha256sums.py",
    }
)
SCRIPT_DIRECTORY = Path(__file__).resolve().parent
MAX_TOOL_BYTES = 1024 * 1024
RELEASE_ARTIFACT_CONTRACT_SHA256 = (
    "ad4a5bd832f95a55ef2d4bee8a451ef3f5af14d244a1a14a232ec5cc4d59e253"
)
REVIEWED_TOOL_SHA256 = {
    "verify_release_prebuilt_provenance.py": "861f9ce31001acd2ba5331d3d099ca751ac733682e85d015dbc905941109184d",
}


def _stable_regular_sibling(name: str) -> tuple[Path, bytes]:
    """Read one regular, singly linked sibling through one no-follow fd."""

    path = SCRIPT_DIRECTORY / name
    try:
        file_flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW
        directory_flags = file_flags | os.O_DIRECTORY
    except AttributeError as error:  # pragma: no cover - unsupported release host
        raise RuntimeError("release host lacks secure no-follow file opens") from error
    directory_descriptor = os.open(SCRIPT_DIRECTORY, directory_flags)
    try:
        directory_before = os.fstat(directory_descriptor)
        if not stat.S_ISDIR(directory_before.st_mode) or directory_before.st_mode & (
            stat.S_IWGRP | stat.S_IWOTH
        ):
            raise RuntimeError(
                "isolated release tool directory must not be group- or world-writable"
            )
        descriptor = os.open(name, file_flags, dir_fd=directory_descriptor)
        try:
            before = os.fstat(descriptor)
            if (
                not stat.S_ISREG(before.st_mode)
                or before.st_nlink != 1
                or before.st_mode & (stat.S_IWGRP | stat.S_IWOTH)
            ):
                raise RuntimeError(
                    "isolated release tool must be a singly linked, "
                    f"non-shared-writable regular file: {name}"
                )
            if before.st_size > MAX_TOOL_BYTES:
                raise RuntimeError(f"isolated release tool exceeds size limit: {name}")
            payload = bytearray()
            while True:
                chunk = os.read(
                    descriptor, min(65536, MAX_TOOL_BYTES + 1 - len(payload))
                )
                if not chunk:
                    break
                payload.extend(chunk)
                if len(payload) > MAX_TOOL_BYTES:
                    raise RuntimeError(
                        f"isolated release tool exceeds size limit: {name}"
                    )
            after = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        directory_after = os.fstat(directory_descriptor)
    finally:
        os.close(directory_descriptor)
    identity = lambda info: (
        info.st_dev,
        info.st_ino,
        info.st_mode,
        info.st_nlink,
        info.st_size,
        info.st_mtime_ns,
        info.st_ctime_ns,
    )
    if (
        identity(directory_before) != identity(directory_after)
        or identity(before) != identity(after)
        or len(payload) != before.st_size
    ):
        raise RuntimeError(f"isolated release tool changed while being read: {name}")
    return path, bytes(payload)


def _load_contract() -> None:
    name = "release_artifact_contract"
    path, payload = _stable_regular_sibling(f"{name}.py")
    if hashlib.sha256(payload).hexdigest() != RELEASE_ARTIFACT_CONTRACT_SHA256:
        raise RuntimeError("isolated release artifact contract differs from review")
    module = types.ModuleType(name)
    module.__file__ = str(path)
    module.__package__ = ""
    sys.modules[name] = module
    exec(compile(payload, str(path), "exec"), module.__dict__)


def _stable_reviewed_source(relative: str) -> tuple[Path, bytes]:
    """Read an allowed top-level or nested source without pathname reopening."""

    if "/" not in relative:
        path, payload = _stable_regular_sibling(relative)
    else:
        contract = sys.modules["release_artifact_contract"]
        _info, payload = contract.stable_read_relative(
            SCRIPT_DIRECTORY,
            relative,
            max_size=MAX_TOOL_BYTES,
            return_payload=True,
        )
        if payload is None:  # pragma: no cover - requested above
            raise RuntimeError(f"unable to read isolated release source: {relative}")
        path = SCRIPT_DIRECTORY / relative
    expected_sha256 = REVIEWED_TOOL_SHA256.get(relative)
    if (
        expected_sha256 is not None
        and hashlib.sha256(payload).hexdigest() != expected_sha256
    ):
        raise RuntimeError(f"isolated release tool differs from review: {relative}")
    return path, payload


def _ensure_package(name: str) -> types.ModuleType:
    module = sys.modules.get(name)
    if isinstance(module, types.ModuleType):
        return module
    module = types.ModuleType(name)
    module.__package__ = name
    module.__path__ = []
    sys.modules[name] = module
    return module


def _load_captured_module(name: str, relative: str) -> None:
    path, payload = _stable_reviewed_source(relative)
    package, _, child = name.rpartition(".")
    module = types.ModuleType(name)
    module.__file__ = str(path)
    module.__package__ = package
    sys.modules[name] = module
    if package:
        setattr(_ensure_package(package), child, module)
    original_path = list(sys.path)
    try:
        exec(compile(payload, str(path), "exec"), module.__dict__)
    finally:
        sys.path[:] = original_path


def _load_fastpq_summary_dependencies() -> None:
    """Preload the nested summary dependency graph from captured exact bytes."""

    scripts = _ensure_package("scripts")
    fastpq = _ensure_package("scripts.fastpq")
    setattr(scripts, "fastpq", fastpq)
    _load_captured_module(
        "scripts.fastpq.validate_row_usage_snapshot",
        "fastpq/validate_row_usage_snapshot.py",
    )
    _load_captured_module(
        "export_prometheus", "acceleration/export_prometheus.py"
    )
    _load_captured_module("scripts.fastpq.wrap_benchmark", "fastpq/wrap_benchmark.py")


def main() -> int:
    if len(sys.argv) < 2:
        raise SystemExit("isolated release tool requires a reviewed target")
    target = sys.argv[1]
    _load_contract()
    if target == "--stdin":
        sys.argv = ["-", *sys.argv[2:]]
        source = sys.stdin.read()
        exec(compile(source, "<isolated-release-stdin>", "exec"), {"__name__": "__main__"})
        return 0

    requested = Path(target)
    try:
        relative = requested.resolve().relative_to(SCRIPT_DIRECTORY).as_posix()
    except ValueError:
        relative = ""
    if relative not in ALLOWED_TOOLS:
        raise SystemExit("isolated release tool target is not reviewed")
    if relative == "fastpq/rollout_manifest_summary.py":
        _load_fastpq_summary_dependencies()
    path, payload = _stable_reviewed_source(relative)
    sys.argv = [str(path), *sys.argv[2:]]
    namespace = {
        "__file__": str(path),
        "__name__": "__main__",
        "__package__": "",
        "__spec__": None,
    }
    exec(compile(payload, str(path), "exec"), namespace)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
