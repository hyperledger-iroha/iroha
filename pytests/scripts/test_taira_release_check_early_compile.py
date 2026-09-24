"""The complete selected test metadata gate rejects Rust errors before codegen.

These tests use mocked Cargo and never access a validator or deployment input.
"""

import contextlib
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import MagicMock, patch


SCRIPT = Path(__file__).resolve().parents[2] / "scripts/taira_release_check.py"
SPEC = importlib.util.spec_from_file_location("taira_release_check_early_compile", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class EarlyCompileGateTests(unittest.TestCase):
    ENV = {"CARGO": "/fixed/cargo", "CARGO_HOME": "/isolated", "CARGO_TARGET_DIR": "/warm"}
    SHIPPING = ("cli", "kagami", "taira-launcher", "sorafs-bin")

    def run_until_codegen(self, scope, metadata_effect, codegen_effect):
        with contextlib.ExitStack() as stack:
            for name in ("require_native_artifact_inspector", "require_network_fixture_prerequisites",
                         "run_pure_fsm_checks", "run_lifecycle_source_checks"):
                stack.enter_context(patch.object(gate, name))
            stack.enter_context(patch.object(gate, "shipping_harnesses", return_value=self.SHIPPING))
            metadata = stack.enter_context(patch.object(
                gate, "check_test_harnesses", side_effect=metadata_effect))
            codegen = stack.enter_context(patch.object(
                gate, "compile_test_harnesses", side_effect=codegen_effect))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            checkpoint = MagicMock()
            with self.assertRaises(gate.CheckError) as caught:
                gate.run_checks(Path("/frozen"), qualification_scope=scope,
                                environment=self.ENV, source_commit="a" * 40,
                                lock_fds=(77, 88), update_independent_checks=checkpoint)
            self.assertEqual(metadata.call_args.args[0], Path("/frozen"))
            expected = gate.native_harness_plan(gate.qualification_stages(scope), self.SHIPPING)[1]
            self.assertTrue({"core", "cli", "network"}.issubset(expected))
            self.assertEqual(metadata.call_args.kwargs,
                             {"harnesses": expected, "lock_fds": (77, 88)})
            self.assertEqual(metadata.call_args.args[1],
                             self.ENV | {"VERGEN_GIT_SHA": "a" * 40,
                                         "IROHA_GIT_COMMIT_HASH": "a" * 40})
            checkpoint.assert_not_called()
            return caught.exception, metadata, codegen

    def test_selected_harness_type_error_stops_complete_codegen_in_both_scopes(self):
        failure = gate.CheckError("selected test Rust metadata rejected E0308")
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                caught, metadata, codegen = self.run_until_codegen(scope, failure, None)
                self.assertIs(caught, failure)
                metadata.assert_called_once()
                codegen.assert_not_called()

    def test_successful_probe_preserves_complete_codegen_graph_and_locks(self):
        failure = gate.CheckError("stop at complete codegen")
        for scope in gate.QUALIFICATION_SCOPES:
            with self.subTest(scope=scope):
                order = []
                def metadata(*_args, **_kwargs):
                    order.append("complete metadata")
                def codegen(*_args, **_kwargs):
                    order.append("complete codegen")
                    raise failure
                caught, checked, compiled = self.run_until_codegen(scope, metadata, codegen)
                self.assertIs(caught, failure)
                self.assertEqual(order, ["complete metadata", "complete codegen"])
                checked.assert_called_once()
                compiled.assert_called_once()
                self.assertEqual(checked.call_args.kwargs, compiled.call_args.kwargs)
                self.assertIs(checked.call_args.args[1], compiled.call_args.args[1])


if __name__ == "__main__":
    unittest.main()
