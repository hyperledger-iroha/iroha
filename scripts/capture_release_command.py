#!/usr/bin/env python3
"""Capture bounded deterministic command output into an exclusive file."""

from __future__ import annotations

import argparse
import os
import re
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

from release_artifact_contract import (
    ReleaseArtifactError,
    exclusive_write_bytes,
    stable_hash_path,
    stable_hash_relative,
    stable_read_relative,
)


MAX_CAPTURE_BYTES = 16 * 1024 * 1024
MAX_EXECUTABLE_BYTES = 256 * 1024 * 1024


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True)
    parser.add_argument("--executable-root", required=True)
    parser.add_argument("--executable-relative", required=True)
    parser.add_argument("--trusted-executable-sha256")
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.arguments or args.arguments[0] != "--":
        parser.error("command arguments must follow --")
    root = Path(args.executable_root)
    relative = args.executable_relative
    try:
        before, executable_payload = stable_read_relative(
            root,
            relative,
            max_size=MAX_EXECUTABLE_BYTES,
            return_payload=True,
        )
        assert executable_payload is not None
        if args.trusted_executable_sha256 is not None:
            if (
                re.fullmatch(
                    r"[0-9a-f]{64}",
                    args.trusted_executable_sha256,
                )
                is None
            ):
                raise ReleaseArtifactError(
                    "trusted release executable SHA256 must be 64 lowercase hex"
                )
            if before.sha256 != args.trusted_executable_sha256:
                raise ReleaseArtifactError(
                    "release executable SHA256 is not trusted"
                )
        if not before.mode & stat.S_IXUSR:
            raise ReleaseArtifactError(
                "captured release executable must be owner-executable"
            )
        temp_parent = os.path.realpath(tempfile.gettempdir())
        with tempfile.TemporaryDirectory(
            prefix="iroha-release-command.",
            dir=temp_parent,
        ) as private_directory_raw:
            private_executable = (
                Path(private_directory_raw) / "release-executable"
            )
            exclusive_write_bytes(
                private_executable,
                executable_payload,
                mode=0o755,
            )
            private_before = stable_hash_path(private_executable)
            if private_before.sha256 != before.sha256:
                raise ReleaseArtifactError(
                    "private release executable digest mismatch"
                )
            command_environment = os.environ.copy()
            command_environment[
                "IROHA_RELEASE_ORIGINAL_EXECUTABLE_ROOT"
            ] = os.path.abspath(root)
            process = subprocess.Popen(
                [str(private_executable), *args.arguments[1:]],
                stdout=subprocess.PIPE,
                stderr=None,
                env=command_environment,
            )
            assert process.stdout is not None
            captured = process.stdout.read(MAX_CAPTURE_BYTES + 1)
            if len(captured) > MAX_CAPTURE_BYTES:
                process.kill()
                process.wait()
                raise ReleaseArtifactError(
                    f"captured release output exceeds {MAX_CAPTURE_BYTES} bytes"
                )
            returncode = process.wait()
            after = stable_hash_relative(root, relative)
            if before != after:
                raise ReleaseArtifactError(
                    "release executable changed while its output was captured"
                )
            if stable_hash_path(private_executable) != private_before:
                raise ReleaseArtifactError(
                    "private release executable changed while its output was "
                    "captured"
                )
            if returncode != 0:
                return returncode
            exclusive_write_bytes(Path(args.output), captured)
    except (OSError, ReleaseArtifactError, subprocess.SubprocessError) as exc:
        print(f"release command capture error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
