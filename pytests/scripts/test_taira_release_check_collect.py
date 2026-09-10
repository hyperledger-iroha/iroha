"""Offline cross-harness regression collection; no Cargo, hosts or runtime inputs."""
import contextlib
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import test_taira_release_check as existing

gate = existing.gate


class CollectIndependentRegressionTests(unittest.TestCase):
    groups = (
        ("crypto", "CRYPTO_STAGES"), ("p2p", "P2P_STAGES"),
        ("core", "CORE_STAGES"), ("test-network", "TEST_NETWORK_STAGES"),
        ("client", "CLIENT_STAGES"), ("torii-unit", "TORII_UNIT_STAGES"),
        ("torii", "TORII_STAGES"), ("daemon", "DAEMON_STAGES"), ("cli", "STAGES"),
    )

    def run_fixtures(self, *, failed=(), ignored=(), missing=None, custody_failure=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executed = root / "executed"
            artifacts = {}
            for name, _ in self.groups:
                tests = [name + "_first", name + "_second"]
                listed = tests[:1] if name == missing else tests
                script = root / name
                script.write_text(f"#!{sys.executable}\nimport sys\nfrom pathlib import Path\n"
                    f"if '--list' in sys.argv: print({chr(10).join(test + ': test' for test in listed)!r}); sys.exit(0)\n"
                    "name = sys.argv[1]\n"
                    f"with Path({str(executed)!r}).open('a') as f: f.write(name + '\\n')\n"
                    f"failed = {name in failed!r} and name.endswith('_first')\n"
                    f"ignored = {name in ignored!r} and name.endswith('_first')\n"
                    "print('test ' + name + (' ... FAILED' if failed else ' ... ignored' if ignored else ' ... ok'))\n"
                    "print('fixture failure' if failed else 'test result: ok. 0 passed; 0 failed; 1 ignored;' if ignored else 'test result: ok. 1 passed; 0 failed; 0 ignored;')\n"
                    "sys.exit(101 if failed else 0)\n")
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
                for name, group in self.groups:
                    stack.enter_context(patch.object(gate, group, ((name, (name + "_first", name + "_second")),)))
                for group in ("CONFIG_STAGES", "PROOF_STAGES", "PROOF_FLOW_STAGES"):
                    stack.enter_context(patch.object(gate, group, ()))
                for function in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "run_config_checks", "require_network_fixture_capacity"):
                    stack.enter_context(patch.object(gate, function))
                batch = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                other = stack.enter_context(patch.object(gate, "compile_harness"))
                network = stack.enter_context(patch.object(gate, "run_network_checks"))
                calls = stack.enter_context(patch.object(gate.subprocess, "run", wraps=gate.subprocess.run))
                stack.enter_context(contextlib.redirect_stdout(output))
                stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
                with self.assertRaises(gate.CheckError) as caught:
                    gate.run_checks(root, environment=env, source_commit="a" * 40, lock_fds=locks)
            batch.assert_called_once()
            self.assertEqual(batch.call_args.kwargs["harnesses"], tuple(name for name, _ in self.groups[:-1]) + ("network", "cli"))
            network.assert_not_called()
            other.assert_not_called()
            self.assertNotIn("[taira-check] PASS:", output.getvalue())
            self.assertTrue(all(call.kwargs["pass_fds"] == locks and call.kwargs["cwd"] == root for call in calls.call_args_list))
            return caught.exception, executed.read_text().splitlines() if executed.exists() else [], released

    def test_all_independent_libraries_daemon_and_cli_report_failures_before_any_network_build(self):
        error, executed, released = self.run_fixtures(failed=("core", "torii", "cli"))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertEqual(len(error.failures), 3)
        self.assertTrue(all(name + "_first" in str(error) for name in ("core", "torii", "cli")))
        self.assertEqual(executed, [name + suffix for name, _ in self.groups[-1:] + self.groups[:-1] for suffix in ("_first", "_second")])
        self.assertEqual(released, [name for name, _ in self.groups[-1:] + self.groups[:-1]])

    def test_cli_failure_does_not_hide_later_zero_exit_ignored_test(self):
        error, executed, released = self.run_fixtures(failed=("cli",), ignored=("daemon",))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertEqual(len(error.failures), 2)
        self.assertIn("daemon_first (exit 0)", str(error))
        self.assertIn("cli_first (exit 101)", str(error))
        self.assertEqual(len(executed), 18)
        self.assertEqual(len(released), 9)

    def test_missing_required_test_stops_immediately_without_becoming_a_regression(self):
        error, executed, released = self.run_fixtures(missing="core")
        self.assertNotIsInstance(error, gate.SelectedRegressionFailures)
        self.assertIn("required regressions missing", str(error))
        self.assertEqual(executed, [name + suffix for name in ("cli", "crypto", "p2p") for suffix in ("_first", "_second")])
        self.assertEqual(released, ["cli", "crypto", "p2p"])

    def test_artifact_release_failure_stops_immediately_even_after_collected_regression(self):
        error, executed, released = self.run_fixtures(failed=("core",), custody_failure="core")
        self.assertNotIsInstance(error, gate.SelectedRegressionFailures)
        self.assertIn("native test copy changed", str(error))
        self.assertEqual(executed, [name + suffix for name in ("cli", "crypto", "p2p", "core") for suffix in ("_first", "_second")])
        self.assertEqual(released, ["cli", "crypto", "p2p"])


if __name__ == "__main__":
    unittest.main()
