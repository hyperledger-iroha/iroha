"""Startup preflight uses actual immutable executable copies, never Cargo or hosts."""
import contextlib
import copy
import hashlib
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import test_taira_release_check as existing

gate = existing.gate


class StartupPreflightTests(unittest.TestCase):
    def run_fixture(self, *, failed=(), ignored=(), reuse=None):
        core_startup = (("owner startup", ("core_startup",)),)
        daemon_startup = (("daemon startup", ("daemon_startup",)),)
        groups = {
            "CONFIG_STAGES": (("config", ("config",)),),
            "STAGES": (("cli", ("cli",)),),
            "PROOF_STAGES": (("long proof", ("proof",)),),
            "CORE_STAGES": (("long consensus", ("core_long",)),) + core_startup,
            "DAEMON_STAGES": (("other daemon", ("daemon_long",)),) + daemon_startup,
        }
        selections = {"config": groups["CONFIG_STAGES"], "cli": groups["STAGES"],
                      "proof": groups["PROOF_STAGES"], "core": groups["CORE_STAGES"],
                      "daemon": groups["DAEMON_STAGES"], "network": gate.NETWORK_STAGES}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "copies"
            output.mkdir(mode=0o700)
            executed = root / "executed"
            paths, identities, observations = {}, {}, []
            for selection, stages in selections.items():
                names = [name for _, names in stages for name in names]
                path = output / selection
                path.write_text(f"#!{sys.executable}\nimport sys\nfrom pathlib import Path\n"
                    f"if '--list' in sys.argv: print({chr(10).join(name + ': test' for name in names)!r}); sys.exit(0)\n"
                    "name = sys.argv[1]\n"
                    f"with Path({str(executed)!r}).open('a') as stream: stream.write(name + '\\n')\n"
                    f"failed = name in {failed!r}\nignored = name in {ignored!r}\n"
                    "print('test ' + name + (' ... FAILED' if failed else ' ... ignored' if ignored else ' ... ok'))\n"
                    "print('fixture failure' if failed else 'test result: ok. 0 passed; 0 failed; 1 ignored;' if ignored else 'test result: ok. 1 passed; 0 failed; 0 ignored;')\n"
                    "sys.exit(101 if failed else 0)\n")
                path.chmod(0o500)
                info = path.stat()
                paths[selection] = str(path)
                identities[path] = tuple(getattr(info, field) for field in
                    ("st_dev", "st_ino", "st_size", "st_mode", "st_uid", "st_nlink", "st_mtime_ns", "st_ctime_ns"))
                observations.append({"selection": selection, "path": str(path),
                    "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "size": info.st_size,
                    "cargo_artifact": {"profile": {"test": True}, "target": {"name": selection}}})
            output.chmod(0o500)
            copies = gate.NativeArtifactCopies(output, paths, identities, observations)
            complete_stages = tuple((name, selections[name]) for name in ("cli", "proof", "core", "daemon"))
            prior = gate.independent_check_evidence(copies, complete_stages) if reuse else None
            if reuse == "changed":
                prior = copy.deepcopy(prior)
                prior["selected_tests"][2]["stages"][-1]["tests"] = ["old_startup"]
            updates = []
            error = None
            with contextlib.ExitStack() as stack:
                for group in ("CONFIG_STAGES", "STAGES", "PROOF_STAGES", "PROOF_FLOW_STAGES",
                              "CORE_STAGES", "DAEMON_STAGES", "CRYPTO_STAGES", "P2P_STAGES",
                              "TEST_NETWORK_STAGES", "CLIENT_STAGES", "TORII_UNIT_STAGES", "TORII_STAGES"):
                    stack.enter_context(patch.object(gate, group, groups.get(group, ())))
                stack.enter_context(patch.object(gate, "CORE_STARTUP_STAGES", core_startup))
                stack.enter_context(patch.object(gate, "DAEMON_STARTUP_STAGES", daemon_startup))
                stack.enter_context(patch.object(gate, "shipping_harnesses", return_value=()))
                for function in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "require_network_fixture_capacity"):
                    stack.enter_context(patch.object(gate, function))
                batch = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                network = stack.enter_context(patch.object(gate, "run_network_checks"))
                stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
                env = dict(os.environ, CARGO="/unused/cargo", CARGO_HOME="/isolated", CARGO_TARGET_DIR=str(root))
                try:
                    gate.run_checks(root, environment=env, source_commit="a" * 40,
                                    completed_independent_checks=prior, update_independent_checks=updates.append)
                except gate.CheckError as caught:
                    error = caught
            self.assertEqual(batch.call_args.kwargs["harnesses"], ("config", "proof", "core", "daemon", "network", "cli"))
            self.assertEqual(list(output.iterdir()), [], "actual immutable copies release on pass and preflight failure")
            self.assertIsNone(copies.directory_fd)
            return error, executed.read_text().splitlines(), updates, network.call_count

    def test_startup_groups_run_before_long_tests_exactly_once_and_preserve_full_checkpoint(self):
        error, executed, updates, network = self.run_fixture()
        self.assertIsNone(error)
        self.assertEqual(executed, ["config", "cli", "core_startup", "daemon_startup", "proof", "core_long", "daemon_long"])
        self.assertEqual(network, 1)
        self.assertEqual(updates[0], None)
        selected = [name for group in updates[1]["selected_tests"] for stage in group["stages"] for name in stage["tests"]]
        self.assertCountEqual(selected, executed[1:])
        self.assertEqual(len(selected), len(set(selected)))

    def test_both_startup_failures_are_collected_before_long_tests_and_no_checkpoint_is_published(self):
        error, executed, updates, network = self.run_fixture(failed=("core_startup", "daemon_startup"))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertEqual(len(error.failures), 2)
        self.assertEqual(executed, ["config", "cli", "core_startup", "daemon_startup"])
        self.assertEqual(updates, [None])
        self.assertEqual(network, 0)

    def test_ignored_startup_is_failure_and_still_collects_other_startup_group(self):
        error, executed, updates, network = self.run_fixture(ignored=("core_startup",))
        self.assertIsInstance(error, gate.SelectedRegressionFailures)
        self.assertIn("core_startup", str(error))
        self.assertEqual(executed, ["config", "cli", "core_startup", "daemon_startup"])
        self.assertEqual(updates, [None])
        self.assertEqual(network, 0)

    def test_only_exact_complete_checkpoint_skips_startup_and_configuration_always_runs(self):
        for reuse in ("exact", "changed"):
            with self.subTest(reuse=reuse):
                error, executed, updates, network = self.run_fixture(reuse=reuse)
                self.assertIsNone(error)
                self.assertEqual(network, 1)
                if reuse == "exact":
                    self.assertEqual(executed, ["config"])
                    self.assertEqual(updates, [])
                else:
                    self.assertEqual(executed, ["config", "cli", "core_startup", "daemon_startup", "proof", "core_long", "daemon_long"])
                    self.assertEqual(updates[0], None)
                    self.assertTrue(updates[1]["passed"])


if __name__ == "__main__":
    unittest.main()
