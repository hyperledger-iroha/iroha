"""Offline checks for the early-gate runner; no Cargo or live inputs required."""

import contextlib
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


SCRIPT = Path(__file__).with_name("taira_release_check.py")
if not SCRIPT.exists():
    SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release_check.py"
SPEC = importlib.util.spec_from_file_location("taira_release_check", SCRIPT)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class EarlyReleaseCheckTests(unittest.TestCase):
    def setUp(self):
        mock = patch.object(gate, "run_pure_fsm_checks")
        self.pure_fsm = mock.start()
        self.addCleanup(mock.stop)
        source = patch.object(gate, "run_lifecycle_source_checks")
        self.lifecycle_source = source.start()
        self.addCleanup(source.stop)
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
        self.assertIn(
            "taira_public_reset::host::tests::candidate_operator_status_child_binds_both_inherited_signers",
            names,
        )
        self.assertIn(
            "taira_public_reset::config::tests::operator_keygen_publishes_canonical_private_key_and_only_public_report",
            names,
        )

    def test_transport_selectors_are_mandatory_unique_and_fail_when_missing(self):
        for stages in (gate.CRYPTO_STAGES, gate.P2P_STAGES, gate.TEST_NETWORK_STAGES):
            names = [name for _, tests in stages for name in tests]
            self.assertTrue(names)
            self.assertEqual(len(names), len(set(names)))
            gate.require_tests("\n".join(f"{name}: test" for name in names), stages)
            with self.assertRaisesRegex(gate.CheckError, "required regressions missing"):
                gate.require_tests("\n".join(f"{name}: test" for name in names[1:]), stages)

    def test_complete_regression_census_tracks_every_native_stage_group(self):
        self.assertEqual(gate.selected_regression_count(), 235)
        for group in ("STAGES", "CRYPTO_STAGES", "P2P_STAGES", "CORE_STAGES",
                      "TEST_NETWORK_STAGES", "NETWORK_STAGES", "PROOF_STAGES",
                      "PROOF_FLOW_STAGES", "TORII_STAGES"):
            original_count = sum(len(names) for _, names in getattr(gate, group))
            with self.subTest(group=group), patch.object(gate, group, (("fixture", ("one", "two")),)):
                self.assertEqual(gate.selected_regression_count(), 235 - original_count + 2)

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
        with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, contextlib.redirect_stdout(io.StringIO()):
            gate.compile_harness(Path("/frozen"), {"CARGO": "/fixed/cargo"}, lock_fds=(77, 88))
        self.assertEqual(spawn.call_args.args[0][:4], ["/fixed/cargo", "--config", "/frozen/.cargo/config.toml", "test"])
        self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
        self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77, 88))
        with patch.object(gate.subprocess, "check_output", side_effect=AssertionError("must not inspect mutable Git")), \
             patch.object(gate, "compile_harness", return_value="/fixture/harness") as compile, \
             patch.object(gate.subprocess, "run", side_effect=[subprocess.CompletedProcess([], 0, "fixture: test\n", ""), subprocess.CompletedProcess([], 0, "test fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "")]) as run, \
             patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "CORE_STAGES", ()), \
             patch.object(gate, "TEST_NETWORK_STAGES", ()), \
             patch.object(gate, "NETWORK_STAGES", ()), \
             patch.object(gate, "PROOF_STAGES", ()), \
             patch.object(gate, "PROOF_FLOW_STAGES", ()), \
             patch.object(gate, "TORII_STAGES", ()), contextlib.redirect_stdout(io.StringIO()):
            gate.run_checks(Path("/frozen"), environment={"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}, source_commit="a" * 40, lock_fds=(77, 88))
        self.assertEqual([call.kwargs["cwd"] for call in run.call_args_list], [Path("/warm"), Path("/warm")])
        self.assertNotIn("frozen", compile.call_args.kwargs)
        self.assertEqual(compile.call_args.args[1]["VERGEN_GIT_SHA"], "a" * 40)


    def test_mutable_check_keeps_git_checks_and_inherits_lane_lock_in_every_child(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/routine", "CARGO_INCREMENTAL": "0"}
        results = [subprocess.CompletedProcess([], 0, "fixture: test\n", ""),
                   subprocess.CompletedProcess([], 0, "test fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "")]
        with patch.object(gate.subprocess, "check_output", return_value="a" * 40) as git, \
             patch.object(gate, "compile_harness", return_value="/fixture/harness") as compile, \
             patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "CORE_STAGES", ()), \
             patch.object(gate, "TEST_NETWORK_STAGES", ()), \
             patch.object(gate, "NETWORK_STAGES", ()), \
             patch.object(gate, "PROOF_STAGES", ()), \
             patch.object(gate, "PROOF_FLOW_STAGES", ()), \
             patch.object(gate, "TORII_STAGES", ()), contextlib.redirect_stdout(io.StringIO()):
            gate.run_checks(Path("/mutable"), environment=env, lock_fds=(77,))
        self.assertEqual(git.call_count, 2)
        self.assertTrue(all(call.kwargs["env"]["CARGO_HOME"] == "/isolated" for call in git.call_args_list))
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,)})
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
        with patch.object(gate, "compile_harness", side_effect=["/warm/cli", "/warm/routes"]) as compile, \
             patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "STAGES", (("CLI", ("cli",)),)), \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "CORE_STAGES", ()), \
             patch.object(gate, "TEST_NETWORK_STAGES", ()), \
             patch.object(gate, "NETWORK_STAGES", ()), \
             patch.object(gate, "PROOF_STAGES", ()), \
             patch.object(gate, "PROOF_FLOW_STAGES", ()), \
             patch.object(gate, "TORII_STAGES", (("Torii", ("route",)),)), \
             contextlib.redirect_stdout(output), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "route.*exit 101"):
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 2)
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harness": "torii"})
        self.assertTrue(all(call.kwargs["cwd"] == Path("/warm") and call.kwargs["pass_fds"] == (77,)
                            for call in run.call_args_list))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_failed_core_progress_stops_before_torii_or_release_success(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        output = io.StringIO()
        with patch.object(gate, "compile_library_harnesses", return_value={
                "core": "/warm/core", "test-network": "/warm/fixture"}) as compile, \
             patch.object(gate, "run_network_checks") as network, \
             patch.object(gate, "CRYPTO_STAGES", ()), \
             patch.object(gate, "P2P_STAGES", ()), \
             patch.object(gate, "run_stages", side_effect=gate.CheckError("public transaction stalled")) as run, \
             contextlib.redirect_stdout(output):
            with self.assertRaisesRegex(gate.CheckError, "public transaction stalled"):
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 1)
        network.assert_not_called()
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "harnesses": ("core", "test-network")})
        self.assertEqual(run.call_args.args[3], gate.CORE_STAGES)
        self.assertEqual(run.call_args.args[4], (77,))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_network_failure_stops_before_independent_harness_builds(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("crypto", "p2p", "core", "test-network")
        with patch.object(gate, "run_network_checks", side_effect=gate.CheckError("consensus stalled")) as network, \
             patch.object(gate, "compile_library_harnesses", return_value={
                 name: "/warm/" + name for name in names}) as batch, \
             patch.object(gate, "compile_harness") as compile, \
             patch.object(gate, "run_stages") as stages, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "consensus stalled"):
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
        network.assert_called_once_with(Path("/frozen"), Path("/warm"),
            env | {"VERGEN_GIT_SHA": "a" * 40, "IROHA_GIT_COMMIT_HASH": "a" * 40}, (77,))
        self.assertEqual(batch.call_count, 1)
        self.assertEqual(batch.call_args.kwargs, {"lock_fds": (77,), "harnesses": names})
        compile.assert_not_called()
        self.assertEqual([call.args[0] for call in stages.call_args_list], ["/warm/" + name for name in names])
        self.assertEqual([call.args[3] for call in stages.call_args_list],
                         [gate.CRYPTO_STAGES, gate.P2P_STAGES, gate.CORE_STAGES, gate.TEST_NETWORK_STAGES])

    def test_transport_or_fixture_failure_stops_before_network_and_release_success(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
        names = ("crypto", "p2p", "core", "test-network")
        for failed, outcomes, expected in (("crypto", [gate.CheckError("crypto failed")], ["crypto"]),
                                           ("p2p", [None, gate.CheckError("p2p failed")], ["crypto", "p2p"]),
                                           ("fixture", [None, None, None, gate.CheckError("fixture failed")],
                                            ["crypto", "p2p", "core", "test-network"])):
            output = io.StringIO()
            with self.subTest(failed=failed), \
                 patch.object(gate, "compile_library_harnesses", return_value={
                     name: "/warm/" + name for name in names}) as compile, \
                 patch.object(gate, "run_stages", side_effect=outcomes) as run, \
                 patch.object(gate, "run_network_checks") as network, contextlib.redirect_stdout(output):
                with self.assertRaisesRegex(gate.CheckError, failed + " failed"):
                    gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
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

    def test_unisolated_low_level_check_is_rejected_before_git_or_cargo(self):
        with patch.object(gate.subprocess, "check_output") as git, patch.object(gate, "compile_harness") as compile:
            with self.assertRaisesRegex(gate.CheckError, "isolated Cargo environment"):
                gate.run_checks(Path("/mutable"), environment={})
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
            with patch.object(gate.subprocess, "Popen", return_value=process) as spawn, contextlib.redirect_stdout(io.StringIO()):
                if accepted:
                    result = gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"}, (77,))
                    self.assertEqual(result, {"iroha3d": "/warm/iroha3d", "iroha": "/warm/iroha"})
                else:
                    with self.assertRaisesRegex(gate.CheckError, "both executable artifacts"):
                        gate.compile_network_binaries(Path("/frozen"), {"CARGO": "/fixed/cargo"}, (77,))
            self.assertEqual(spawn.call_args.kwargs["cwd"], "/")
            self.assertEqual(spawn.call_args.kwargs["pass_fds"], (77,))
            self.assertNotIn("iroha3d_taira", spawn.call_args.args[0])

    def test_network_gate_forbids_fallback_builds_and_sandbox_skips(self):
        env = {"CARGO_TARGET_DIR": "/warm"}
        with patch.object(gate, "compile_network_binaries", return_value={"iroha3d": "/warm/node", "iroha": "/warm/client"}), \
             patch.object(gate, "compile_harness", return_value="/warm/network"), \
             patch.object(gate.tempfile, "mkdtemp", return_value="/warm/private-fixture") as fixture, \
             patch.object(gate, "run_stages") as run, contextlib.redirect_stdout(io.StringIO()):
            gate.run_network_checks(Path("/frozen"), Path("/warm"), env, (77, 88))
        selected = run.call_args.args[2]
        for key in ("IROHA_TEST_SKIP_BUILD", "IROHA_FAIL_ON_SANDBOX_SKIP", "IROHA_TEST_REQUIRE_NETWORK", "IROHA_TEST_SERIALIZE_NETWORKS", "IROHA_TEST_NETWORK_KEEP_DIRS"):
            self.assertEqual(selected[key], "1")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHAD"], "/warm/node")
        self.assertEqual(selected["TEST_NETWORK_BIN_IROHA"], "/warm/client")
        self.assertEqual(selected["TEST_NETWORK_TMP_DIR"], "/warm/private-fixture")
        self.assertEqual(run.call_args.args[3:], (gate.NETWORK_STAGES, (77, 88)))
        self.assertEqual(fixture.call_args.kwargs["dir"], Path("/warm"))


class NetworkFixtureCapacityTests(unittest.TestCase):
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
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40)
        compile.assert_not_called()
        fsm.assert_not_called()

    def test_capacity_is_checked_again_after_builds_before_starting_peers(self):
        with patch.object(gate, "compile_network_binaries", return_value={"iroha3d": "/node", "iroha": "/cli"}), \
             patch.object(gate, "compile_harness", return_value="/harness"), \
             patch.object(gate.shutil, "disk_usage", return_value=MagicMock(free=0)), \
             patch.object(gate, "run_stages") as run, patch.object(gate.tempfile, "mkdtemp") as fixture:
            with self.assertRaisesRegex(gate.CheckError, "four-peer fixtures require"):
                gate.run_network_checks(Path("/frozen"), Path("/warm"), {"CARGO_TARGET_DIR": "/warm"}, ())
        run.assert_not_called()
        fixture.assert_not_called()


class NativeLibraryBatchBuildTests(unittest.TestCase):
    names = ("crypto", "p2p", "core", "test-network")
    env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}

    @staticmethod
    def artifact(name, executable=None):
        return json.dumps({"reason": "compiler-artifact", "target": {
            "name": gate.HARNESS_TARGETS[name][1], "kind": ["lib"]}, "profile": {"test": True},
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
             contextlib.redirect_stdout(io.StringIO()):
            actual = gate.compile_library_harnesses(Path("/frozen"), self.env,
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

    def test_invalid_or_nonlibrary_selections_fail_before_cargo(self):
        for names in ((), ("crypto", "crypto"), ("unreviewed",), ("cli",), ("network",)):
            with self.subTest(names=names), patch.object(gate.subprocess, "Popen") as spawn:
                with self.assertRaises(gate.CheckError):
                    gate.compile_library_harnesses(Path("/frozen"), self.env, harnesses=names)
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
                    gate.compile_library_harnesses(Path("/frozen"), self.env, harnesses=self.names)

    def test_cargo_failure_with_complete_artifacts_still_fails_and_preserves_diagnostic(self):
        diagnostic = "error[E0308]: synthetic library build failure\n"
        lines = "".join(self.artifact(name) for name in self.names)
        lines += json.dumps({"reason": "compiler-message", "message": {"rendered": diagnostic}}) + "\n"
        stderr = io.StringIO()
        with patch.object(gate.subprocess, "Popen", return_value=self.process(lines, 101)), \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(stderr):
            with self.assertRaisesRegex(gate.CheckError, "build failed"):
                gate.compile_library_harnesses(Path("/frozen"), self.env, harnesses=self.names)
        self.assertEqual(stderr.getvalue(), diagnostic)

    def test_failed_batch_stops_before_any_native_test_or_network_start(self):
        with patch.object(gate, "run_pure_fsm_checks"), patch.object(gate, "run_lifecycle_source_checks"), \
             patch.object(gate, "require_network_fixture_capacity"), \
             patch.object(gate, "compile_library_harnesses", side_effect=gate.CheckError("batch failed")), \
             patch.object(gate, "run_stages") as run, patch.object(gate, "run_network_checks") as network, \
             patch.object(gate, "compile_harness") as other, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "batch failed"):
                gate.run_checks(Path("/frozen"), environment=self.env, source_commit="a" * 40)
        run.assert_not_called()
        network.assert_not_called()
        other.assert_not_called()


class PureFsmGateTests(unittest.TestCase):
    def setUp(self):
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
        with patch.object(gate.subprocess, "run", side_effect=self.results()) as run, \
             contextlib.redirect_stdout(io.StringIO()) as output:
            gate.run_lifecycle_source_checks(Path("/frozen"), self.env, (77, 88))
        executable = str(self.target / "taira-consensus-fsm-check/lifecycle-source-tests")
        self.assertEqual(run.call_args_list[0].args[0], ["/pinned/rustc", "--edition=2024", "--test",
            "/frozen/crates/iroha_core/src/sumeragi/v2_lifecycle_source_contract_harness.rs",
            "-o", executable])
        for call in run.call_args_list:
            self.assertEqual(call.kwargs["pass_fds"], (77, 88))
            self.assertEqual(call.kwargs["env"], self.env)
            self.assertEqual(call.kwargs["cwd"], "/")
        self.assertIn("lifecycle source contracts PASS: 2 listed, 2 passed, 0 ignored", output.getvalue())
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_lifecycle_failure_stops_before_any_cargo_or_network_work(self):
        env = self.env | {"CARGO": "/pinned/cargo", "CARGO_HOME": "/isolated"}
        with patch.object(gate, "run_pure_fsm_checks") as fsm, \
             patch.object(gate, "run_lifecycle_source_checks", side_effect=gate.CheckError("source contract failed")) as source, \
             patch.object(gate, "compile_library_harnesses") as libraries, \
             patch.object(gate, "compile_harness") as compile, \
             patch.object(gate, "run_network_checks") as network, \
             contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "source contract failed"):
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
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
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
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
