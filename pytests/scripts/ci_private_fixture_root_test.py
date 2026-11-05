#!/usr/bin/env python3
"""Exercise private CI roots on the real filesystem without changing HOME."""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "ci_private_fixture_root", ROOT / "scripts/ci_private_fixture_root.py"
)
assert SPEC and SPEC.loader
HELPER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HELPER)


class PrivateFixtureRootTests(unittest.TestCase):
    """Creation and refusal controls retain the actual service-account policy."""

    def setUp(self) -> None:
        self.home = HELPER.service_home()
        self.temporary = tempfile.TemporaryDirectory(prefix=".t-", dir=self.home)
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def create_root(self) -> Path:
        root = HELPER.create_fixture_root()
        self.addCleanup(root.rmdir)
        return root

    def test_fresh_private_roots_use_the_system_account_home(self) -> None:
        original_home = os.environ.get("HOME")
        with mock.patch.object(Path, "home", side_effect=AssertionError("HOME lookup")):
            first, second = self.create_root(), self.create_root()
        self.assertNotEqual(first, second)
        for root in (first, second):
            self.assertEqual(root.parent, self.home)
            self.assertEqual(root.resolve(), root)
            self.assertEqual(root.stat().st_uid, os.geteuid())
            self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o700)
            HELPER.validate_directory(root, private=True)
            HELPER.validate_socket_budget(root)
        self.assertEqual(os.environ.get("HOME"), original_home)

    def test_unsafe_ancestor_refused_before_creating_any_root(self) -> None:
        child = self.directory / "c"
        child.mkdir(mode=0o700)
        self.directory.chmod(0o770)
        self.addCleanup(self.directory.chmod, 0o700)
        with mock.patch.object(HELPER, "service_home", return_value=child):
            with self.assertRaisesRegex(ValueError, "untrusted.*ancestor"):
                HELPER.create_fixture_root()
        self.assertEqual(list(child.iterdir()), [])

    def test_symlink_and_noncanonical_paths_are_refused(self) -> None:
        alias = self.directory / "alias"
        alias.symlink_to(self.home, target_is_directory=True)
        for path in (alias, self.directory / "..", Path("relative")):
            with self.subTest(path=path), self.assertRaises(ValueError):
                HELPER.validate_directory(path, private=False)

    def test_control_and_backslash_components_are_refused(self) -> None:
        for name in ("control\x85", "back\\slash"):
            path = self.directory / name
            path.mkdir(mode=0o700)
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "canonical"):
                HELPER.validate_directory(path, private=True)

    def test_git_worktree_markers_and_bare_repository_are_refused(self) -> None:
        for marker_kind in ("directory", "file", "broken-link"):
            with self.subTest(marker_kind=marker_kind):
                marker = self.directory / ".git"
                if marker_kind == "directory":
                    marker.mkdir()
                elif marker_kind == "file":
                    marker.write_text("gitdir: elsewhere\n")
                else:
                    marker.symlink_to(self.directory / "absent")
                with self.assertRaisesRegex(ValueError, "outside Git"):
                    HELPER.validate_directory(self.directory, private=True)
                if marker_kind == "directory":
                    marker.rmdir()
                else:
                    marker.unlink()
        subprocess.run(
            ["git", "init", "--bare", "--quiet", str(self.directory)],
            check=True,
            capture_output=True,
        )
        with self.assertRaisesRegex(ValueError, "outside Git"):
            HELPER.validate_directory(self.directory, private=True)

    def test_root_requires_exact_private_mode(self) -> None:
        self.directory.chmod(0o750)
        with self.assertRaisesRegex(ValueError, "mode 0700"):
            HELPER.validate_directory(self.directory, private=True)

    def test_oversized_root_refused_before_creation(self) -> None:
        long_home = self.directory / ("x" * 100)
        long_home.mkdir(mode=0o700)
        with mock.patch.object(HELPER, "service_home", return_value=long_home):
            with self.assertRaisesRegex(ValueError, "socket path too long"):
                HELPER.create_fixture_root()
        self.assertEqual(list(long_home.iterdir()), [])

    def test_socket_budget_counts_complete_utf8_endpoint_bytes(self) -> None:
        suffix = "/" + HELPER.BROKER_CHILD + "/" + HELPER.BROKER_SOCKET
        available = HELPER.SOCKET_MAX_BYTES - len(suffix.encode())
        exact = Path("/" + "x" * (available - 1))
        HELPER.validate_socket_budget(exact)
        for path in (Path(str(exact) + "x"), Path("/" + "é" * (available - 1))):
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "too long"):
                HELPER.validate_socket_budget(path)

    def test_existing_name_collision_never_reuses_or_modifies_directory(self) -> None:
        root = self.create_root()
        marker = root / "retain"
        marker.write_text("existing material")
        self.addCleanup(marker.unlink)
        token = root.name.removeprefix(HELPER.ROOT_PREFIX)
        with mock.patch.object(HELPER.secrets, "token_hex", return_value=token):
            with self.assertRaisesRegex(ValueError, "fresh private fixture root"):
                HELPER.create_fixture_root()
        self.assertEqual(marker.read_text(), "existing material")

    def test_exports_both_paths_and_preserves_existing_environment_lines(self) -> None:
        root = self.create_root()
        output = self.directory / "env"
        output.write_text("EXISTING=keep\n")
        HELPER.export_fixture_root(output, root)
        self.assertEqual(
            output.read_text(),
            f"EXISTING=keep\nTEST_NETWORK_TMP_DIR={root}\n"
            f"TAIRA_TESTNET_BEACON_FIXTURE_DIR={root}\n",
        )
        link = self.directory / "env-link"
        link.symlink_to(output)
        original = output.read_bytes()
        with self.assertRaises(OSError):
            HELPER.export_fixture_root(link, root)
        self.assertEqual(output.read_bytes(), original)

    def test_unsafe_root_is_never_exported(self) -> None:
        output = self.directory / "env"
        output.write_text("EXISTING=keep\n")
        self.directory.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "mode 0700"):
            HELPER.export_fixture_root(output, self.directory)
        self.assertEqual(output.read_text(), "EXISTING=keep\n")

    def test_actual_cli_exports_a_private_root_under_the_service_home(self) -> None:
        output = self.directory / "github-env"
        output.touch(mode=0o600)
        result = subprocess.run(
            [
                sys.executable, str(ROOT / "scripts/ci_private_fixture_root.py"),
                "--github-env", str(output),
            ],
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        environment = dict(line.split("=", 1) for line in output.read_text().splitlines())
        root = Path(environment["TEST_NETWORK_TMP_DIR"])
        self.addCleanup(root.rmdir)
        self.assertEqual(root.parent, self.home)
        self.assertEqual(environment["TAIRA_TESTNET_BEACON_FIXTURE_DIR"], str(root))
        HELPER.validate_directory(root, private=True)
        HELPER.validate_socket_budget(root)

    def test_action_invokes_selected_checkout_and_keeps_artifact_paths(self) -> None:
        action = (ROOT / ".github/actions/setup-native-ci/action.yml").read_text()
        step = action.split("    - name: Prepare private network fixture root\n", 1)[1]
        step = step.split("    - name:", 1)[0]
        self.assertIn("working-directory: ${{ inputs.working-directory }}", step)
        self.assertIn(
            'run: python scripts/ci_private_fixture_root.py --github-env "$GITHUB_ENV"',
            step,
        )
        workflow = (ROOT / ".github/workflows/pr.yml").read_text()
        self.assertNotIn("TEST_NETWORK_TMP_DIR:", workflow.split("jobs:", 1)[0])
        self.assertIn("${{ env.TEST_NETWORK_TMP_DIR }}/irohad_test_network_*", workflow)
        broker = (ROOT / "crates/iroha_test_network/src/disposable_runtime_provider_broker.rs").read_text()
        self.assertIn('PRIVATE_DIRECTORY_PREFIX: &str = ".iroha-b-";', broker)
        self.assertIn("PRIVATE_DIRECTORY_RANDOM_BYTES: usize = 6;", broker)
        self.assertIn(f'BROKER_SOCKET_BASENAME: &str = "{HELPER.BROKER_SOCKET}";', broker)
        config = (ROOT / "crates/iroha_config/src/parameters/actual.rs").read_text()
        self.assertIn(f"raw.len() > {HELPER.SOCKET_MAX_BYTES}", config)


if __name__ == "__main__":
    unittest.main()
