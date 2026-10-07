"""Mutation tests for complete compiler-produced oracle harness admission."""
from copy import deepcopy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("oracle", ROOT / "ci/native_prover_oracle.py")
ORACLE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ORACLE)
TARGET = ROOT / "target/qualification/oracle-admission-test"


def messages():
    return [{"reason": "compiler-artifact", "target": {"name": name},
             "profile": {"test": True}, "executable": str(TARGET / name)}
            for name in ORACLE.HARNESSES] + [{"reason": "build-finished", "success": True}]


def encoded(value):
    return "\n".join(json.dumps(message) for message in value)


class OracleAdmissionTests(unittest.TestCase):
    def test_shipping_rejection_requires_actual_const_failure(self):
        compiler_error = {
            "reason": "compiler-message", "target": {"name": "kaigi_zk"},
            "message": {"level": "error", "code": {"code": "E0080"},
                        "message": "evaluation panicked: iroha_plonk_oracle is test-only",
                        "spans": [{"is_primary": True, "file_name": "crates/kaigi_zk/src/lib.rs",
                                   "text": [{"text": 'const _: () = assert!(!iroha_plonk::ORACLE_BUILD, "iroha_plonk_oracle is test-only");'}]}]},
        }
        baseline = [compiler_error, {"reason": "build-finished", "success": False}]
        self.assertTrue(ORACLE.shipping_rejection(encoded(baseline), 101))
        self.assertFalse(ORACLE.shipping_rejection(encoded(baseline), 0))
        for mutation in ("no_error", "other_crate", "other_error", "missing_code", "other_message",
                         "foreign_file", "other_expression", "not_primary", "success", "unfinished", "duplicate_error"):
            value = deepcopy(baseline)
            if mutation == "no_error": value.pop(0)
            elif mutation == "other_crate": value[0]["target"]["name"] = "iroha_plonk"
            elif mutation == "other_error": value[0]["message"]["code"]["code"] = "E0463"
            elif mutation == "missing_code": value[0]["message"]["code"] = None
            elif mutation == "other_message": value[0]["message"]["message"] = "unrelated constant failure"
            elif mutation == "foreign_file": value[0]["message"]["spans"][0]["file_name"] = "other.rs"
            elif mutation == "other_expression": value[0]["message"]["spans"][0]["text"] = []
            elif mutation == "not_primary": value[0]["message"]["spans"][0]["is_primary"] = False
            elif mutation == "success": value[-1]["success"] = True
            elif mutation == "unfinished": value.pop()
            else: value.insert(0, deepcopy(compiler_error))
            with self.subTest(mutation=mutation):
                self.assertFalse(ORACLE.shipping_rejection(encoded(value), 101))
        # Rust reports the primary span in core::panic and nests the consumer location in
        # the macro invocation chain. A dependency definition site is not a consumer guard.
        expanded = deepcopy(baseline)
        call = expanded[0]["message"]["spans"][0]
        call["is_primary"] = False
        expanded[0]["message"]["spans"] = [{
            "is_primary": True, "file_name": "/rustc/library/core/src/panic.rs",
            "expansion": {"span": call},
        }]
        self.assertTrue(ORACLE.shipping_rejection(encoded(expanded), 101))
        expanded[0]["message"]["spans"][0]["expansion"] = {"def_site_span": call}
        self.assertFalse(ORACLE.shipping_rejection(encoded(expanded), 101))

    def test_disabled_oracle_mode_or_missing_large_cases_fail(self):
        for name, required in ORACLE.REQUIRED_TESTS.items():
            listing = "\n".join(f"{test}: test" for test in required) + f"\n{ORACLE.TIMING_TEST}: test\ntiming::parser: test\n"
            self.assertEqual(ORACLE.test_inventory(name, listing), required | {"timing::parser"})
            for absent in required:
                with self.subTest(name=name, absent=absent), self.assertRaises(ValueError):
                    ORACLE.test_inventory(name, listing.replace(f"{absent}: test\n", ""))

    def test_partial_empty_ignored_or_repeated_results_fail(self):
        self.assertTrue(ORACLE.complete_result("test result: ok. 45 passed; 0 failed; 0 ignored;", 45))
        for result, expected in [
            ("test result: ok. 0 passed; 0 failed; 0 ignored;", 0),
            ("test result: ok. 37 passed; 0 failed; 8 ignored;", 45),
            ("test result: ok. 37 passed; 0 failed; 0 ignored;", 45),
            ("test result: FAILED. 44 passed; 1 failed; 0 ignored;", 45),
            ("test result: ok. 45 passed; 0 failed; 0 ignored;\n" * 2, 45),
        ]:
            with self.subTest(result=result):
                self.assertFalse(ORACLE.complete_result(result, expected))

    def test_complete_compiler_harnesses_and_correctness_only_execution(self):
        found = ORACLE.compiler_harnesses(encoded(messages()), TARGET)
        self.assertEqual(set(found), set(ORACLE.HARNESSES))
        for path in found.values():
            self.assertEqual(ORACLE.test_command(path), [str(path), "--include-ignored", "--skip", ORACLE.TIMING_TEST, "--test-threads=2"])

    def test_missing_or_duplicate_target_is_not_partial_success(self):
        baseline = messages()
        for index in range(len(ORACLE.HARNESSES)):
            for mutation in (baseline[:index] + baseline[index + 1:], [baseline[index], *baseline]):
                with self.subTest(index=index), self.assertRaises(ValueError):
                    ORACLE.compiler_harnesses(encoded(mutation), TARGET)

    def test_benchmark_non_test_foreign_path_and_failed_build_are_rejected(self):
        for kind in ("benchmark", "non_test", "foreign", "relative", "failed", "unfinished", "duplicate_finish"):
            value = deepcopy(messages())
            if kind == "benchmark": value[0]["target"]["name"] = "kernel_benchmarks"
            elif kind == "non_test": value[0]["profile"]["test"] = False
            elif kind == "foreign": value[0]["executable"] = str(ROOT / "unrelated-binary")
            elif kind == "relative": value[0]["executable"] = "relative-binary"
            elif kind == "failed": value[-1]["success"] = False
            elif kind == "unfinished": value.pop()
            else: value.append(value[-1])
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                ORACLE.compiler_harnesses(encoded(value), TARGET)


if __name__ == "__main__":
    unittest.main()
