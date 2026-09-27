"""Pre-network retry evidence with disposable executables; no Cargo or live network."""

from pathlib import Path
import unittest
from unittest.mock import patch

import test_taira_release_independent_checkpoint as existing

release = existing.release
gate = existing.gate


class PreNetworkCheckpointTests(unittest.TestCase):
    def setUp(self):
        self.fixture = existing.IndependentCheckpointTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        # A short priority prefix and a deferred case expose the exact point at
        # which the first attempt ran out of space copying shipping binaries.
        for name, value in (
            ("STAGES", (("priority CLI", ("cli_first",)),
                        ("deferred CLI", ("cli_extra",)))),
            ("PRIORITY_CLI_TESTS", ("cli_first",)),
            ("CORE_STARTUP_STAGES", (("core", ("core_first",)),)),
        ):
            override = patch.object(gate, name, value)
            override.start()
            self.addCleanup(override.stop)
        self.fixture.network.side_effect = self.network
        self.fail_at_shipping_copy = True

    @property
    def prefix(self) -> Path:
        return self.fixture.fixture.out / "pre-network-checks.json"

    @property
    def complete(self) -> Path:
        return self.fixture.fixture.out / "independent-checks.json"

    def network(self, _root, fixture_root, env, lock_fds, *, harness, stages):
        self.assertTrue(self.prefix.is_file(), "the passed prefix must be durable before shipping codegen")
        self.assertFalse(self.complete.exists(), "a prefix cannot claim the deferred census")
        if self.fail_at_shipping_copy:
            raise gate.CheckError("fixture shipping artifact copy reserve")
        gate.run_stages(harness, fixture_root, env, stages, lock_fds)

    def fail_once(self):
        with self.assertRaisesRegex(release.PrepareError, "shipping artifact copy reserve"):
            self.fixture.prepare()
        self.assertEqual(self.fixture.ran(), ["core_first", "cli_first"])
        self.assertTrue(self.prefix.is_file())
        self.assertFalse(self.complete.exists())
        self.assertFalse((self.fixture.fixture.out / "checks.json").exists())
        self.fail_at_shipping_copy = False

    def test_capacity_retry_reuses_only_exact_pre_network_pass(self):
        self.fail_once()
        result, _, build = self.fixture.prepare()
        self.assertEqual(self.fixture.ran(),
                         ["core_first", "cli_first", "network_first", "cli_extra"])
        self.assertEqual(self.fixture.compile.call_count, 2)
        self.assertEqual(build.call_count, 1)
        self.assertEqual(result["attempt"], "attempts/000002")
        self.assertTrue(self.complete.is_file())
        prefix = release.read_record(self.prefix)
        self.assertEqual(prefix["request"]["commit"], self.fixture.fixture.args.expected_commit)
        self.assertEqual([row["selection"] for row in prefix["evidence"]["selected_tests"]],
                         ["cli", "core"])

    def test_changed_executable_retires_prefix_before_failed_rerun(self):
        self.fail_once()
        executable = Path(self.fixture.artifacts["cli"]["executable"])
        executable.write_bytes(executable.read_bytes() + b"# changed Cargo output\n")
        self.fixture.failures.write_text("cli_first\n")
        with self.assertRaisesRegex(release.PrepareError, "cli_first"):
            self.fixture.prepare()
        self.assertEqual(self.fixture.ran(),
                         ["core_first", "cli_first", "core_first", "cli_first"])
        self.assertFalse(self.prefix.exists())
        self.assertTrue((self.fixture.fixture.out / "attempts/000002/retired-pre-network-checks.json").is_file())
        self.fixture.network.assert_called_once()

    def test_changed_census_retires_prefix_and_reexecutes_new_priority(self):
        self.fail_once()
        with patch.object(gate, "PRIORITY_CLI_TESTS", ("cli_first", "cli_extra")):
            self.fixture.prepare()
        self.assertEqual(self.fixture.ran(),
                         ["core_first", "cli_first", "core_first", "cli_first",
                          "cli_extra", "network_first"])
        self.assertTrue((self.fixture.fixture.out / "attempts/000002/retired-pre-network-checks.json").is_file())

    def test_tampered_request_cannot_reuse_prefix(self):
        self.fail_once()
        record = release.read_record(self.prefix)
        record["request"]["commit"] = "c" * 40
        self.prefix.rename(self.fixture.fixture.out / "retained-invalid-prefix.json")
        release.write_record(self.prefix, record)
        with self.assertRaisesRegex(release.PrepareError, "pre-network native check checkpoint differs"):
            self.fixture.prepare()
        self.assertEqual(self.fixture.compile.call_count, 1)
        self.fixture.network.assert_called_once()


if __name__ == "__main__":
    unittest.main()
