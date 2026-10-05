#!/usr/bin/env python3
"""Run the authenticated ZIP and public Swift SDK executables in Release.

Requires Python 3.10+, macOS, full Xcode/Swift, the installed complete XCFramework selected by
Package.swift, and a ZIP already authenticated by validate_norito_bridge_archive.
Pass that verifier's archive_sha256 as --archive-sha256. This consumer gate does
not replace artifact authentication. MOBILE_SDK_APPLE_ARTIFACT_DIR and
DEVELOPER_DIR are inherited; local-unit artifacts are refused. Dedicated external
work directories retain fixtures/reports; explicit persistent scratch paths and
SwiftPM's cache are reused without cleaning. Only generated fixture files change.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "scripts/fixtures/swift_release_consumers"


def digest(path: Path) -> str:
    """Hash an input or staged archive without loading it into memory."""
    result = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            result.update(chunk)
    return result.hexdigest()


def regular_destination(path: Path) -> None:
    """Refuse aliases and non-files before replacing a generated fixture."""
    if os.path.lexists(path):
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1:
            raise ValueError(f"generated destination must be a single-link regular file: {path}")


def write_file(path: Path, payload: bytes) -> None:
    """Atomically update one generated file, preserving unchanged fixtures."""
    regular_destination(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() and path.read_bytes() == payload:
        return
    descriptor, temporary = tempfile.mkstemp(prefix=".consumer-", dir=path.parent)
    try:
        with os.fdopen(descriptor, "wb") as output:
            output.write(payload)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def stage_archive(source: Path, destination: Path, expected: str) -> None:
    """Copy only the authenticated bytes, leaving an existing ZIP intact on error."""
    regular_destination(destination)
    if destination.exists() and digest(destination) == expected:
        return
    descriptor, temporary = tempfile.mkstemp(prefix=".consumer-archive-", dir=destination.parent)
    try:
        with source.open("rb") as archive, os.fdopen(descriptor, "wb") as output:
            shutil.copyfileobj(archive, output, length=1024 * 1024)
        if digest(Path(temporary)) != expected:
            raise ValueError("ZIP bytes do not match the authenticated archive SHA-256")
        os.replace(temporary, destination)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def run_consumers(arguments: argparse.Namespace) -> dict[str, object]:
    """Stage maintained fixtures and require both actual Release executions."""
    if not re.fullmatch(r"[0-9a-f]{64}", arguments.archive_sha256):
        raise ValueError("--archive-sha256 must be the verifier's lowercase SHA-256")
    if "MOBILE_SDK_LOCAL_UNIT_ARTIFACT_DIR" in os.environ:
        raise ValueError("Release consumers cannot use local-unit artifacts")
    sdk = arguments.sdk_path.resolve(strict=True)
    work = arguments.work_dir.resolve()
    if work == ROOT or ROOT in work.parents or work == sdk or sdk in work.parents:
        raise ValueError("--work-dir must be outside the reviewed source tree")
    archive = arguments.archive.resolve(strict=True)
    if digest(archive) != arguments.archive_sha256:
        raise ValueError("ZIP bytes do not match the authenticated archive SHA-256")
    work.mkdir(parents=True, exist_ok=True)
    report: dict[str, object] = {
        "status": "running", "archive_sha256": arguments.archive_sha256,
        "sdk_path": str(sdk), "consumers": [],
    }
    environment = {**os.environ, "IROHA_RELEASE_CONSUMER_SDK_PATH": str(sdk)}
    try:
        for kind, product, scratch in (
            ("archive", "ArchiveConsumer", arguments.archive_scratch),
            ("sdk", "IrohaNativeConsumer", arguments.sdk_scratch),
        ):
            package = work / kind
            for relative in (Path("Package.swift"), Path("Sources") / product / "main.swift"):
                write_file(package / relative, (FIXTURES / kind / relative).read_bytes())
            if kind == "archive":
                stage_archive(archive, package / "NoritoBridge.xcframework.zip", arguments.archive_sha256)
            else:
                write_file(package / "Package.resolved", (sdk / "Package.resolved").read_bytes())
            command = [
                str(arguments.swift), "run", "--package-path", str(package),
                "--configuration", "release", "--disable-automatic-resolution",
                "--scratch-path", str(scratch.resolve()),
            ]
            if arguments.cache_path:
                command += ["--cache-path", str(arguments.cache_path.resolve())]
            command.append(product)
            print(f"[swift-release-consumers] executing {kind} in Release", flush=True)
            result = subprocess.run(command, env=environment, check=False)
            report["consumers"].append({"consumer": kind, "exit_code": result.returncode, "command": command})
            if result.returncode != 0:
                raise RuntimeError(f"{kind} Release consumer failed with exit {result.returncode}")
        report["status"] = "passed"
        return report
    finally:
        if report["status"] != "passed":
            report["status"] = "failed"
        write_file(work / "report.json", (json.dumps(report, indent=2) + "\n").encode())


def main() -> int:
    """Parse explicit artifact/cache inputs and run the maintained consumers."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--archive-sha256", required=True)
    parser.add_argument("--sdk-path", type=Path, default=ROOT / "IrohaSwift")
    parser.add_argument("--work-dir", type=Path, required=True)
    parser.add_argument("--archive-scratch", type=Path, required=True)
    parser.add_argument("--sdk-scratch", type=Path, required=True)
    parser.add_argument("--cache-path", type=Path)
    parser.add_argument("--swift", type=Path, default=Path("/usr/bin/swift"))
    result = run_consumers(parser.parse_args())
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, RuntimeError) as error:
        print(f"[swift-release-consumers] error: {error}", file=sys.stderr)
        raise SystemExit(1)
