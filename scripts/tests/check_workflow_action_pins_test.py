from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check_workflow_action_pins.py"
PIN = "0123456789abcdef0123456789abcdef01234567"
DIGEST = "sha256:" + ("ab" * 32)


def run_checker(workflows_dir: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(CHECKER),
            "--workflows-dir",
            str(workflows_dir),
        ],
        text=True,
        capture_output=True,
        check=False,
    )


def write_workflow(workflows_dir: Path, name: str, uses: str) -> None:
    workflows_dir.mkdir(parents=True, exist_ok=True)
    (workflows_dir / name).write_text(
        "name: pin-test\n"
        "jobs:\n"
        "  guard:\n"
        "    runs-on: ubuntu-latest\n"
        "    steps:\n"
        f"      - uses: {uses}\n",
        encoding="utf-8",
    )


class WorkflowActionPinsTests(unittest.TestCase):
    def test_accepts_full_commit_local_action_and_container_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workflows = Path(temporary) / "workflows"
            write_workflow(workflows, "remote.yml", f"actions/checkout@{PIN} # v4")
            write_workflow(workflows, "local.yaml", "./.github/actions/local")
            write_workflow(
                workflows,
                "container.yml",
                f"docker://example.invalid/tool@{DIGEST}",
            )

            result = run_checker(workflows)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("every workflow and composite remote action", result.stdout)

    def test_rejects_tag_branch_short_sha_and_interpolation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workflows = Path(temporary) / "workflows"
            write_workflow(workflows, "tag.yml", "actions/checkout@v4")
            write_workflow(workflows, "branch.yml", "owner/action@main")
            write_workflow(workflows, "short.yml", "owner/action@0123456789ab")
            write_workflow(workflows, "expression.yml", "${{ matrix.action }}")

            result = run_checker(workflows)

        self.assertEqual(result.returncode, 1)
        self.assertIn("tag.yml:6", result.stderr)
        self.assertIn("branch.yml:6", result.stderr)
        self.assertIn("short.yml:6", result.stderr)
        self.assertIn("expression.yml:6", result.stderr)

    def test_rejects_container_tag_and_non_sha256_digest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            workflows = Path(temporary) / "workflows"
            write_workflow(workflows, "tag.yml", "docker://alpine:3.20")
            write_workflow(workflows, "digest.yml", "docker://alpine@sha512:abcd")

            result = run_checker(workflows)

        self.assertEqual(result.returncode, 1)
        self.assertIn("immutable sha256 image digest", result.stderr)
        self.assertIn("64 lowercase hex digits", result.stderr)

    def test_rejects_floating_remote_action_inside_composite(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            github_dir = Path(temporary) / ".github"
            workflows = github_dir / "workflows"
            write_workflow(workflows, "guard.yml", f"actions/checkout@{PIN}")
            composite = github_dir / "actions" / "build" / "action.yml"
            composite.parent.mkdir(parents=True)
            composite.write_text(
                "name: build\n"
                "runs:\n"
                "  using: composite\n"
                "  steps:\n"
                "    - uses: owner/build-action@main\n",
                encoding="utf-8",
            )

            result = run_checker(workflows)

        self.assertEqual(result.returncode, 1)
        self.assertIn("action.yml:5", result.stderr)

    def test_rejects_missing_or_empty_workflow_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary)
            missing = run_checker(temporary_path / "missing")
            self.assertEqual(missing.returncode, 1)
            self.assertIn("directory is missing", missing.stderr)

            empty_dir = temporary_path / "empty"
            empty_dir.mkdir()
            empty = run_checker(empty_dir)

        self.assertEqual(empty.returncode, 1)
        self.assertIn("contains no YAML files", empty.stderr)

    def test_repository_workflows_are_fully_pinned(self) -> None:
        result = run_checker(ROOT / ".github" / "workflows")
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
