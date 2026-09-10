"""Shipping source coverage and native artifact controls; no Cargo or live inputs."""

import contextlib
import copy
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

import test_taira_release_check as existing

gate = existing.gate
ACTUAL_SHIPPING_AUDIT = gate.shipping_harnesses
ACTUAL_COMPILE_TESTS = gate.compile_test_harnesses
BINARIES = (("iroha3d_taira", "irohad"), ("iroha", "iroha_cli"),
            ("sorafs-node", "sorafs_node"), ("kagami", "iroha_kagami"))
SHIPPING = ("taira-launcher", "cli", "sorafs-bin", "kagami")


def shipping_source(root):
    (root / "scripts").mkdir(exist_ok=True)
    (root / "scripts/taira_release.py").write_text("BINARIES = " + repr(BINARIES) + "\n")
    for name, package in BINARIES:
        directory = root / "crates" / package
        (directory / "src").mkdir(parents=True, exist_ok=True)
        (directory / "src/main.rs").write_text("fn main() {}\n")
        (directory / "Cargo.toml").write_text(
            f'[package]\nname = "{package}"\n[features]\ndefault = ["shipping"]\nshipping = ["entry"]\nentry = []\n'
            f'[[bin]]\nname = "{name}"\npath = "src/main.rs"\nrequired-features = ["entry"]\n')


class ShippingSourceTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        shipping_source(self.root)

    def test_real_shipping_table_manifests_and_defaults_have_exact_native_coverage(self):
        self.assertEqual(ACTUAL_SHIPPING_AUDIT(existing.SCRIPT.parent.parent), SHIPPING)
        self.assertEqual(ACTUAL_SHIPPING_AUDIT(self.root), SHIPPING)
        self.assertEqual(gate.selected_regression_count(), existing.EXPECTED_REGRESSION_COUNT)

    def test_new_shipping_target_requires_explicit_early_coverage(self):
        table = self.root / "scripts/taira_release.py"
        table.write_text("BINARIES = " + repr(BINARIES + (("new-bin", "new_package"),)) + "\n")
        with self.assertRaisesRegex(gate.CheckError, "lacks exact early native coverage"):
            ACTUAL_SHIPPING_AUDIT(self.root)

    def test_missing_wrong_or_duplicate_native_target_is_rejected(self):
        for replacement in (None, ("wrong", "kagami", "bin", ["-p", "other", "--bin", "kagami"]),
                            ("wrong", "kagami", "lib", ["-p", "iroha_kagami", "--lib"])):
            targets = dict(gate.HARNESS_TARGETS)
            if replacement is None:
                del targets["kagami"]
            else:
                targets["kagami"] = replacement
            with self.subTest(replacement=replacement), patch.dict(gate.HARNESS_TARGETS, targets, clear=True):
                with self.assertRaisesRegex(gate.CheckError, "lacks exact early native coverage"):
                    ACTUAL_SHIPPING_AUDIT(self.root)
        with patch.dict(gate.HARNESS_TARGETS, {"duplicate": gate.HARNESS_TARGETS["kagami"]}):
            with self.assertRaisesRegex(gate.CheckError, "lacks exact early native coverage"):
                ACTUAL_SHIPPING_AUDIT(self.root)

    def test_manifest_package_binary_path_and_default_feature_drift_is_rejected(self):
        path = self.root / "crates/iroha_kagami/Cargo.toml"
        original = path.read_text()
        for changed in (original.replace('name = "iroha_kagami"', 'name = "other"'),
                        original.replace('name = "kagami"', 'name = "other"'),
                        original.replace('src/main.rs', 'src/absent.rs'),
                        original.replace('src/main.rs', '../outside.rs'),
                        original.replace('default = ["shipping"]', 'default = []')):
            with self.subTest(changed=changed):
                path.write_text(changed)
                with self.assertRaises(gate.CheckError):
                    ACTUAL_SHIPPING_AUDIT(self.root)
        path.write_text(original)
        self.assertEqual(ACTUAL_SHIPPING_AUDIT(self.root), SHIPPING)

    def test_nonliteral_duplicate_or_malformed_shipping_table_is_rejected_without_execution(self):
        path = self.root / "scripts/taira_release.py"
        for source in ("BINARIES = unavailable()\n", "BINARIES = ()\n",
                       "BINARIES = (('a', '../outside'),)\n",
                       "BINARIES = " + repr(BINARIES) + "\nBINARIES = ()\n"):
            with self.subTest(source=source):
                path.write_text(source)
                with self.assertRaises(gate.CheckError):
                    ACTUAL_SHIPPING_AUDIT(self.root)

    def test_invalid_exported_macro_path_is_checked_even_on_linux_only_source(self):
        path = self.root / "crates/iroha_kagami/src/linux.rs"
        path.write_text('#[cfg(target_os = "linux")] fn example() { norito :: json :: json!({}); }')
        with self.assertRaisesRegex(gate.CheckError, "invalid Norito JSON macro path"):
            ACTUAL_SHIPPING_AUDIT(self.root)
        path.write_text('#[cfg(target_os = "linux")] fn example() { norito::json!({}); }')
        self.assertEqual(ACTUAL_SHIPPING_AUDIT(self.root), SHIPPING)


class ShippingArtifactTests(unittest.TestCase):
    make_fixture_writable = existing.NativeArtifactIsolationTests.make_fixture_writable
    artifact = existing.NativeArtifactIsolationTests.artifact

    def setUp(self):
        existing.NativeArtifactIsolationTests.setUp(self)
        shipping_source(self.source)
        self.executed = self.directory / "executed"
        self.names = {"cli": "cli_fixture", "kagami": gate.KAGAMI_STAGES[0][1][0],
                      "network": "network_fixture"}
        self.events = {}
        for selection in (*SHIPPING, "network"):
            name = self.names.get(selection)
            payload = (f"#!{sys.executable}\nimport sys\nfrom pathlib import Path\n"
                       f"if '--list' in sys.argv: print({str(name) + ': test'!r}); sys.exit(0)\n"
                       f"assert sys.argv[1] == {name!r}, 'compile-only target must not execute'\n"
                       f"with Path({str(self.executed)!r}).open('a') as f: f.write({str(name) + chr(10)!r})\n"
                       "print('test ' + sys.argv[1] + ' ... ok')\n"
                       "print('test result: ok. 1 passed; 0 failed; 0 ignored;')\n").encode()
            _, _, event = self.artifact(selection, payload)
            self.events[selection] = event
        self.checkpoint = None
        self.observations = []
        self.missing = None
        self.wrong_manifest = False
        self.network_failure = False
        self.network_calls = 0
        stack = contextlib.ExitStack()
        self.addCleanup(stack.close)
        for name in ("CONFIG_STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES", "TEST_NETWORK_STAGES",
                     "CLIENT_STAGES", "TORII_UNIT_STAGES", "TORII_STAGES", "DAEMON_STAGES", "PROOF_STAGES", "PROOF_FLOW_STAGES"):
            stack.enter_context(patch.object(gate, name, ()))
        stack.enter_context(patch.object(gate, "STAGES", (("CLI", (self.names["cli"],)),)))
        stack.enter_context(patch.object(gate, "NETWORK_STAGES", (("network", (self.names["network"],)),)))
        stack.enter_context(patch.object(gate, "shipping_harnesses", wraps=ACTUAL_SHIPPING_AUDIT))
        for name in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "require_network_fixture_capacity"):
            stack.enter_context(patch.object(gate, name))
        stack.enter_context(patch.object(gate, "native_artifact_guard", side_effect=lambda *_: contextlib.nullcontext()))
        stack.enter_context(patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=1024**4)))
        self.compiler = stack.enter_context(patch.object(gate, "compile_test_harnesses", side_effect=self.build))
        stack.enter_context(patch.object(gate, "run_network_checks", side_effect=self.network))

    def build(self, root, env, *, harnesses, lock_fds):
        self.assertEqual(harnesses, ("kagami", "network", "cli", "taira-launcher", "sorafs-bin"))
        events = copy.deepcopy([self.events[name] for name in harnesses if name != self.missing])
        if self.wrong_manifest:
            events[0]["manifest_path"] = str(self.source / "crates/wrong/Cargo.toml")
        child = MagicMock()
        child.stdout = io.StringIO("\n".join(json.dumps(event) for event in events))
        child.wait.return_value = 0
        process = MagicMock()
        process.__enter__.return_value = child
        with patch.object(gate.subprocess, "Popen", return_value=process) as cargo:
            result = ACTUAL_COMPILE_TESTS(root, env, harnesses=harnesses, lock_fds=lock_fds)
        command = cargo.call_args.args[0]
        for name, package in BINARIES:
            self.assertIn(package, command)
            self.assertIn(name, command)
        self.assertNotIn("--features", command)
        self.assertEqual(command.count("--no-run"), 1)
        self.observations.append(result.observations)
        return result

    def network(self, root, fixture, env, lock_fds, *, harness):
        self.network_calls += 1
        for row in self.observations[-1]:
            self.assertEqual(Path(row["path"]).exists(), row["selection"] == "network")
        self.assertEqual({row["selection"] for row in self.checkpoint["artifacts"]}, {"cli", "kagami"})
        gate.run_stages(harness, fixture, env, gate.NETWORK_STAGES, lock_fds)
        if self.network_failure:
            raise gate.CheckError("fixture network failure")

    def run_gate(self):
        def update(value):
            self.checkpoint = copy.deepcopy(value)
        gate.run_checks(self.source, environment=self.env | {"CARGO_HOME": str(self.directory)},
                        source_commit="a" * 40, completed_independent_checks=self.checkpoint,
                        update_independent_checks=update)

    def test_shipping_compile_only_artifacts_are_required_released_and_never_counted_as_tests(self):
        self.run_gate()
        self.compiler.assert_called_once()
        self.assertEqual(self.executed.read_text().splitlines(), list(self.names.values()))
        self.assertTrue(all(not Path(row["path"]).exists() for row in self.observations[-1]))

    def test_missing_shipping_artifacts_or_wrong_manifest_stop_before_any_test(self):
        for missing, wrong in (("taira-launcher", False), ("sorafs-bin", False), ("kagami", False), (None, True)):
            with self.subTest(missing=missing, wrong=wrong):
                self.missing, self.wrong_manifest = missing, wrong
                with self.assertRaises(gate.CheckError):
                    self.run_gate()
                self.assertFalse(self.executed.exists())
                self.assertIsNone(self.checkpoint)
        self.assertEqual(self.network_calls, 0)

    def test_network_retry_recompiles_shipping_closure_and_reuses_only_exact_selected_pass(self):
        self.network_failure = True
        with self.assertRaisesRegex(gate.CheckError, "fixture network failure"):
            self.run_gate()
        self.network_failure = False
        self.run_gate()
        self.assertEqual(self.compiler.call_count, 2)
        self.assertEqual(self.executed.read_text().splitlines(), list(self.names.values()) + [self.names["network"]])
        self.missing = "sorafs-bin"
        with self.assertRaises(gate.CheckError):
            self.run_gate()
        self.assertEqual(self.network_calls, 2)


class NativeShippingGraphTests(unittest.TestCase):
    make_fixture_writable = existing.NativeArtifactIsolationTests.make_fixture_writable
    artifact = existing.NativeArtifactIsolationTests.artifact
    process = existing.NativeArtifactIsolationTests.process

    def setUp(self):
        existing.NativeArtifactIsolationTests.setUp(self)
        shipping_source(self.source)
        self.selections = ("iroha3d", "iroha", "taira-launcher", "sorafs-bin", "kagami")
        self.events = []
        for selection in self.selections:
            _, _, event = self.artifact(selection)
            event["profile"]["test"] = False
            self.events.append(event)
        shipping = patch.object(gate, "shipping_harnesses", wraps=ACTUAL_SHIPPING_AUDIT)
        shipping.start()
        self.addCleanup(shipping.stop)

    def test_one_normal_build_matches_shipping_packages_and_keeps_fixture_launcher(self):
        with patch.object(gate.subprocess, "Popen", return_value=self.process(self.events)) as cargo:
            copies = gate.compile_network_binaries(self.source, self.env, (77,))
        self.assertEqual(set(copies), set(self.selections))
        self.assertTrue(all(Path(path).exists() for path in copies.values()))
        self.assertTrue(all(row["cargo_artifact"]["profile"]["test"] is False
                            for row in copies.observations))
        command = cargo.call_args.args[0]
        self.assertEqual([command[i + 1] for i, value in enumerate(command) if value == "-p"],
                         ["irohad", "iroha_cli", "sorafs_node", "iroha_kagami"])
        self.assertEqual([command[i + 1] for i, value in enumerate(command) if value == "--bin"],
                         ["iroha3d", "iroha", "iroha3d_taira", "sorafs-node", "kagami"])
        self.assertNotIn("--features", command)
        self.assertEqual(command.count("build"), 1)
        self.assertEqual(cargo.call_count, 1)
        self.assertEqual(cargo.call_args.kwargs["pass_fds"], (77,))

    def test_missing_or_test_only_shipping_binary_prevents_native_network_build_success(self):
        for missing in range(len(self.events)):
            for test_only in (False, True):
                events = copy.deepcopy(self.events)
                if test_only:
                    events[missing]["profile"]["test"] = True
                else:
                    del events[missing]
                with self.subTest(missing=missing, test_only=test_only), \
                     patch.object(gate.subprocess, "Popen", return_value=self.process(events)):
                    with self.assertRaisesRegex(gate.CheckError, "every required executable artifact"):
                        gate.compile_network_binaries(self.source, self.env, (77,))
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])


if __name__ == "__main__":
    unittest.main()
