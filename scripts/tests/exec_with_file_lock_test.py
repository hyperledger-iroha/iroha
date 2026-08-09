#!/usr/bin/env python3
"""Focused tests for authenticated multi-path command locking."""

from __future__ import annotations

import fcntl
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "scripts/exec_with_file_lock.py"


class ExecWithFileLockTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_runner(
        self, *arguments: str, environment: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-I", "-S", str(RUNNER), *arguments],
            check=False,
            capture_output=True,
            text=True,
            env=environment,
        )

    def test_holds_every_lock_in_stable_path_order(self) -> None:
        first = self.root / "first.lock"
        second = self.root / "second.lock"
        child = r"""
import fcntl
import os
from pathlib import Path
import sys

descriptors = [int(value) for value in os.environ["TEST_LOCK_FDS"].split(",")]
paths = [Path(value) for value in sys.argv[1:]]
assert len(descriptors) == len(paths) == 2
for descriptor, path in zip(descriptors, paths, strict=True):
    held = os.fstat(descriptor)
    current = path.stat()
    assert (held.st_dev, held.st_ino) == (current.st_dev, current.st_ino)
    contender = os.open(path, os.O_RDWR)
    try:
        try:
            fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            pass
        else:
            raise AssertionError(f"lock was not held: {path}")
    finally:
        os.close(contender)
print("held")
"""
        result = self.run_runner(
            "TEST_LOCK_FDS",
            str(second),
            str(first),
            "--",
            sys.executable,
            "-I",
            "-S",
            "-c",
            child,
            str(first),
            str(second),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "held\n")

    def test_rejects_contention_duplicate_paths_and_symlinks(self) -> None:
        lock = self.root / "contended.lock"
        descriptor = os.open(lock, os.O_RDWR | os.O_CREAT, 0o600)
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
            contended = self.run_runner(
                "TEST_LOCK_FDS", str(lock), "--", "/usr/bin/true"
            )
        finally:
            os.close(descriptor)
        self.assertNotEqual(contended.returncode, 0)
        self.assertIn("another process holds", contended.stderr)

        duplicate = self.run_runner(
            "TEST_LOCK_FDS", str(lock), str(lock), "--", "/usr/bin/true"
        )
        self.assertNotEqual(duplicate.returncode, 0)
        self.assertIn("duplicate lock paths", duplicate.stderr)

        target = self.root / "target.lock"
        target.touch()
        symbolic = self.root / "symbolic.lock"
        symbolic.symlink_to(target)
        unsafe = self.run_runner(
            "TEST_LOCK_FDS", str(symbolic), "--", "/usr/bin/true"
        )
        self.assertNotEqual(unsafe.returncode, 0)
        self.assertIn("unable to open lock file", unsafe.stderr)

    def test_requires_explicit_separator_and_canonical_environment_name(self) -> None:
        lock = self.root / "contract.lock"
        missing_separator = self.run_runner(
            "TEST_LOCK_FDS", str(lock), "/usr/bin/true"
        )
        self.assertNotEqual(missing_separator.returncode, 0)
        self.assertIn("usage:", missing_separator.stderr)

        malformed_name = self.run_runner(
            "NOT-AN-ENV", str(lock), "--", "/usr/bin/true"
        )
        self.assertNotEqual(malformed_name.returncode, 0)
        self.assertIn("environment name is not canonical", malformed_name.stderr)


if __name__ == "__main__":
    unittest.main()
