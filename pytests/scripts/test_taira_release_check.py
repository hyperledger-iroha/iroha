"""Offline checks for the early-gate runner; no Cargo or live inputs required."""

import contextlib
import fcntl
import hashlib
import stat
import struct
import importlib.util
import io
import json
import os
import sys
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import MagicMock, patch


EXPECTED_REGRESSION_COUNT = 481

SCRIPT = Path(__file__).with_name("taira_release_check.py")
if not SCRIPT.exists():
    SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release_check.py"
# Match direct script execution so deferred sibling imports work when this
# suite is run alone, without another test module modifying sys.path first.
sys.path.insert(0, str(SCRIPT.parent))
SPEC = importlib.util.spec_from_file_location("taira_release_check", SCRIPT)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


def isolate_shipping_fixture(case):
    """Keep unrelated orchestration fixtures focused on their selected harnesses."""
    audit = patch.object(gate, "shipping_harnesses", return_value=())
    audit.start()
    case.addCleanup(audit.stop)


class FixtureCopies(dict):
    """Only the mapping/context surface when a test mocks artifact compilation."""
    def __init__(self, value):
        super().__init__({name: value for name in gate.HARNESS_TARGETS} if isinstance(value, str) else value)
    def __enter__(self):
        return self
    def __exit__(self, *args):
        return False
    def release(self, selection):
        pass


class BasicReleaseQualificationTests(unittest.TestCase):
    def test_basic_census_keeps_security_and_application_checks_and_defers_advanced_core(self):
        basic, full = gate.qualification_stages(), gate.qualification_stages("full")
        self.assertEqual(gate.selected_regression_count(), 299)
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)
        self.assertEqual(set(basic), set(full))
        for name in basic:
            with self.subTest(selection=name):
                if name not in {"core", "proof-flows"}:
                    self.assertEqual(basic[name], full[name])
                names = [test for _, tests in basic[name] for test in tests]
                self.assertEqual(len(names), len(set(names)))
        self.assertEqual(basic["core"], gate.CORE_ADMISSION_STARTUP_STAGES)
        self.assertEqual(basic["proof-flows"], ())
        self.assertTrue(full["proof-flows"])
        self.assertEqual(basic["network"], gate.NETWORK_STAGES)
        for stage in gate.TORII_STARTUP_STAGES:
            self.assertIn(stage, basic["torii-unit"])
        for stage in gate.CORE_ADMISSION_STARTUP_STAGES:
            self.assertIn(stage, full["core"])

    def test_unknown_scope_fails_before_any_source_or_build_action(self):
        for scope in ("", "skip", "core_testnet", None):
            with self.subTest(scope=scope), patch.object(gate, "shipping_harnesses") as shipping, \
                 patch.object(gate, "run_pure_fsm_checks") as fsm:
                with self.assertRaisesRegex(gate.CheckError, "scope must be basic or full"):
                    gate.run_checks(Path("/unread"), qualification_scope=scope)
                shipping.assert_not_called()
                fsm.assert_not_called()

    @staticmethod
    def copies():
        copies = FixtureCopies({name: name for name in gate.HARNESS_TARGETS})
        copies.observations = [{"selection": name, "sha256": str(index) * 64, "size": 20,
                                "cargo_artifact": {"name": name}}
                               for index, name in enumerate(gate.HARNESS_TARGETS)]
        return copies

    def test_both_scopes_keep_identical_compile_graph_and_execute_exact_recorded_census(self):
        builds = []
        for scope in gate.QUALIFICATION_SCOPES:
            executed, output, checkpoint = [], io.StringIO(), MagicMock()
            copies = self.copies()
            with self.subTest(scope=scope), \
                 patch.object(gate, "shipping_harnesses", return_value=("cli", "kagami", "taira-launcher", "sorafs-bin")), \
                 patch.object(gate, "require_network_fixture_capacity"), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "compile_test_harnesses", return_value=copies) as compile, \
                 patch.object(copies, "release") as release, \
                 patch.object(gate, "run_stages", side_effect=lambda harness, root, env, stages, locks:
                              executed.extend((harness, test) for _, tests in stages for test in tests)), \
                 patch.object(gate, "run_network_checks", side_effect=lambda *args, **kwargs:
                              executed.extend(("network", test) for _, tests in gate.NETWORK_STAGES for test in tests)), \
                 contextlib.redirect_stdout(output):
                gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                source_commit="a" * 40, update_independent_checks=checkpoint)
            builds.append(compile.call_args.kwargs["harnesses"])
            selected = gate.qualification_stages(scope)
            expected = [(name, test) for name, stages in selected.items()
                        for _, tests in stages for test in tests]
            self.assertCountEqual(executed, expected)
            self.assertEqual(len(executed), gate.selected_regression_count(scope))
            self.assertEqual(checkpoint.call_args_list[0].args, (None,))
            evidence = checkpoint.call_args_list[1].args[0]
            self.assertEqual(evidence["qualification_scope"], scope)
            recorded = [(row["selection"], test) for row in evidence["selected_tests"]
                        for stage in row["stages"] for test in stage["tests"]]
            self.assertCountEqual(recorded, [row for row in executed if row[0] not in {"config", "network"}])
            self.assertIn(f"PASS: {len(expected)} {scope} regressions", output.getvalue())
            if scope == "basic":
                self.assertIn("proof-flows", builds[-1])
                self.assertNotIn("proof-flows", {row["selection"] for row in evidence["artifacts"]})
                self.assertIn(unittest.mock.call("proof-flows"), release.call_args_list)
        self.assertEqual(builds[0], builds[1])

    def test_independent_evidence_cannot_cross_scope_even_with_identical_artifacts_and_cases(self):
        stages = (("cli", gate.STAGES),)
        basic = gate.independent_check_evidence(self.copies(), stages, qualification_scope="basic")
        full = gate.independent_check_evidence(self.copies(), stages, qualification_scope="full")
        self.assertNotEqual(basic, full)
        self.assertEqual(basic | {"qualification_scope": "full"}, full)

    def test_basic_startup_failure_prevents_network_and_success_checkpoint(self):
        def fail_startup(harness, root, env, stages, locks):
            if stages == gate.CORE_ADMISSION_STARTUP_STAGES:
                raise gate.SelectedRegressionFailures(["empty Queue startup admission failed"])
        checkpoint, output = MagicMock(), io.StringIO()
        with patch.object(gate, "shipping_harnesses", return_value=("kagami",)), \
             patch.object(gate, "require_network_fixture_capacity"), \
             patch.object(gate, "run_pure_fsm_checks"), \
             patch.object(gate, "run_lifecycle_source_checks"), \
             patch.object(gate, "compile_test_harnesses", return_value=self.copies()), \
             patch.object(gate, "run_stages", side_effect=fail_startup), \
             patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(output):
            with self.assertRaisesRegex(gate.SelectedRegressionFailures, "empty Queue startup admission failed"):
                gate.run_checks(Path("/frozen"), environment={"CARGO": "/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"},
                                source_commit="a" * 40, update_independent_checks=checkpoint)
        checkpoint.assert_called_once_with(None)
        network.assert_not_called()
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_standalone_cli_selects_basic_by_default_and_forwards_explicit_full(self):
        import taira_release as release
        for arguments, scope in (([], "basic"), (["--native-check-scope", "full"], "full")):
            with self.subTest(scope=scope), patch.object(sys, "argv", [str(SCRIPT), *arguments]), \
                 patch.object(release, "development_check") as check:
                self.assertEqual(gate.main(), 0)
                self.assertEqual(check.call_args.kwargs, {"native_check_scope": scope})


class EarlyReleaseCheckTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        mock = patch.object(gate, "run_pure_fsm_checks")
        self.pure_fsm = mock.start()
        self.addCleanup(mock.stop)
        source = patch.object(gate, "run_lifecycle_source_checks")
        self.lifecycle_source = source.start()
        self.addCleanup(source.stop)
        config = patch.object(gate, "run_config_checks")
        self.config = config.start()
        self.addCleanup(config.stop)
        capacity = patch.object(gate, "require_network_fixture_capacity")
        self.capacity = capacity.start()
        self.addCleanup(capacity.stop)

    def test_failed_regression_does_not_hide_later_independent_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            harness = root / "harness"
            harness.write_text(f"#!{sys.executable}\n" + "import sys\nfrom pathlib import Path\n"
                "if '--list' in sys.argv:\n print('first: test\\nsecond: test\\nthird: test');sys.exit(0)\n"
                "name=sys.argv[1]\n"
                "with Path('executed').open('a') as f:f.write(name+'\\n')\n"
                "print('test '+name+(' ... ok' if name=='second' else ' ... FAILED'))\n"
                "print('test result: ok. 1 passed; 0 failed; 0 ignored;' if name=='second' else 'fixture failure')\n"
                "sys.exit(0 if name=='second' else 101)\n")
            harness.chmod(0o700)
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, '2 selected regressions failed:.*first.*third'):
                    gate.run_stages(str(harness), root, dict(os.environ), (("independent fixtures", ("first", "second", "third")),), ())
            self.assertEqual((root / "executed").read_text().splitlines(), ['first', 'second', 'third'])

    def test_command_reuses_native_cargo_lane_and_does_not_run_full_suite(self):
        self.assertEqual(gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}), [
            "/fixed/cargo", "--config", "/repo/.cargo/config.toml", "test",
            "--manifest-path", "/repo/Cargo.toml", "--locked", "--offline", "-p", "iroha_cli",
            "--bin", "iroha", "--no-run", "--message-format=json-render-diagnostics",
        ])

    def test_artifact_accepts_only_executable_iroha_binary_test_harness(self):
        event = {"reason": "compiler-artifact", "target": {"name": "iroha", "kind": ["bin"]},
                 "profile": {"test": True}, "executable": "/warm/debug/deps/iroha-test"}
        self.assertEqual(gate.test_artifact(json.dumps(event)), event["executable"])
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "iroha", "kind": ["lib"]}},
                        {"target": {"name": "kagami", "kind": ["bin"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes)))
        self.assertIsNone(gate.test_artifact("[cargo-fast] normal progress"))

    def test_torii_selection_requires_shipping_external_contract_harness(self):
        command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, harness="torii")
        self.assertIn("iroha_torii", command)
        self.assertIn("taira_app_contracts", command)
        self.assertNotIn("--lib", command)
        self.assertNotIn("--no-default-features", command)
        event = {"reason": "compiler-artifact", "target": {"name": "taira_app_contracts", "kind": ["test"]},
                 "profile": {"test": True}, "executable": "/warm/taira-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="torii"), event["executable"])
        self.assertIsNone(gate.test_artifact(json.dumps(event)))
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "torii_core_routes", "kind": ["test"]}},
                        {"target": {"name": "taira_app_contracts", "kind": ["lib"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness="torii"))

    def test_core_selection_requires_the_actual_library_test_artifact(self):
        command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, harness="core")
        self.assertEqual(command[8:11], ["-p", "iroha_core", "--lib"])
        event = {"reason": "compiler-artifact", "target": {"name": "iroha_core", "kind": ["lib"]},
                 "profile": {"test": True}, "executable": "/warm/core-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="core"), event["executable"])
        self.assertIsNone(gate.test_artifact(json.dumps(event)))
        self.assertIsNone(gate.test_artifact(json.dumps(event | {"profile": {"test": False}}), harness="core"))
        for selection in ({"harness": "unreviewed"}, {"harness": ""}):
            with self.assertRaises(gate.CheckError):
                gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, **selection)

    def test_transport_selections_require_their_actual_library_test_artifacts(self):
        for harness, package in (("crypto", "iroha_crypto"), ("p2p", "iroha_p2p"),
                                 ("test-network", "iroha_test_network")):
            with self.subTest(harness=harness):
                command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, harness=harness)
                self.assertEqual(command[8:11], ["-p", package, "--lib"])
                self.assertNotIn("--no-default-features", command)
                event = {"reason": "compiler-artifact", "target": {"name": package, "kind": ["lib"]},
                         "profile": {"test": True}, "executable": "/warm/" + package}
                self.assertEqual(gate.test_artifact(json.dumps(event), harness=harness), event["executable"])
                for changes in ({"profile": {"test": False}}, {"executable": None},
                                {"target": {"name": package, "kind": ["bin"]}},
                                {"target": {"name": "unrelated", "kind": ["lib"]}}):
                    self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness=harness))

    def test_failed_build_preserves_rendered_compiler_error(self):
        diagnostic = "error[E0308]: synthetic fixture type mismatch\n"
        event = {"reason": "compiler-message", "message": {"rendered": diagnostic}}
        child = MagicMock()
        child.stdout = io.StringIO("[cargo-fast] warm lane\n" + json.dumps(event) + "\n")
        child.wait.return_value = 101
        process = MagicMock()
        process.__enter__.return_value = child
        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=process), contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            with self.assertRaisesRegex(gate.CheckError, "native CLI build failed"):
                gate.compile_harness(Path("/fixture-only"), {"CARGO": "/fixed/cargo"})
        self.assertEqual(stderr.getvalue(), diagnostic)
        self.assertIn("[cargo-fast] warm lane", stdout.getvalue())
        self.assertNotIn("compiler-message", stdout.getvalue())

    def test_missing_or_renamed_regression_is_fatal(self):
        names = [name for _, tests in gate.STAGES for name in tests]
        gate.require_tests("\n".join(f"{name}: test" for name in names))
        for listing in ("", "\n".join(f"{name}: test" for name in names[:-1])):
            with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                gate.require_tests(listing)

    def test_selection_has_no_duplicate_test_names(self):
        names = [name for _, tests in gate.STAGES for name in tests]
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(gate.STAGES[0], ("core canary command composition", (
            "taira_public_reset::host::tests::coordinator_write_canary_argv_passes_child_validation_for_all_core_actions",
            "taira::tests::final_canary_predecessor_requires_its_independent_faucet_policy",
            "taira::tests::write_canary_policy_inputs_are_operation_and_action_scoped",
        )))
        self.assertIn(
            "taira_public_reset::host::tests::candidate_operator_status_child_binds_both_inherited_signers",
            names,
        )
        self.assertIn(
            "taira_public_reset::config::tests::operator_keygen_publishes_canonical_private_key_and_only_public_report",
            names,
        )

    def test_transport_selectors_are_mandatory_unique_and_fail_when_missing(self):
        for stages in (gate.CONFIG_STAGES, gate.CRYPTO_STAGES, gate.P2P_STAGES, gate.TEST_NETWORK_STAGES):
            names = [name for _, tests in stages for name in tests]
            self.assertTrue(names)
            self.assertEqual(len(names), len(set(names)))
            gate.require_tests("\n".join(f"{name}: test" for name in names), stages)
            with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                gate.require_tests("\n".join(f"{name}: test" for name in names[1:]), stages)

    def test_complete_regression_census_tracks_every_native_stage_group(self):
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)
        for group in ("STAGES", "CONFIG_STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES",
                      "TEST_NETWORK_STAGES", "NETWORK_STAGES", "PROOF_STAGES",
                      "PROOF_FLOW_STAGES", "TORII_STAGES", "CLIENT_STAGES", "TORII_UNIT_STAGES", "DAEMON_STAGES", "KAGAMI_STAGES"):
            original_count = sum(len(names) for _, names in getattr(gate, group))
            with self.subTest(group=group), patch.object(gate, group, (("fixture", ("one", "two")),)):
                self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT - original_count + 2)

    def test_exact_one_test_passes(self):
        result = subprocess.CompletedProcess([], 0,
            "test example ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 99 filtered out\n", "")
        gate.require_one_pass("example", result)

    def test_zero_ignored_wrong_or_failed_test_is_fatal(self):
        outputs = (
            (0, "test result: ok. 0 passed; 0 failed; 0 ignored; 100 filtered out\n"),
            (0, "test example ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored;\n"),
            (0, "test another ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n"),
            (101, "test example ... FAILED\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n"),
        )
        for code, output in outputs:
            with self.subTest(code=code, output=output), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "did not execute and pass"):
                    gate.require_one_pass("example", subprocess.CompletedProcess([], code, output, ""))


    def test_frozen_harness_uses_captured_manifest_config_and_no_git_lookup(self):
        child = MagicMock()
        child.stdout = io.StringIO(json.dumps({"reason": "compiler-artifact", "target": {"name": "iroha", "kind": ["bin"]}, "profile": {"test": True}, "executable": "/warm/iroha-test"}) + "\n")
        child.wait.return_value = 0
        process = MagicMock()
        process.__enter__.return_value = child
        with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), contextlib.redirect_stdout(io.StringIO()):
            gate.compile_harness(Path("/frozen"), {"CARGO": "/fixed/cargo"}, lock_fds=(77, 88))
        self.assertEqual(spawn.call_args.args[0][:4], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test"])
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))
        with patch.object(gate.subprocess, "check_output", side_effect=AssertionError("must not inspect mutable Git")), \
             patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies("/fixture/harness")) as compile, \
             patch.object(gate.subprocess, "run", side_effect=[subprocess.CompletedProcess([], 0, "fixture: test\n", ""), subprocess.CompletedProcess([], 0, "test fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "")]) as run, \
             patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "CORE_STAGES", ()), \
             patch.object(gate, "DAEMON_STAGES", ()), \
             patch.object(gate, "CLIENT_STAGES", ()), \
             patch.object(gate, "TORII_UNIT_STAGES", ()), \
             patch.object(gate, "TEST_NETWORK_STAGES", ()), \
             patch.object(gate, "NETWORK_STAGES", ()), \
             patch.object(gate, "PROOF_STAGES", ()), \
             patch.object(gate, "PROOF_FLOW_STAGES", ()), \
             patch.object(gate, "TORII_STAGES", ()), contextlib.redirect_stdout(io.StringIO()):
            gate.run_checks(Path("/frozen"), qualification_scope="full", environment={"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}, source_commit="a" * 40, lock_fds=(77, 88))
        self.assertEqual([call.kwargs["cwd"] for call in run.call_args_list], [Path("/warm"), Path("/warm")])
        self.assertNotIn("frozen", compile.call_args.kwargs)
        self.assertEqual(compile.call_args.args[1]["VERGEN_GIT_SHA"], "a" * 40)


    def test_mutable_check_keeps_git_checks_and_inherits_lane_lock_in_every_child(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/routine", "CARGO_INCREMENTAL": "0"}
        results = [subprocess.CompletedProcess([], 0, "fixture: test\n", ""),
                   subprocess.CompletedProcess([], 0, "test fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "")]
        with patch.object(gate.subprocess, "check_output", return_value="a" * 40) as git, \
             patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies("/fixture/harness")) as compile, \
             patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "CORE_STAGES", ()), \
             patch.object(gate, "DAEMON_STAGES", ()), \
             patch.object(gate, "CLIENT_STAGES", ()), \
             patch.object(gate, "TORII_UNIT_STAGES", ()), \
             patch.object(gate, "TEST_NETWORK_STAGES", ()), \
             patch.object(gate, "NETWORK_STAGES", ()), \
             patch.object(gate, "PROOF_STAGES", ()), \
             patch.object(gate, "PROOF_FLOW_STAGES", ()), \
             patch.object(gate, "TORII_STAGES", ()), contextlib.redirect_stdout(io.StringIO()):
            gate.run_checks(Path("/mutable"), qualification_scope="full", environment=env, lock_fds=(77,))
        self.assertEqual(git.call_count, 2)
        self.assertTrue(all(call.kwargs["env"]["CARGO_HOME"] == "/isolated" for call in git.call_args_list))
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("config", "cli")})
        self.assertTrue(all(call.kwargs["pass_fds"] == (77,) for call in run.call_args_list))
        self.assertTrue(all(call.kwargs["cwd"] == Path("/mutable") for call in run.call_args_list))
        self.assertEqual(env, {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/routine", "CARGO_INCREMENTAL": "0"})
        self.assertEqual(compile.call_args.args[1]["CARGO_INCREMENTAL"], "0")
        self.assertTrue(all(call.kwargs["env"]["CARGO_INCREMENTAL"] == "0" for call in run.call_args_list))

    def test_torii_contract_failure_prevents_overall_pass_and_keeps_same_custody(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        results = [subprocess.CompletedProcess([], 0, "cli: test\n", ""),
                   subprocess.CompletedProcess([], 0, "test cli ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", ""),
                   subprocess.CompletedProcess([], 0, "route: test\n", ""),
                   subprocess.CompletedProcess([], 101, "test route ... FAILED\n", "")]
        output = io.StringIO()
        with patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({"torii": "/warm/routes", "cli": "/warm/cli"})) as compile, \
             patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "STAGES", (("CLI", ("cli",)),)), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "CORE_STAGES", ()), \
             patch.object(gate, "DAEMON_STAGES", ()), \
             patch.object(gate, "CLIENT_STAGES", ()), \
             patch.object(gate, "TORII_UNIT_STAGES", ()), \
             patch.object(gate, "TEST_NETWORK_STAGES", ()), \
             patch.object(gate, "NETWORK_STAGES", ()), \
             patch.object(gate, "PROOF_STAGES", ()), \
             patch.object(gate, "PROOF_FLOW_STAGES", ()), \
             patch.object(gate, "TORII_STAGES", (("Torii", ("route",)),)), \
             contextlib.redirect_stdout(output), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "route.*exit 101"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 1)
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("config", "torii", "cli")})
        self.assertTrue(all(call.kwargs["cwd"] == Path("/warm") and call.kwargs["pass_fds"] == (77,)
                            for call in run.call_args_list))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_cli_contract_infrastructure_failure_stops_before_core_or_release_success(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        output = io.StringIO()
        with patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({
                "cli": "/warm/cli", "core": "/warm/core", "test-network": "/warm/fixture"})) as compile, \
             patch.object(gate, "run_network_checks") as network, \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "run_stages", side_effect=gate.CheckError("public transaction stalled")) as run, \
             contextlib.redirect_stdout(output):
            with self.assertRaisesRegex(gate.CheckError, "public transaction stalled"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 1)
        network.assert_not_called()
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("config", "proof", "proof-flows", "core", "test-network", "client", "torii-unit", "torii", "daemon", "network", "cli")})
        self.assertEqual(run.call_args.args[3], gate.STAGES)
        self.assertEqual(run.call_args.args[4], (77,))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_network_failure_does_not_trigger_separate_harness_builds(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("config", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "torii-unit", "torii", "daemon", "network", "cli")
        with patch.object(gate, "run_network_checks", side_effect=gate.CheckError("consensus stalled")) as network, \
             patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({
                 name: "/warm/" + name for name in names})) as batch, \
             patch.object(gate, "compile_harness", return_value=FixtureCopies("/warm/torii")) as compile, \
             patch.object(gate, "run_stages") as stages, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "consensus stalled"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        network.assert_called_once_with(Path("/frozen"), Path("/warm"),
            env | {"VERGEN_GIT_SHA": "a" * 40, "IROHA_GIT_COMMIT_HASH": "a" * 40}, (77,), harness="/warm/network")
        self.assertEqual(batch.call_count, 1)
        self.assertEqual(batch.call_args.kwargs, {"lock_fds": (77,), "harnesses": names})
        compile.assert_not_called()
        self.assertEqual([call.args[0] for call in stages.call_args_list], ["/warm/" + name for name in ("cli", "core", "torii-unit", "daemon") + names[1:-2]])
        self.assertEqual([call.args[3] for call in stages.call_args_list],
                         [gate.STAGES, gate.CORE_STARTUP_STAGES, gate.TORII_STARTUP_STAGES, gate.DAEMON_STARTUP_STAGES, gate.PROOF_STAGES, gate.PROOF_FLOW_STAGES, gate.CRYPTO_STAGES, gate.P2P_STAGES, tuple(stage for stage in gate.CORE_STAGES if stage not in gate.CORE_STARTUP_STAGES), gate.TEST_NETWORK_STAGES, gate.CLIENT_STAGES, tuple(stage for stage in gate.TORII_UNIT_STAGES if stage not in gate.TORII_STARTUP_STAGES), gate.TORII_STAGES, tuple(stage for stage in gate.DAEMON_STAGES if stage not in gate.DAEMON_STARTUP_STAGES)])

    def test_transport_or_fixture_failure_stops_before_network_and_release_success(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("config", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "torii-unit", "torii", "daemon", "network", "cli")
        for failed, expected in (("crypto", ["cli", "core", "torii-unit", "daemon", "proof", "proof-flows", "crypto"]),
                                 ("p2p", ["cli", "core", "torii-unit", "daemon", "proof", "proof-flows", "crypto", "p2p"]),
                                 ("fixture", ["cli", "core", "torii-unit", "daemon", "proof", "proof-flows", "crypto", "p2p", "core", "test-network"])):
            outcomes = [None] * (len(expected) - 1) + [gate.CheckError(failed + " failed")]
            output = io.StringIO()
            with self.subTest(failed=failed), \
                 patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({
                     name: "/warm/" + name for name in names})) as compile, \
                 patch.object(gate, "run_stages", side_effect=outcomes) as run, \
                 patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(output):
                with self.assertRaisesRegex(gate.CheckError, failed + " failed"):
                    gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
            self.assertEqual(compile.call_count, 1)
            self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": names})
            self.assertEqual(compile.call_args.args[0], Path("/frozen"))
            self.assertEqual(compile.call_args.args[1]["CARGO_TARGET_DIR"], "/warm")
            self.assertEqual([call.args[0] for call in run.call_args_list], ["/warm/" + name for name in expected])
            network.assert_not_called()
            for call in run.call_args_list:
                self.assertEqual(call.args[1], Path("/warm"))
                self.assertEqual(call.args[4], (77,))
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_public_contract_library_failures_stop_before_http_and_node_builds(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("config", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "torii-unit", "torii", "daemon", "network", "cli")
        for failed in ("client", "torii-unit", "torii"):
            def run(harness, *args):
                if harness == "/warm/" + failed:
                    raise gate.CheckError(failed + " failed")
            with self.subTest(failed=failed), \
                 patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies({name: "/warm/" + name for name in names})), \
                 patch.object(gate, "run_stages", side_effect=run), \
                 patch.object(gate, "compile_harness") as other, \
                 patch.object(gate, "run_network_checks") as network, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, failed + " failed"):
                    gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40)
            other.assert_not_called()
            network.assert_not_called()

    def test_unisolated_low_level_check_is_rejected_before_git_or_cargo(self):
        with patch.object(gate.subprocess, "check_output") as git, patch.object(gate, "compile_harness") as compile:
            with self.assertRaisesRegex(gate.CheckError, "isolated Cargo environment"):
                gate.run_checks(Path("/mutable"), qualification_scope="full", environment={})
        git.assert_not_called()
        compile.assert_not_called()

    def test_network_build_accepts_only_both_real_binary_artifacts(self):
        events = [{"reason": "compiler-artifact", "target": {"name": name, "kind": ["bin"]},
                   "profile": {"test": False}, "executable": "/warm/" + name}
                  for name in ("iroha3d", "iroha")]
        for selected, accepted in ((events, True), (events[:1], False),
                                   ([event | {"profile": {"test": True}} for event in events], False)):
            child = MagicMock()
            child.stdout = io.StringIO("\n".join(json.dumps(event) for event in selected))
            child.wait.return_value = 0
            process = MagicMock()
            process.__enter__.return_value = child
            with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), contextlib.redirect_stdout(io.StringIO()):
                if accepted:
                    result = gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"}, (77,))
                    self.assertEqual(result, {"iroha3d": "/warm/iroha3d", "iroha": "/warm/iroha"})
                else:
                    with self.assertRaisesRegex(gate.CheckError, "every required executable artifact"):
                        gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"}, (77,))
            self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
            self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77,))
            self.assertNotIn("iroha3d_taira", spawn.call_args.args[0])

    def test_network_gate_forbids_fallback_builds_and_sandbox_skips(self):
        env = {"CARGO_TARGET_DIR": "/warm"}
        with patch.object(gate, "compile_network_binaries", return_value={"iroha3d": "/warm/node", "iroha": "/warm/client"}), \
             patch.object(gate, "compile_harness", return_value=FixtureCopies("/warm/network")), \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/private-fixture") as fixture, \
             patch.object(gate, "run_stages") as run, contextlib.redirect_stdout(io.StringIO()):
            gate.run_network_checks(Path("/frozen"), Path("/warm"), env, (77, 88), harness="/warm/network")
        selected = run.call_args.args[2]
        for key in ("IROHA_TEST_SKIP_BUILD", "IROHA_FAIL_ON_SANDBOX_SKIP", "IROHA_TEST_REQUIRE_NETWORK", "IROHA_TEST_SERIALIZE_NETWORKS", "IROHA_TEST_NETWORK_KEEP_DIRS"):
            self.assertEqual(selected[key], "1")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHAD"], "/warm/node")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHA"], "/warm/client")
        self.assertEqual(selected["TEST_NETWORK_TMP_DIR"], "/warm/private-fixture")
        self.assertEqual(run.call_args.args[3:], (gate.NETWORK_STAGES, (77, 88)))
        self.assertEqual(fixture.call_args.kwargs["dir"], Path("/warm"))


class EarlyConfigurationGateTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)

    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated",
           "CARGO_TARGET_DIR": "/warm", "CARGO_INCREMENTAL": "1"}

    def test_configuration_target_preserves_defaults_and_requires_its_exact_artifact(self):
        self.assertEqual(gate.compile_command(Path("/frozen"), self.env, harness="config"), [
            "/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test",
            "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "iroha_config", "--test", "taira_config_contracts", "--no-run",
            "--message-format=json-render-diagnostics",
        ])
        event = {"reason": "compiler-artifact", "target": {
            "name": "taira_config_contracts", "kind": ["test"]},
            "profile": {"test": True}, "executable": "/warm/config-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), harness="config"), event["executable"])
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "iroha_config_integration", "kind": ["test"]}},
                        {"target": {"name": "taira_config_contracts", "kind": ["lib"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes), harness="config"))

    def test_configuration_stage_uses_same_captured_root_environment_and_locks(self):
        copies = FixtureCopies({"config": "/warm/config-contracts"})
        with patch.object(gate, "compile_harness") as compile, \
             patch.object(copies, "release") as release, \
             patch.object(gate, "run_stages") as run:
            gate.run_config_checks(copies, Path("/warm"), self.env, (77, 88))
        compile.assert_not_called()
        run.assert_called_once_with("/warm/config-contracts", Path("/warm"), self.env,
                                    gate.CONFIG_STAGES, (77, 88))
        release.assert_called_once_with("config")
        self.assertIs(run.call_args.args[2], self.env)

    def test_one_batch_runs_configuration_first_after_source_audits(self):
        events = []
        libraries = ("config", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "torii-unit", "torii", "daemon", "network", "cli")

        def run_stage(harness, root, env, stages, lock_fds):
            self.assertEqual((root, lock_fds), (Path("/warm"), (77,)))
            events.append("config-pass" if stages == gate.CONFIG_STAGES else harness)

        def compile_libraries(*args, **kwargs):
            self.assertEqual(events, ["fsm", "source"])
            self.assertEqual(kwargs, {"lock_fds": (77,), "harnesses": libraries})
            events.append("library-build")
            return FixtureCopies({name: "/warm/" + name for name in libraries})

        with patch.object(gate, "require_network_fixture_capacity"), \
             patch.object(gate, "run_pure_fsm_checks", side_effect=lambda *args: events.append("fsm")), \
             patch.object(gate, "run_lifecycle_source_checks", side_effect=lambda *args: events.append("source")), \
             patch.object(gate, "compile_harness") as separate, \
             patch.object(gate, "compile_test_harnesses", side_effect=compile_libraries) as batch, \
             patch.object(gate, "run_stages", side_effect=run_stage), \
             patch.object(gate, "run_network_checks", side_effect=gate.CheckError("stop after ordering check")), \
             patch.object(gate.subprocess, "check_output", side_effect=AssertionError("captured source requires no Git lookup")), \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "stop after ordering check"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=self.env,
                                source_commit="a" * 40, lock_fds=(77,))
        separate.assert_not_called()
        self.assertEqual(batch.call_count, 1)
        self.assertEqual(events, ["fsm", "source", "library-build", "config-pass",
                                  *["/warm/" + name for name in ("cli", "core", "torii-unit", "daemon") + libraries[1:-2]]])

    def test_batch_or_configuration_failure_stops_all_later_execution_and_passes(self):
        for phase in ("build", "schema"):
            output = io.StringIO()
            with self.subTest(phase=phase), \
                 patch.object(gate, "require_network_fixture_capacity"), \
                 patch.object(gate, "run_pure_fsm_checks"), \
                 patch.object(gate, "run_lifecycle_source_checks"), \
                 patch.object(gate, "compile_harness") as compile, \
                 patch.object(gate, "run_stages", side_effect=gate.SelectedRegressionFailures(["config schema failed"])) as run, \
                 patch.object(gate, "compile_test_harnesses", return_value=FixtureCopies("/warm/config"),
                              side_effect=gate.CheckError("config build failed") if phase == "build" else None) as libraries, \
                 patch.object(gate, "run_network_checks") as network, \
                 contextlib.redirect_stdout(output):
                checkpoint = MagicMock()
                with self.assertRaisesRegex(gate.CheckError, "config " + phase + " failed"):
                    gate.run_checks(Path("/frozen"), qualification_scope="full", environment=self.env,
                                    source_commit="a" * 40, lock_fds=(77, 88),
                                    update_independent_checks=checkpoint)
            compile.assert_not_called()
            self.assertEqual(libraries.call_count, 1)
            if phase == "build":
                run.assert_not_called()
            else:
                self.assertEqual(run.call_count, 1)
                self.assertEqual(run.call_args.args[3:], (gate.CONFIG_STAGES, (77, 88)))
            checkpoint.assert_not_called()
            network.assert_not_called()
            self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_configuration_always_runs_before_exact_independent_checkpoint_reuse(self):
        for config_fails in (False, True):
            with self.subTest(config_fails=config_fails), contextlib.ExitStack() as stack:
                for name in ("CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES", "TEST_NETWORK_STAGES",
                             "CLIENT_STAGES", "TORII_UNIT_STAGES", "TORII_STAGES", "DAEMON_STAGES",
                             "PROOF_STAGES", "PROOF_FLOW_STAGES"):
                    stack.enter_context(patch.object(gate, name, ()))
                copies = FixtureCopies({name: "/warm/" + name for name in ("config", "cli", "network")})
                copies.observations = [{"selection": name, "sha256": str(index) * 64, "size": 20,
                                        "cargo_artifact": {"name": name}}
                                       for index, name in enumerate(copies, 1)]
                evidence = gate.independent_check_evidence(copies, (("cli", gate.STAGES),), qualification_scope="full")
                for name in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "require_network_fixture_capacity"):
                    stack.enter_context(patch.object(gate, name))
                batch = stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                separate = stack.enter_context(patch.object(gate, "compile_harness"))
                network = stack.enter_context(patch.object(gate, "run_network_checks"))
                run = stack.enter_context(patch.object(gate, "run_stages", side_effect=
                    gate.SelectedRegressionFailures(["config failed"]) if config_fails else None))
                released = stack.enter_context(patch.object(copies, "release"))
                output = stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
                checkpoint = MagicMock()
                arguments = dict(environment=self.env, source_commit="a" * 40,
                                 completed_independent_checks=evidence,
                                 update_independent_checks=checkpoint)
                if config_fails:
                    with self.assertRaisesRegex(gate.SelectedRegressionFailures, "config failed"):
                        gate.run_checks(Path("/frozen"), qualification_scope="full", **arguments)
                    network.assert_not_called()
                    self.assertNotIn("reused exact", output.getvalue())
                    self.assertNotIn("[taira-check] PASS:", output.getvalue())
                else:
                    gate.run_checks(Path("/frozen"), qualification_scope="full", **arguments)
                    network.assert_called_once()
                    self.assertIn("reused exact", output.getvalue())
                    self.assertEqual([call.args[0] for call in released.call_args_list], ["config", "cli", "network"])
                run.assert_called_once()
                self.assertEqual(run.call_args.args[3], gate.CONFIG_STAGES)
                self.assertEqual(batch.call_args.kwargs["harnesses"], ("config", "network", "cli"))
                checkpoint.assert_not_called()
                separate.assert_not_called()
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)


class NetworkFixtureCapacityTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)

    def test_storage_floor_accepts_exact_boundary_and_rejects_one_byte_less(self):
        for available, passes in ((gate.NETWORK_FIXTURE_FREE_BYTES, True),
                                  (gate.NETWORK_FIXTURE_FREE_BYTES - 1, False)):
            with self.subTest(available=available), patch.object(
                    gate.shutil, "disk_usage", return_value=MagicMock(free=available)) as usage:
                if passes:
                    gate.require_network_fixture_capacity(Path("/warm"))
                else:
                    with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                        gate.require_network_fixture_capacity(Path("/warm"))
                usage.assert_called_once_with(Path("/warm"))

    def test_insufficient_space_stops_before_compilation(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        with patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=0)), \
             patch.object(gate, "compile_harness") as compile, \
             patch.object(gate, "run_pure_fsm_checks") as fsm, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40)
        compile.assert_not_called()
        fsm.assert_not_called()

    def test_capacity_is_checked_again_after_builds_before_starting_peers(self):
        with patch.object(gate, "compile_network_binaries", return_value={"iroha3d": "/node", "iroha": "/cli"}), \
             patch.object(gate, "compile_harness", return_value=FixtureCopies("/harness")), \
             patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=0)), \
             patch.object(gate, "run_stages") as run, patch.object(gate.tempfile, "mkdtemp") as fixture:
            with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, (), harness="/harness")
        run.assert_not_called()
        fixture.assert_not_called()


class NativeTestBatchBuildTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)

    names = ("crypto", "p2p", "core", "test-network")
    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}

    @staticmethod
    def artifact(name, executable=None):
        return json.dumps({"reason": "compiler-artifact", "target": {
            "name": gate.HARNESS_TARGETS[name][1], "kind": [gate.HARNESS_TARGETS[name][2]]}, "profile": {"test": True},
            "executable": executable or "/warm/" + name}) + "\n"

    def process(self, lines, code=0):
        child = MagicMock()
        child.stdout = io.StringIO(lines)
        child.wait.return_value = code
        process = MagicMock()
        process.__enter__.return_value = child
        return process

    def test_one_build_preserves_captured_cargo_custody_and_complete_selected_artifacts(self):
        lines = "[cargo-fast] warm lane\n" + "".join(self.artifact(name) for name in reversed(self.names))
        lines += self.artifact("crypto")  # Repeating the same exact artifact is harmless.
        lines += json.dumps({"reason": "compiler-artifact", "target": {"name": "unrelated", "kind": ["lib"]},
                             "profile": {"test": True}, "executable": "/warm/unrelated"}) + "\n"
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), \
             contextlib.redirect_stdout(io.StringIO()):
            actual = gate.compile_test_harnesses(Path("/frozen"), self.env,
                                                    harnesses=self.names, lock_fds=(77, 88))
        self.assertEqual(actual, {name: "/warm/" + name for name in self.names})
        self.assertEqual(spawn.call_count, 1)
        self.assertEqual(spawn.call_args.args[0], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml",
            "test", "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "iroha_crypto", "-p", "iroha_p2p", "-p", "iroha_core", "-p", "iroha_test_network",
            "--lib", "--no-run", "--message-format=json-render-diagnostics"])
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertIs(spawn.call_args.kwargs["env"], self.env)
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))

    def test_mixed_batch_includes_configuration_in_one_graph_and_requires_every_artifact(self):
        names = ("config", "proof", "proof-flows", "crypto", "p2p", "core", "test-network", "client", "torii-unit", "torii", "network")
        lines = "".join(self.artifact(name) for name in reversed(names))
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines)) as spawn, \
             patch.object(gate, "isolate_native_artifacts", side_effect=lambda root, env, rows: {name: row["executable"] for name, row in rows.items()}), \
             contextlib.redirect_stdout(io.StringIO()):
            actual = gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names, lock_fds=(77,))
        self.assertEqual(set(actual), set(names))
        self.assertEqual(spawn.call_count, 1)
        self.assertEqual(spawn.call_args.args[0], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml",
            "test", "--manifest-path", "/frozen/Cargo.toml", "--locked", "--offline",
            "-p", "iroha_config", "-p", "fastpq_prover", "-p", "iroha_crypto", "-p", "iroha_p2p", "-p", "iroha_core", "-p", "iroha_test_network",
            "-p", "iroha", "-p", "iroha_torii", "--test", "taira_config_contracts", "--lib", "--test", "fastpq_integration", "--test", "taira_app_contracts",
            "--test", "taira_consensus_contracts", "--no-run", "--message-format=json-render-diagnostics"])
        for missing in ("config", "torii", "network"):
            incomplete = "".join(self.artifact(name) for name in names if name != missing)
            with self.subTest(missing=missing), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(incomplete)), \
                 patch.object(gate, "isolate_native_artifacts") as isolate, \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "0 test executables"):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names)
            isolate.assert_not_called()

    def test_invalid_selections_fail_before_cargo(self):
        for names in ((), ("crypto", "crypto"), ("unreviewed",), ("iroha",)):
            with self.subTest(names=names), patch.object(gate.subprocess, "Popen") as spawn:
                with self.assertRaises(gate.CheckError):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=names)
                spawn.assert_not_called()

    def test_incomplete_ambiguous_wrong_profile_or_shared_artifacts_fail_closed(self):
        good = "".join(self.artifact(name) for name in self.names)
        wrong_profile = json.loads(self.artifact("test-network"))
        wrong_profile["profile"]["test"] = False
        wrong_kind = json.loads(self.artifact("test-network"))
        wrong_kind["target"]["kind"] = ["bin"]
        without_fixture = "".join(self.artifact(name) for name in self.names[:-1])
        for lines, error in ((without_fixture, "0 test executables"),
                             (without_fixture + json.dumps(wrong_profile) + "\n", "0 test executables"),
                             (without_fixture + json.dumps(wrong_kind) + "\n", "0 test executables"),
                             (good + self.artifact("crypto", "/warm/other"), "2 test executables"),
                             ("".join(self.artifact(name, "/warm/same") for name in self.names), "reused one executable")):
            with self.subTest(error=error), \
                 patch.object(gate.subprocess, "Popen", return_value=self.process(lines)), \
                 contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, error):
                    gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=self.names)

    def test_cargo_failure_with_complete_artifacts_still_fails_and_preserves_diagnostic(self):
        diagnostic = "error[E0308]: synthetic library build failure\n"
        lines = "".join(self.artifact(name) for name in self.names)
        lines += json.dumps({"reason": "compiler-message", "message": {"rendered": diagnostic}}) + "\n"
        stderr = io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines, 101)), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
            with self.assertRaisesRegex(gate.CheckError, "build failed"):
                gate.compile_test_harnesses(Path("/frozen"), self.env, harnesses=self.names)
        self.assertEqual(stderr.getvalue(), diagnostic)

    def test_failed_batch_stops_before_any_native_test_or_network_start(self):
        with patch.object(gate, "run_pure_fsm_checks"), patch.object(gate, "run_lifecycle_source_checks"), \
             patch.object(gate, "run_config_checks"), \
             patch.object(gate, "require_network_fixture_capacity"), \
             patch.object(gate, "compile_test_harnesses", side_effect=gate.CheckError("batch failed")), \
             patch.object(gate, "run_stages") as run, patch.object(gate, "run_network_checks") as network, \
             patch.object(gate, "compile_harness") as other, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "batch failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=self.env, source_commit="a" * 40)
        run.assert_not_called()
        network.assert_not_called()
        other.assert_not_called()



class NativeArtifactIsolationTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name).resolve()
        self.target = self.directory / "warm"
        self.source = self.target / "taira-release-sources" / "fixture" / "source"
        self.source.mkdir(parents=True, mode=0o700)
        (self.target / "debug").mkdir(mode=0o700)
        self.env = {"CARGO": "/fixed/cargo", "CARGO_TARGET_DIR": str(self.target)}
        import taira_cargo_cache as cache
        import release_artifact_contract as contract
        self.cache, self.contract = cache, contract
        metadata = patch.object(cache, "local_package_names", return_value={"irohad", "iroha_cli"})
        self.metadata = metadata.start()
        self.addCleanup(metadata.stop)
        self.stdout = io.StringIO()
        redirect = contextlib.redirect_stdout(self.stdout)
        redirect.__enter__()
        self.addCleanup(redirect.__exit__, None, None, None)
        # Read-only captures are retained in production; release test-owned paths for cleanup.
        self.addCleanup(self.make_fixture_writable)

    def make_fixture_writable(self):
        for path in self.target.glob("taira-native-artifacts-*"):
            path.chmod(0o700)
            for child in path.iterdir():
                child.chmod(0o600)

    def artifact(self, selection="iroha", payload=b"#!/bin/sh\nexit 0\n"):
        if selection in ("iroha", "iroha3d"):
            package = "iroha_cli" if selection == "iroha" else "irohad"
            name, kind, is_test = selection, "bin", False
            executable = self.target / "debug" / selection
        else:
            _, name, kind, arguments = gate.HARNESS_TARGETS[selection]
            package, is_test = arguments[1], True
            executable = self.target / "debug" / "deps" / (name + "-" + selection + "-0123456789abcdef")
            executable.parent.mkdir(exist_ok=True)
        executable.write_bytes(payload)
        executable.chmod(0o700)
        row = {"name": name, "executable": str(executable), "profile": {"test": is_test},
               "manifest_path": str(self.source / "crates" / package / "Cargo.toml")}
        event = {"reason": "compiler-artifact", "target": {"name": name, "kind": [kind]},
                 **{key: value for key, value in row.items() if key != "name"}}
        return executable, row, event

    def assert_profile_locked(self):
        fd = os.open(self.target / "debug" / ".cargo-lock", os.O_RDWR)
        try:
            with self.assertRaises(BlockingIOError):
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        finally:
            os.close(fd)

    def assert_profile_unlocked(self):
        fd = os.open(self.target / "debug" / ".cargo-lock", os.O_RDWR)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        finally:
            os.close(fd)

    def isolate(self, rows):
        return gate.isolate_native_artifacts(self.source, self.env, rows)

    def test_real_profile_lock_covers_validation_and_copy_then_releases(self):
        executable, row, _ = self.artifact()
        original = self.contract.stable_hash_path
        def guarded_hash(*args, **kwargs):
            self.assert_profile_locked()
            return original(*args, **kwargs)
        with patch.object(self.contract, "stable_hash_path", side_effect=guarded_hash):
            actual = self.isolate({"iroha": row})
        self.assert_profile_unlocked()
        self.metadata.assert_called_once_with(self.source, self.env)
        copied = Path(actual["iroha"])
        self.assertNotEqual(executable.stat().st_ino, copied.stat().st_ino)
        self.assertEqual(copied.stat().st_nlink, 1)
        self.assertEqual(stat.S_IMODE(copied.stat().st_mode), 0o500)
        self.assertEqual(stat.S_IMODE(copied.parent.stat().st_mode), 0o500)
        original_bytes = copied.read_bytes()
        executable.write_bytes(b"replacement from another build")
        self.assertEqual(copied.read_bytes(), original_bytes)
        replacement = executable.with_suffix(".next")
        replacement.write_bytes(b"another replacement")
        os.replace(replacement, executable)
        self.assertEqual(copied.read_bytes(), original_bytes)
        prefix = "[taira-check] isolated native artifact "
        events = [json.loads(line[len(prefix):]) for line in self.stdout.getvalue().splitlines()
                  if line.startswith(prefix)]
        self.assertEqual(events, [{"selection": "iroha", "path": str(copied),
            "sha256": hashlib.sha256(original_bytes).hexdigest(), "size": len(original_bytes),
            "cargo_artifact": row}])

    def test_strict_foreign_fingerprints_reject_without_retirement_or_publication(self):
        _, row, _ = self.artifact()
        directory = self.target / "debug" / ".fingerprint" / "iroha_cli-0123456789abcdef"
        directory.mkdir(parents=True)
        path = b"src/main.rs"
        raw = b"\x01\x00\x00\x00\xff\x01" + struct.pack("<I", 1)
        raw += b"\x00" + struct.pack("<I", len(path)) + path + b"\x00" + struct.pack("<I", 0)
        record = directory / "dep-bin-iroha"
        record.write_bytes(raw)
        record.chmod(0o600)
        with self.assertRaisesRegex(gate.CheckError, "foreign Cargo source fingerprints"):
            self.isolate({"iroha": row})
        self.assertEqual(record.read_bytes(), raw)
        self.assertFalse((self.target / "taira-release-cache-retired").exists())
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_unsafe_manifest_and_paths_fail_before_metadata_or_copy(self):
        executable, row, _ = self.artifact()
        outside = self.directory / "outside"
        outside.write_bytes(b"outside"); outside.chmod(0o700)
        symlink = self.target / "debug" / "link"
        symlink.symlink_to(executable)
        directory_link = self.target / "linked-debug"
        directory_link.symlink_to(self.target / "debug", target_is_directory=True)
        for changed in (row | {"manifest_path": str(self.directory / "foreign/Cargo.toml")},
                        row | {"executable": str(outside)}, row | {"executable": "relative"},
                        row | {"executable": str(symlink)},
                        row | {"executable": str(directory_link / "iroha")}):
            with self.subTest(record=changed), self.assertRaises(gate.CheckError):
                self.isolate({"iroha": changed})
        self.metadata.assert_not_called()
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def test_hardlink_nonexecutable_unsafe_mode_empty_and_nonregular_are_rejected(self):
        executable, row, _ = self.artifact()
        for mode in (0o600, 0o722):
            executable.chmod(mode)
            with self.subTest(mode=mode), self.assertRaises(gate.CheckError):
                self.isolate({"iroha": row})
        executable.chmod(0o700)
        sibling = executable.with_suffix(".linked")
        os.link(executable, sibling)
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        sibling.unlink()
        executable.write_bytes(b"")
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        executable.unlink(); executable.mkdir()
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        executable.rmdir(); os.mkfifo(executable)
        with self.assertRaises(gate.CheckError): self.isolate({"iroha": row})
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def test_replacement_after_validation_is_rejected_before_execution(self):
        executable, row, _ = self.artifact()
        original = self.contract.stable_hash_path
        def replace_after_hash(*args, **kwargs):
            result = original(*args, **kwargs)
            replacement = executable.with_suffix(".next")
            replacement.write_bytes(b"foreign executable"); replacement.chmod(0o700)
            os.replace(replacement, executable)
            return result
        with patch.object(self.contract, "stable_hash_path", side_effect=replace_after_hash):
            with self.assertRaisesRegex(gate.CheckError, "stable capture"):
                self.isolate({"iroha": row})
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_same_size_mutation_during_descriptor_copy_is_rejected(self):
        executable, row, _ = self.artifact()
        original_open, original_read = self.contract.stable_open_relative, os.read
        active = {"fd": None, "changed": False}
        @contextlib.contextmanager
        def track_open(*args, **kwargs):
            with original_open(*args, **kwargs) as fd:
                active["fd"] = fd
                try: yield fd
                finally: active["fd"] = None
        def mutate_after_read(fd, count):
            result = original_read(fd, count)
            if fd == active["fd"] and result and not active["changed"]:
                active["changed"] = True
                executable.write_bytes(b"x" * len(result))
            return result
        with patch.object(self.contract, "stable_open_relative", side_effect=track_open), \
             patch.object(gate.os, "read", side_effect=mutate_after_read):
            with self.assertRaisesRegex(gate.CheckError, "changed while"):
                self.isolate({"iroha": row})
        self.assertTrue(active["changed"])
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_destination_replacement_before_publication_does_not_emit_artifact(self):
        _, row, _ = self.artifact()
        original_open = self.contract.stable_open_relative
        @contextlib.contextmanager
        def replace_destination_after_copy(*args, **kwargs):
            with original_open(*args, **kwargs) as fd:
                yield fd
            output, = self.target.glob("taira-native-artifacts-*")
            destination = output / "iroha"
            replacement = output / "replacement"
            replacement.write_bytes(b"foreign"); replacement.chmod(0o500)
            os.replace(replacement, destination)
        with patch.object(self.contract, "stable_open_relative", side_effect=replace_destination_after_copy):
            with self.assertRaisesRegex(gate.CheckError, "before publication"):
                self.isolate({"iroha": row})
        self.assertNotIn("isolated native artifact", self.stdout.getvalue())
        self.assert_profile_unlocked()

    def test_capacity_reserve_failure_does_not_publish_or_create_copy_directory(self):
        executable, row, _ = self.artifact()
        space = MagicMock(free=gate.NETWORK_FIXTURE_FREE_BYTES + executable.stat().st_size - 1)
        with patch.object(gate.shutil, "disk_usage", return_value=space):
            with self.assertRaisesRegex(gate.CheckError, "working-space reserve"):
                self.isolate({"iroha": row})
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def process(self, events, code=0):
        child = MagicMock()
        child.stdout = io.StringIO("\n".join(json.dumps(event) for event in events))
        child.wait.return_value = code
        process = MagicMock()
        process.__enter__.return_value = child
        return process

    def test_every_harness_and_network_binary_uses_an_isolated_execution_path(self):
        for selection in gate.HARNESS_TARGETS:
            with self.subTest(selection=selection):
                executable, _, event = self.artifact(selection)
                with patch.object(gate.subprocess, "Popen", return_value=self.process([event])):
                    copies = gate.compile_harness(self.source, self.env, harness=selection)
                    self.addCleanup(copies.__exit__, None, None, None)
                    path = copies[selection]
                self.assertNotEqual(path, str(executable))
                self.assertTrue(Path(path).parent.name.startswith("taira-native-artifacts-"))
                self.assertEqual(Path(path).read_bytes(), executable.read_bytes())
        rows = [self.artifact(selection) for selection in ("iroha3d", "iroha")]
        with patch.object(gate.subprocess, "Popen", return_value=self.process([row[2] for row in rows])):
            copied = gate.compile_network_binaries(self.source, self.env, (77,))
        self.assertEqual(set(copied), {"iroha3d", "iroha"})
        for selection, path in copied.items():
            self.assertNotEqual(path, str(self.target / "debug" / selection))
        self.assert_profile_unlocked()

    def test_batched_libraries_and_integrations_copy_every_accepted_artifact_before_returning(self):
        selections = ("crypto", "p2p", "core", "test-network", "client", "torii-unit", "torii", "network")
        events = [self.artifact(selection)[2] for selection in selections]
        with patch.object(gate.subprocess, "Popen", return_value=self.process(events)) as cargo:
            copies = gate.compile_test_harnesses(self.source, self.env,
                                                   harnesses=selections, lock_fds=(77, 88))
        self.assertEqual(cargo.call_count, 1)
        self.addCleanup(copies.__exit__, None, None, None)
        self.assertEqual(set(copies), set(selections))
        self.assertEqual(len({Path(path).parent for path in copies.values()}), 1)
        for selection, path in copies.items():
            self.assertEqual(Path(path).name, selection)
            self.assertEqual(stat.S_IMODE(Path(path).stat().st_mode), 0o500)
        self.assert_profile_unlocked()

    def test_real_harness_exit_releases_copy_on_success_and_failure_with_logs_retained(self):
        for succeeds in (True, False):
            with self.subTest(succeeds=succeeds):
                payload = (f"#!{sys.executable}\nimport sys\nfrom pathlib import Path\n"
                    "assert Path(sys.argv[0]).is_file()\n"
                    "if '--list' in sys.argv: print('fixture: test'); sys.exit(0)\n"
                    + ("print('test fixture ... ok\\ntest result: ok. 1 passed; 0 failed; 0 ignored;')\n"
                       if succeeds else "print('diagnostic retained'); sys.exit(101)\n")).encode()
                original, row, event = self.artifact("core", payload)
                with patch.object(gate.subprocess, "Popen", return_value=self.process([event])):
                    copies = gate.compile_harness(self.source, self.env, harness="core")
                copied = Path(copies["core"])
                error_output = io.StringIO()
                expected = contextlib.nullcontext() if succeeds else self.assertRaisesRegex(gate.CheckError, "fixture.*101")
                with expected, contextlib.redirect_stderr(error_output):
                    with copies:
                        gate.run_stages(copies["core"], self.target, dict(os.environ),
                                        (("real child", ("fixture",)),), ())
                        self.assertTrue(copied.exists(), "copy lives until the child has completed")
                self.assertFalse(copied.exists())
                self.assertEqual(original.read_bytes(), payload)
                self.assertEqual(stat.S_IMODE(copied.parent.stat().st_mode), 0o500)
                self.assertIn('released native test artifact', self.stdout.getvalue())
                self.assertIn(hashlib.sha256(payload).hexdigest(), self.stdout.getvalue())
                if not succeeds:
                    self.assertIn('diagnostic retained', error_output.getvalue())

    def test_batch_releases_completed_then_unused_tests_preserving_native_and_unowned_files(self):
        rows = {name: self.artifact(name)[1] for name in ("core", "network", "iroha", "iroha3d")}
        copies = self.isolate(rows)
        output = Path(copies["core"]).parent
        output.chmod(0o700)
        note = output / "diagnostic.txt"
        note.write_text("retain diagnostic")
        output.chmod(0o500)
        with self.assertRaisesRegex(gate.CheckError, "later fixture failed"):
            with copies:
                copies.release("core")
                self.assertFalse(Path(copies["core"]).exists())
                self.assertTrue(Path(copies["network"]).exists())
                raise gate.CheckError("later fixture failed")
        self.assertFalse(Path(copies["network"]).exists())
        for name in ("iroha", "iroha3d"):
            self.assertTrue(Path(copies[name]).exists())
        for row in rows.values():
            self.assertTrue(Path(row["executable"]).exists(), "Cargo output must remain warm")
        self.assertEqual(note.read_text(), "retain diagnostic")
        self.assertIsNone(copies.directory_fd)

    def test_failed_early_run_preserves_original_failure_and_releases_only_owned_batch(self):
        for replaced in (False, True):
            with self.subTest(replaced=replaced):
                rows = {name: self.artifact(name)[1] for name in ("cli", "client")}
                copies = self.isolate(rows)
                copied = Path(copies["cli"])
                def failed_stage(*args):
                    if replaced:
                        copied.parent.chmod(0o700)
                        copied.unlink()
                        copied.write_bytes(b"foreign replacement")
                        copied.chmod(0o500)
                        copied.parent.chmod(0o500)
                    raise gate.CheckError("early CLI fixture failed")
                errors = io.StringIO()
                with contextlib.ExitStack() as stack:
                    for name in ("CRYPTO_STAGES", "P2P_STAGES", "TEST_NETWORK_STAGES", "TORII_UNIT_STAGES", "TORII_STAGES", "NETWORK_STAGES"):
                        stack.enter_context(patch.object(gate, name, ()))
                    for name in ("run_pure_fsm_checks", "run_lifecycle_source_checks", "run_config_checks"):
                        stack.enter_context(patch.object(gate, name))
                    stack.enter_context(patch.object(gate, "compile_test_harnesses", return_value=copies))
                    network = stack.enter_context(patch.object(gate, "compile_network_binaries"))
                    stack.enter_context(patch.object(gate, "run_stages", side_effect=failed_stage))
                    stack.enter_context(contextlib.redirect_stderr(errors))
                    with self.assertRaisesRegex(gate.CheckError, "early CLI fixture failed"):
                        gate.run_checks(self.source, qualification_scope="full", environment=self.env | {"CARGO_HOME": "/isolated"},
                                        source_commit="a" * 40)
                if replaced:
                    self.assertEqual(copied.read_bytes(), b"foreign replacement")
                    self.assertIn("changed before release: cli", errors.getvalue())
                else:
                    self.assertFalse(copied.exists())
                self.assertFalse(Path(copies["client"]).exists())
                self.assertTrue(all(Path(row["executable"]).exists() for row in rows.values()))
                network.assert_not_called()
                self.assertIsNone(copies.directory_fd)

    def test_replaced_test_copy_is_retained_and_other_owned_copy_is_released(self):
        copies = self.isolate({name: self.artifact(name)[1] for name in ("core", "network")})
        original = Path(copies["core"])
        output = original.parent
        output.chmod(0o700)
        original.unlink()
        original.write_bytes(b"foreign replacement")
        original.chmod(0o500)
        output.chmod(0o500)
        with self.assertRaisesRegex(gate.CheckError, "changed before release: core"):
            with copies:
                pass
        self.assertEqual(original.read_bytes(), b"foreign replacement")
        self.assertFalse(Path(copies["network"]).exists())
        self.assertIsNone(copies.directory_fd)

    def test_changed_directory_never_deletes_replacement_or_masks_test_failure(self):
        copies = self.isolate({"core": self.artifact("core")[1]})
        output = Path(copies["core"]).parent
        archived = output.with_name(output.name + "-renamed")
        output.rename(archived)
        output.mkdir(mode=0o700)
        foreign = output / "core"
        foreign.write_bytes(b"foreign")
        foreign.chmod(0o500)
        output.chmod(0o500)
        errors = io.StringIO()
        with self.assertRaisesRegex(gate.CheckError, "actual regression failed"), contextlib.redirect_stderr(errors):
            with copies:
                raise gate.CheckError("actual regression failed")
        self.assertEqual(foreign.read_bytes(), b"foreign")
        self.assertTrue((archived / "core").exists())
        self.assertIn("directory changed before release", errors.getvalue())
        self.assertIsNone(copies.directory_fd)

    def test_empty_and_unknown_selections_reject_before_creating_output(self):
        for rows in ({}, {"../foreign": {}}):
            with self.assertRaisesRegex(gate.CheckError, "known nonempty"):
                self.isolate(rows)
        self.metadata.assert_not_called()
        self.assertEqual(list(self.target.glob("taira-native-artifacts-*")), [])

    def test_cargo_failure_and_ambiguous_metadata_never_copy(self):
        _, _, event = self.artifact("core")
        for events, code in (([event], 101),
                             ([event, event | {"manifest_path": "/other/Cargo.toml"}], 0)):
            with patch.object(gate.subprocess, "Popen", return_value=self.process(events, code)), \
                 patch.object(gate, "isolate_native_artifacts") as isolate:
                with self.assertRaises(gate.CheckError):
                    gate.compile_harness(self.source, self.env, harness="core")
                isolate.assert_not_called()

    def test_mutable_development_copy_uses_real_profile_lock_without_source_claim(self):
        executable, row, _ = self.artifact()
        development = self.directory / "checkout"
        development.mkdir(mode=0o700)
        row["manifest_path"] = str(development / "crates/iroha_cli/Cargo.toml")
        original = self.contract.stable_hash_path
        def guarded_hash(*args, **kwargs):
            self.assert_profile_locked()
            return original(*args, **kwargs)
        with patch.object(self.contract, "stable_hash_path", side_effect=guarded_hash):
            copied = gate.isolate_native_artifacts(development, self.env, {"iroha": row})
        self.assertNotEqual(copied["iroha"], str(executable))
        self.metadata.assert_not_called()
        self.assert_profile_unlocked()

class PureFsmGateTests(unittest.TestCase):
    def setUp(self):
        isolate_shipping_fixture(self)
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.target = Path(self.directory.name).resolve()
        self.env = {"RUSTC": "/pinned/rustc", "CARGO_TARGET_DIR": str(self.target)}

    @staticmethod
    def results(output=None, code=0):
        return [subprocess.CompletedProcess([], 0, "", ""),
                subprocess.CompletedProcess([], 0, "one: test\ntwo: test\n", ""),
                subprocess.CompletedProcess([], code, output if output is not None else
                    "test two ... ok\ntest one ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n", "")]

    def test_exact_production_source_all_tests_and_lock_custody(self):
        output = io.StringIO()
        with patch.object(gate.subprocess, "run", side_effect=self.results()) as run, contextlib.redirect_stdout(output):
            gate.run_pure_fsm_checks(Path("/frozen"), self.env, (77, 88))
        executable = str(self.target / "taira-consensus-fsm-check/sumeragi-core-tests")
        self.assertEqual(run.call_args_list[0].args[0], ["/pinned/rustc", "--edition=2024", "--test",
            "/frozen/crates/iroha_sumeragi_core/src/lib.rs", "-o", executable])
        self.assertEqual(run.call_args_list[1].args[0], [executable, "--list", "--format", "terse"])
        self.assertEqual(run.call_args_list[2].args[0], [executable, "--color", "never", "--test-threads=6"])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["pass_fds"], (77, 88))
            self.assertEqual(call.kwargs["env"], self.env)
            self.assertEqual(call.kwargs["cwd"], "/")
        self.assertIn("pure FSM PASS: 2 listed, 2 passed, 0 ignored", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_empty_duplicate_or_malformed_census_never_executes_suite(self):
        for listing in ("", "one: test\none: test\n", "one: test\nother: benchmark\n"):
            results = self.results(); results[1] = subprocess.CompletedProcess([], 0, listing, "")
            with patch.object(gate.subprocess, "run", side_effect=results) as run, contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "census"):
                    gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
            self.assertEqual(run.call_count, 2)

    def test_lifecycle_source_gate_uses_captured_shared_assertions_and_same_locks(self):
        results = [subprocess.CompletedProcess([], 0, "", "source asset audit passed"),
                   subprocess.CompletedProcess([], 0, "", "native instruction audit passed")] + self.results()
        with patch.object(gate.subprocess, "run", side_effect=results) as run, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            gate.run_lifecycle_source_checks(Path("/frozen"), self.env, (77, 88))
        self.assertEqual(run.call_args_list[0].args[0], [sys.executable, "-I", "-B",
            "/frozen/scripts/tests/sumeragi_source_contract_asset_compaction_test.py"])
        executable = str(self.target / "taira-consensus-fsm-check/lifecycle-source-tests")
        self.assertEqual(run.call_args_list[1].args[0], [sys.executable, "-I", "-B",
            "/frozen/scripts/check_taira_initial_executor.py", "--repo", "/frozen", "--self-test"])
        self.assertEqual(run.call_args_list[2].args[0], ["/pinned/rustc", "--edition=2024", "--test",
            "/frozen/crates/iroha_core/src/sumeragi/v2_lifecycle_source_contract_harness.rs",
            "-o", executable])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["pass_fds"], (77, 88))
            self.assertEqual(call.kwargs["env"], self.env)
            self.assertEqual(call.kwargs["cwd"], "/")
        self.assertIn("lifecycle source contracts PASS: 2 listed, 2 passed, 0 ignored", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_native_instruction_audit_failure_stops_before_rust_or_cargo(self):
        results = [subprocess.CompletedProcess([], 0, "", ""),
                   subprocess.CompletedProcess([], 1, "", "missing reviewed disposition\n")]
        with patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "_run_standalone_checks") as rust, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as error:
            with self.assertRaisesRegex(gate.CheckError, "native Initial instruction source audit failed"):
                gate.run_lifecycle_source_checks(Path("/frozen"), self.env, (77,))
        self.assertEqual(run.call_count, 2)
        rust.assert_not_called()
        self.assertEqual(error.getvalue(), "missing reviewed disposition\n")

    def test_asset_audit_failure_stops_before_rust_or_cargo_and_preserves_diagnostic(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        diagnostic = "AssertionError: invalid region edge: from/before\n"
        with patch.object(gate, "run_pure_fsm_checks"), \
             patch.object(gate.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", diagnostic)) as run, \
             patch.object(gate, "_run_standalone_checks") as rust, \
             patch.object(gate, "run_config_checks") as config, \
             patch.object(gate, "compile_test_harnesses") as libraries, \
             patch.object(gate, "run_network_checks") as network, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()) as error:
            with self.assertRaisesRegex(gate.CheckError, "source-asset grammar and inventory audit failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.kwargs["pass_fds"], (77,))
        self.assertEqual(run.call_args.kwargs["timeout"], 120)
        self.assertEqual(error.getvalue(), diagnostic)
        rust.assert_not_called()
        config.assert_not_called()
        libraries.assert_not_called()
        network.assert_not_called()
        self.assertEqual(gate.selected_regression_count("full"), EXPECTED_REGRESSION_COUNT)

    def test_lifecycle_failure_stops_before_any_cargo_or_network_work(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        with patch.object(gate, "run_pure_fsm_checks") as fsm, \
             patch.object(gate, "run_lifecycle_source_checks", side_effect=gate.CheckError("source contract failed")) as source, \
             patch.object(gate, "compile_test_harnesses") as libraries, \
             patch.object(gate, "compile_harness") as compile, \
             patch.object(gate, "run_network_checks") as network, \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "source contract failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        fsm.assert_called_once()
        source.assert_called_once_with(Path("/frozen"),
            env | {"VERGEN_GIT_SHA": "a" * 40, "IROHA_GIT_COMMIT_HASH": "a" * 40}, (77,))
        libraries.assert_not_called()
        compile.assert_not_called()
        network.assert_not_called()

    def test_partial_ignored_substituted_duplicate_and_failed_results_rejected(self):
        good = self.results()[-1].stdout
        cases = [(good.replace("test two ... ok\n", ""), 0),
                 (good.replace("test two ... ok", "test other ... ok"), 0),
                 (good + "test one ... ok\n", 0),
                 (good.replace("2 passed; 0 failed; 0 ignored", "1 passed; 0 failed; 1 ignored"), 0),
                 (good, 101)]
        for text, code in cases:
            with patch.object(gate.subprocess, "run", side_effect=self.results(text, code)), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaisesRegex(gate.CheckError, "without skips"):
                    gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())

    def test_compiler_failure_never_runs_stale_output(self):
        with patch.object(gate.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "compile failure")) as run, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "compilation failed"):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
        self.assertEqual(run.call_count, 1)

    def test_fsm_failure_precedes_any_native_network_or_cargo_build(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        with patch.object(gate, "run_pure_fsm_checks", side_effect=gate.CheckError("FSM failed")) as fsm, \
             patch.object(gate, "run_network_checks") as network, patch.object(gate, "compile_harness") as compile, \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "FSM failed"):
                gate.run_checks(Path("/frozen"), qualification_scope="full", environment=env, source_commit="a" * 40, lock_fds=(77,))
        fsm.assert_called_once()
        self.assertEqual(fsm.call_args.args[2], (77,))
        network.assert_not_called(); compile.assert_not_called()

    def test_unpinned_compiler_and_symlink_output_rejected_before_compilation(self):
        with patch.object(gate.subprocess, "run") as run:
            for compiler in ("rustc", ""):
                with self.assertRaisesRegex(gate.CheckError, "pinned RUSTC"):
                    gate.run_pure_fsm_checks(Path("/frozen"), self.env | {"RUSTC": compiler}, ())
            (self.target / "taira-consensus-fsm-check").symlink_to(self.target, target_is_directory=True)
            with self.assertRaisesRegex(gate.CheckError, "direct directory"):
                gate.run_pure_fsm_checks(Path("/frozen"), self.env, ())
        run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
