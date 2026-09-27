"""Offline cross-harness regression collection; no Cargo, hosts or runtime inputs."""
import contextlib
import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import test_taira_release_check as existing
from taira_fake_libtest import executable

gate = existing.gate


class CollectIndependentRegressionTests(unittest.TestCase):
    def setUp(self):
        existing.isolate_shipping_fixture(self)

    groups = (
        ("crypto", "CRYPTO_STAGES"), ("p2p", "P2P_STAGES"),
        ("core", "CORE_STAGES"), ("test-network", "TEST_NETWORK_STAGES"),
        ("client", "CLIENT_STAGES"), ("torii-unit", "TORII_UNIT_STAGES"),
        ("torii", "TORII_STAGES"), ("torii-lifecycle", "TORII_LIFECYCLE_STAGES"), ("daemon", "DAEMON_STAGES"), ("cli", "STAGES"),
    )

    def run_fixtures(self, *, failed=(), ignored=(), missing=None, custody_failure=None,
                     priority_cli=("cli_second",)):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executed = root / "executed"
            artifacts = {}
            for name, _ in self.groups:
                tests = [name + "_first", name + "_second"]
                listed = tests[:1] if name == missing else tests
                script = root / name
                script.write_text(executable(listed, executed,
                    failed=(name + "_first",) if name in failed else (),
                    ignored=(name + "_first",) if name in ignored else ()))
                script.chmod(0o500)
                artifacts[name] = str(script)
            artifacts["network"] = str(root / "unused-network")
            copies = existing.FixtureCopies(artifacts)
            released = []

            def release(name):
                if name == custody_failure:
                    raise gate.CheckError("native test copy changed before release: " + name)
                released.append(name)

            copies.release = release
            output = io.StringIO()
            with contextlib.ExitStack() as stack:
                descriptor = stack.enter_context((root / "lock").open("w"))
                locks = (descriptor.fileno(),)
                env = dict(os.environ, CARGO="/unused/cargo", CARGO_HOME="/isolated", CARGO_TARGET_DIR=str(root))
                existing.isolate_stage_fixture(stack, keep=("NETWORK_STAGES",))
                for name, group in self.groups:
                    stack.enter_context(patch.object(gate, group, ((name, (name + "_first", name + "_second")),)))
                # One CLI case runs before the four-peer fixture; the other is
                # deferred with every library/daemon group after that fixture.
                stack.enter_context(patch.object(gate, "PRIORITY_CLI_TESTS", priority_cli))
                stack.enter_context(patch.object(gate, "PRIORITY_TORII_STAGE_LABELS", ()))
                for function in ("run_lifecycle_source_checks", "run_config_checks", "require_network_fixture_capacity", "check_test_harnesses"):
                    stack.enter_context(patch.object(gate, function))
                batch = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                other = stack.enter_context(patch.object(gate, "compile_harness"))
                network = stack.enter_context(patch.object(gate, "run_network_checks"))
                calls = stack.enter_context(patch.object(gate.subprocess, "run", wraps=gate.subprocess.run))
                stack.enter_context(contextlib.redirect_stdout(output))
                stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
                with self.assertRaises(gate.CheckError) as caught:
                    gate.run_checks(root, qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=locks)
            batch.assert_called_once()
            self.assertEqual(batch.call_args.kwargs["harnesses"], tuple(name for name, _ in self.groups[:-1]) + ("network", "cli"))
            other.assert_not_called()
            self.assertNotIn("[taira-check] PASS:", output.getvalue())
            self.assertTrue(all(call.kwargs["pass_fds"] == locks and call.kwargs["cwd"] == root for call in calls.call_args_list))
            return (caught.exception, executed.read_text().splitlines() if executed.exists() else [],
                    released, network.call_count)

    def deferred(self, names):
        return [name + suffix for name in names for suffix in ("_first", "_second")]

    def test_deferred_library_daemon_and_cli_failures_are_collected_after_the_network_fixture(self):
        error, executed, released, network = self.run_fixtures(failed=("core", "torii", "torii-lifecycle", "cli"))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertEqual(len(error.failures), 4)
        self.assertTrue(all(name + "_first" in str(error) for name in ("core", "torii", "torii-lifecycle", "cli")))
        libraries = [name for name, _ in self.groups[:-1]]
        self.assertEqual(executed, ["cli_second", "cli_first"] + self.deferred(libraries))
        self.assertEqual(network, 1)
        self.assertEqual(released, ["network", "cli"] + libraries)

    def test_priority_cli_failure_stops_before_the_network_fixture_and_deferred_groups(self):
        error, executed, released, network = self.run_fixtures(
            failed=("cli", "core"), priority_cli=("cli_first",))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertEqual(len(error.failures), 1)
        self.assertIn("cli_first (FAILED)", str(error))
        self.assertEqual(executed, ["cli_first"])
        self.assertEqual(network, 0)
        self.assertEqual(released, [])

    def test_cli_failure_does_not_hide_later_zero_exit_ignored_test(self):
        error, executed, released, network = self.run_fixtures(failed=("cli",), ignored=("daemon",))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertEqual(len(error.failures), 2)
        self.assertIn("daemon_first (exit 0)", str(error))
        self.assertIn("cli_first (FAILED)", str(error))
        self.assertEqual(len(executed), 20)
        self.assertEqual(network, 1)
        self.assertEqual(len(released), 11)

    def test_missing_required_test_stops_immediately_without_becoming_a_regression(self):
        error, executed, released, network = self.run_fixtures(missing="core")
        self.assertNotIsInstance(error, gate.SelectedRegressionFailures)
        self.assertIn("required regressions missing", str(error))
        self.assertEqual(executed, ["cli_second", "cli_first"] + self.deferred(("crypto", "p2p")))
        self.assertEqual(network, 1)
        self.assertEqual(released, ["network", "cli", "crypto", "p2p"])

    def test_artifact_release_failure_stops_immediately_even_after_collected_regression(self):
        error, executed, released, network = self.run_fixtures(failed=("core",), custody_failure="core")
        self.assertNotIsInstance(error, gate.SelectedRegressionFailures)
        self.assertIn("native test copy changed", str(error))
        self.assertEqual(executed, ["cli_second", "cli_first"] + self.deferred(("crypto", "p2p", "core")))
        self.assertEqual(network, 1)
        self.assertEqual(released, ["network", "cli", "crypto", "p2p"])


if __name__ == "__main__":
    unittest.main()
