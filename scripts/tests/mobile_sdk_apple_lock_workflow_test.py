#!/usr/bin/env python3
"""Exercise the Apple artifact workflow's independent reviewed lock handoff."""

from __future__ import annotations

import hashlib
import re
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/mobile_sdk_artifacts.yml"
APPLE_WORKFLOWS = (
    (WORKFLOW, "apple-mobile-sdk", 3),
    (ROOT / ".github/workflows/sorafs-orchestrator-sdk.yml", "sdk-parity", 2),
    (ROOT / ".github/workflows/numeric_v1_sdk.yml", "swift", 1),
)
STEP_NAME = "Materialize the authenticated external Apple Cargo lock"
LOCK_INPUT = b'version = 4\n\n[[package]]\nname = "reviewed-fixture"\nversion = "1.0.0"\n'


def materialization_step(workflow: Path = WORKFLOW) -> str:
    """Read the exact checked-in shell step, without a YAML dependency."""
    source = workflow.read_text(encoding="utf-8")
    match = re.search(
        rf"(?m)^      - name: {re.escape(STEP_NAME)}\n"
        r"(?:        working-directory: \$\{\{ github.workspace \}\}\n)?"
        r"        shell: bash\n        run: \|\n((?:          .*\n)+)",
        source,
    )
    if match is None:
        raise AssertionError("Apple external lock materialization step is missing")
    return textwrap.dedent(match.group(1))


class AppleWorkflowLockTests(unittest.TestCase):
    """Execute the workflow shell against the real lock materialization owner."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name).resolve()
        self.root = self.base / "source"
        (self.root / "ci").mkdir(parents=True)
        self.source_lock = self.root / "Cargo.lock"
        self.source_lock.write_bytes(LOCK_INPUT)
        self.source_lock.chmod(0o400)
        # This explicit fixture declaration owns only these public test bytes.
        # All materialization and path authentication code remains unchanged.
        owner = (ROOT / "ci/privacy_sdk_cargo_lockfile.sh").read_text(encoding="utf-8")
        declaration = r'(readonly PRIVACY_SDK_CANONICAL_CARGO_LOCK_SHA256=\\\n")[0-9a-f]{64}("\n)'
        owner, replacements = re.subn(
            declaration,
            lambda match: match.group(1) + hashlib.sha256(LOCK_INPUT).hexdigest() + match.group(2),
            owner,
        )
        self.assertEqual(replacements, 1)
        (self.root / "ci/privacy_sdk_cargo_lockfile.sh").write_text(owner, encoding="utf-8")
        self.runner_temp = self.base / "runner-temp"
        self.runner_temp.mkdir(mode=0o700)
        self.github_env = self.base / "github-env"
        self.github_env.touch()
        self.source_identity = self.source_lock.stat()

    def run_step(
        self, *, runner_temp: Path | None = None, workflow: Path = WORKFLOW,
    ) -> subprocess.CompletedProcess[str]:
        """Execute the original workflow command without Cargo or network activity."""
        return subprocess.run(
            ["/bin/bash", "-e", "-o", "pipefail", "-c", materialization_step(workflow)],
            cwd=self.root,
            env={
                "PATH": "/usr/bin:/bin",
                "GITHUB_WORKSPACE": str(self.root),
                "RUNNER_TEMP": str(runner_temp or self.runner_temp),
                "GITHUB_ENV": str(self.github_env),
                "MOBILE_SDK_PYTHON_BINARY": str(Path(sys.executable).resolve()),
            },
            capture_output=True,
            text=True,
            check=False,
        )

    def test_real_owner_creates_an_independent_read_only_external_snapshot(self) -> None:
        for workflow, _, _ in APPLE_WORKFLOWS:
            with self.subTest(workflow=workflow.name):
                runner_temp = self.runner_temp / workflow.stem
                runner_temp.mkdir(mode=0o700)
                self.github_env.write_bytes(b"")
                result = self.run_step(runner_temp=runner_temp, workflow=workflow)
                self.assertEqual(result.returncode, 0, result.stderr)
                selected = runner_temp / "iroha-mobile-apple-lock/Cargo.lock"
                self.assertEqual(selected.resolve(strict=True), selected)
                self.assertEqual(selected.read_bytes(), LOCK_INPUT)
                metadata = selected.stat()
                self.assertEqual(stat.S_IMODE(metadata.st_mode), 0o400)
                self.assertEqual(metadata.st_nlink, 1)
                self.assertNotEqual(
                    (metadata.st_dev, metadata.st_ino),
                    (self.source_identity.st_dev, self.source_identity.st_ino),
                )
                self.assertEqual(self.source_lock.read_bytes(), LOCK_INPUT)
                after = self.source_lock.stat()
                for field in ("st_dev", "st_ino", "st_mode", "st_size", "st_mtime_ns", "st_ctime_ns"):
                    self.assertEqual(getattr(after, field), getattr(self.source_identity, field))
                self.assertEqual(
                    self.github_env.read_text(),
                    f"IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH={selected}\n",
                )

    def test_unreviewed_root_graph_is_refused_before_export(self) -> None:
        self.source_lock.chmod(0o600)
        self.source_lock.write_bytes(LOCK_INPUT + b"# unreviewed graph change\n")
        self.source_lock.chmod(0o400)
        result = self.run_step()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("differs from its authenticated reviewed state", result.stderr)
        self.assertEqual(self.github_env.read_bytes(), b"")
        self.assertFalse((self.runner_temp / "iroha-mobile-apple-lock/Cargo.lock").exists())

    def test_existing_snapshot_directory_is_preserved_and_refused(self) -> None:
        selected_parent = self.runner_temp / "iroha-mobile-apple-lock"
        selected_parent.mkdir()
        retained = selected_parent / "Cargo.lock"
        retained.write_bytes(b"retained existing input\n")
        result = self.run_step()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(retained.read_bytes(), b"retained existing input\n")
        self.assertEqual(self.github_env.read_bytes(), b"")

    def test_source_contained_snapshot_is_refused(self) -> None:
        result = self.run_step(runner_temp=self.root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("explicit canonical external Cargo.lock", result.stderr)
        self.assertEqual(self.github_env.read_bytes(), b"")

    def test_every_apple_artifact_owner_receives_the_same_external_selection(self) -> None:
        for workflow, job_name, count in APPLE_WORKFLOWS:
            with self.subTest(workflow=workflow.name):
                source = workflow.read_text(encoding="utf-8")
                job = re.search(
                    rf"(?ms)^  {re.escape(job_name)}:\n.*?(?=^  [A-Za-z0-9_-]+:\n|\Z)",
                    source,
                )
                self.assertIsNotNone(job)
                apple = job.group(0)
                calls = [
                    line for line in apple.splitlines()
                    if (
                        "scripts/build_norito_xcframework.sh" in line
                        or "scripts/check_mobile_sdk_artifacts.sh --apple-only" in line
                        or "scripts/package_mobile_sdk_artifacts.sh --apple " in line
                    )
                ]
                self.assertEqual(len(calls), count)
                for call in calls:
                    self.assertIn('--lockfile-path "$IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH"', call)
                    self.assertNotIn('$GITHUB_WORKSPACE/Cargo.lock', call)
                    self.assertNotIn("--local-integration", call)
                    self.assertNotIn("--allow-dirty-source", call)
                self.assertLess(apple.index(STEP_NAME), apple.index(calls[0]))
                self.assertEqual(materialization_step(workflow), materialization_step())


if __name__ == "__main__":
    unittest.main()
