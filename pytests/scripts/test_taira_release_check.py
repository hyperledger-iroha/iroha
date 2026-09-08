"""Offline checks for the early-gate runner; no Cargo or live inputs required."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
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
        command = gate.compile_command(Path("/repo"), {"CARGO": "/fixed/cargo"}, torii=True)
        self.assertIn("iroha_torii", command)
        self.assertIn("taira_app_contracts", command)
        self.assertNotIn("--lib", command)
        self.assertNotIn("--no-default-features", command)
        event = {"reason": "compiler-artifact", "target": {"name": "taira_app_contracts", "kind": ["test"]},
                 "profile": {"test": True}, "executable": "/warm/taira-contracts"}
        self.assertEqual(gate.test_artifact(json.dumps(event), torii=True), event["executable"])
        self.assertIsNone(gate.test_artifact(json.dumps(event)))
        for changes in ({"profile": {"test": False}}, {"executable": None},
                        {"target": {"name": "torii_core_routes", "kind": ["test"]}},
                        {"target": {"name": "taira_app_contracts", "kind": ["lib"]}}):
            self.assertIsNone(gate.test_artifact(json.dumps(event | changes), torii=True))

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
             patch.object(gate, "TORII_STAGES", ()), contextlib.redirect_stdout(io.StringIO()):
            gate.run_checks(Path("/frozen"), environment={"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}, source_commit="a" * 40, lock_fds=(77, 88))
        self.assertEqual([call.kwargs["cwd"] for call in run.call_args_list], [Path("/warm"), Path("/warm")])
        self.assertNotIn("frozen", compile.call_args.kwargs)
        self.assertEqual(compile.call_args.args[1]["VERGEN_GIT_SHA"], "a" * 40)


    def test_mutable_check_keeps_git_checks_and_inherits_lane_lock_in_every_child(self):
        env = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/routine"}
        results = [subprocess.CompletedProcess([], 0, "fixture: test\n", ""),
                   subprocess.CompletedProcess([], 0, "test fixture ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n", "")]
        with patch.object(gate.subprocess, "check_output", return_value="a" * 40) as git, \
             patch.object(gate, "compile_harness", return_value="/fixture/harness") as compile, \
             patch.object(gate.subprocess, "run", side_effect=results) as run, \
             patch.object(gate, "STAGES", (("fixtures", ("fixture",)),)), \
             patch.object(gate, "TORII_STAGES", ()), contextlib.redirect_stdout(io.StringIO()):
            gate.run_checks(Path("/mutable"), environment=env, lock_fds=(77,))
        self.assertEqual(git.call_count, 2)
        self.assertTrue(all(call.kwargs["env"]["CARGO_HOME"] == "/isolated" for call in git.call_args_list))
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,)})
        self.assertTrue(all(call.kwargs["pass_fds"] == (77,) for call in run.call_args_list))
        self.assertTrue(all(call.kwargs["cwd"] == Path("/mutable") for call in run.call_args_list))

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
             patch.object(gate, "TORII_STAGES", (("Torii", ("route",)),)), \
             contextlib.redirect_stdout(output), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaisesRegex(gate.CheckError, "route.*exit 101"):
                gate.run_checks(Path("/frozen"), environment=env, source_commit="a" * 40, lock_fds=(77,))
        self.assertEqual(compile.call_count, 2)
        self.assertEqual(compile.call_args.kwargs, {"lock_fds": (77,), "torii": True})
        self.assertTrue(all(call.kwargs["cwd"] == Path("/warm") and call.kwargs["pass_fds"] == (77,)
                            for call in run.call_args_list))
        self.assertNotIn("[taira-check] PASS:", output.getvalue())

    def test_unisolated_low_level_check_is_rejected_before_git_or_cargo(self):
        with patch.object(gate.subprocess, "check_output") as git, patch.object(gate, "compile_harness") as compile:
            with self.assertRaisesRegex(gate.CheckError, "isolated Cargo environment"):
                gate.run_checks(Path("/mutable"), environment={})
        git.assert_not_called()
        compile.assert_not_called()


if __name__ == "__main__":
    unittest.main()
