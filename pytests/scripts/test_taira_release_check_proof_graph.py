"""Proof gates share native compilation and exact checkpoints; no Cargo or network."""

import contextlib
import copy
import io
from pathlib import Path
import sys
import unittest
from unittest.mock import MagicMock, patch

import test_taira_release_check as existing

gate = existing.gate


class ProofGraphTests(unittest.TestCase):
    make_fixture_writable = existing.NativeArtifactIsolationTests.make_fixture_writable
    artifact = existing.NativeArtifactIsolationTests.artifact

    def setUp(self):
        existing.NativeArtifactIsolationTests.setUp(self)
        self.executed = self.directory / "executed"
        self.failures = self.directory / "failures"
        self.failures.write_text("")
        self.rows = {}
        self.names = {
            "cli": ["cli_fixture"],
            "proof": [name for _, names in gate.PROOF_STAGES for name in names],
            "proof-flows": [name for _, names in gate.PROOF_FLOW_STAGES for name in names],
            "network": ["network_fixture"],
        }
        self.assertEqual([len(self.names[name]) for name in ("proof", "proof-flows")], [4, 2])
        for name, tests in self.names.items():
            payload = (
                f"#!{sys.executable}\nimport sys\nfrom pathlib import Path\n"
                f"if '--list' in sys.argv: print({chr(10).join(test + ': test' for test in tests)!r}); sys.exit(0)\n"
                "name = sys.argv[1]\n"
                f"with Path({str(self.executed)!r}).open('a') as f: f.write(name + '\\n')\n"
                f"failed = name in Path({str(self.failures)!r}).read_text().splitlines()\n"
                "print('test ' + name + (' ... FAILED' if failed else ' ... ok'))\n"
                "print('fixture failure' if failed else 'test result: ok. 1 passed; 0 failed; 0 ignored;')\n"
                "sys.exit(101 if failed else 0)\n"
            ).encode()
            self.rows[name] = self.artifact(name, payload)[1]
        self.checkpoint = None
        self.updates = []
        self.copies = []
        self.network_calls = 0
        self.network_failure = False
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        for group in ("CONFIG_STAGES", "CONFIG_UNIT_STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES",
                      "TEST_NETWORK_STAGES", "CLIENT_STAGES", "TORII_UNIT_STAGES",
                      "TORII_STAGES", "DAEMON_STAGES"):
            stack.enter_context(patch.object(gate, group, ()))
        stack.enter_context(patch.object(gate, "STAGES", (("CLI fixture", tuple(self.names["cli"])),)))
        stack.enter_context(patch.object(gate, "NETWORK_STAGES", (("network fixture", tuple(self.names["network"])),)))
        for function in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "run_config_checks",
                         "require_network_fixture_capacity"):
            stack.enter_context(patch.object(gate, function))
        stack.enter_context(patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=1024**4)))
        stack.enter_context(patch.object(gate, "native_artifact_guard", side_effect=lambda *_: contextlib.nullcontext()))
        self.compile = stack.enter_context(patch.object(gate, "compile_test_harnesses", side_effect=self.build))
        self.later_compile = stack.enter_context(patch.object(gate, "compile_harness", side_effect=AssertionError("no late proof compilation")))
        stack.enter_context(patch.object(gate, "run_network_checks", side_effect=self.network))
        stack.enter_context(contextlib.redirect_stderr(io.StringIO()))

    def build(self, root, env, *, harnesses, lock_fds):
        self.assertEqual(harnesses, ("proof", "proof-flows", "network", "cli"))
        copies = gate.isolate_native_artifacts(root, env, {name: self.rows[name] for name in harnesses})
        self.copies.append(copy.deepcopy(copies.observations))
        return copies

    def update(self, value):
        self.checkpoint = copy.deepcopy(value)
        self.updates.append(copy.deepcopy(value))

    def network(self, root, fixture_root, env, lock_fds, *, harness, stages):
        self.network_calls += 1
        self.assertIsNotNone(self.checkpoint, "all proof gates must pass before production/network work")
        for row in self.copies[-1]:
            self.assertEqual(Path(row["path"]).exists(), row["selection"] == "network")
        self.assertEqual(stages, gate.NETWORK_STAGES)
        gate.run_stages(harness, fixture_root, env, stages, lock_fds)
        if self.network_failure:
            raise gate.CheckError("fixture network failure")

    def run_gate(self):
        gate.run_checks(self.source, qualification_scope="full", environment=self.env | {"CARGO_HOME": str(self.directory)},
                        source_commit="a" * 40, completed_independent_checks=self.checkpoint,
                        update_independent_checks=self.update)
        self.later_compile.assert_not_called()

    def ran(self):
        return self.executed.read_text().splitlines() if self.executed.exists() else []

    def independent_names(self):
        return self.names["cli"] + self.names["proof"] + self.names["proof-flows"]

    def test_all_six_proof_regressions_run_before_production_and_bind_checkpoint(self):
        self.run_gate()
        self.assertEqual(self.ran(), self.independent_names() + self.names["network"])
        self.assertEqual(self.compile.call_count, 1)
        self.assertEqual([row["selection"] for row in self.checkpoint["selected_tests"]],
                         ["cli", "proof", "proof-flows"])
        self.assertEqual([row["selection"] for row in self.checkpoint["artifacts"]],
                         ["cli", "proof", "proof-flows"])
        self.assertTrue(all(not Path(row["path"]).exists() for row in self.copies[0]))

    def test_proof_failures_collect_both_harnesses_and_block_production(self):
        self.failures.write_text(self.names["proof"][0] + "\n" + self.names["proof-flows"][0] + "\n")
        with self.assertRaises(gate.SelectedRegressionFailures) as caught:
            self.run_gate()
        self.assertEqual(len(caught.exception.failures), 2)
        self.assertEqual(self.ran(), self.independent_names())
        self.assertEqual(self.network_calls, 0)
        self.assertEqual(self.updates, [None])
        self.later_compile.assert_not_called()

    def test_network_retry_reuses_proof_pass_across_fresh_copy_paths(self):
        self.network_failure = True
        with self.assertRaisesRegex(gate.CheckError, "network failure"):
            self.run_gate()
        saved = copy.deepcopy(self.checkpoint)
        self.network_failure = False
        self.run_gate()
        self.assertEqual(self.ran(), self.independent_names() + self.names["network"] * 2)
        self.assertEqual(self.checkpoint, saved)
        self.assertNotEqual(self.copies[0][0]["path"], self.copies[1][0]["path"])
        self.assertEqual(self.network_calls, 2)

    def test_changed_proof_artifact_retires_pass_before_failed_rerun(self):
        self.run_gate()
        path = Path(self.rows["proof"]["executable"])
        path.write_bytes(path.read_bytes() + b"# changed compiled fixture\n")
        self.failures.write_text(self.names["proof"][0] + "\n")
        with self.assertRaises(gate.SelectedRegressionFailures):
            self.run_gate()
        self.assertIsNone(self.checkpoint)
        self.assertEqual(self.network_calls, 1)
        self.assertEqual(self.ran(), self.independent_names() + self.names["network"] + self.independent_names())

    def test_incomplete_or_changed_proof_census_cannot_reuse_pass(self):
        self.run_gate()
        self.checkpoint["selected_tests"] = self.checkpoint["selected_tests"][:-1]
        self.run_gate()
        changed = (("updated proof ownership", tuple(self.names["proof"])),)
        with patch.object(gate, "PROOF_STAGES", changed):
            self.run_gate()
        self.assertEqual(self.ran(), (self.independent_names() + self.names["network"]) * 3)
        self.assertEqual(self.updates.count(None), 3)


if __name__ == "__main__":
    unittest.main()
