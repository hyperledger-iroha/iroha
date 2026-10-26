"""Dependency-free adapter/fixture controls; these do not execute the Python SDK.

Run with Python 3.11+ via unittest from the captured repository. The separate
sdk_transaction_workload.py target requires the same-source native extension.
No HTTP, Cargo, native SDK, subprocess workloads or ledger writes occur here.
"""
from __future__ import annotations

import ast
import copy
from contextlib import redirect_stderr
import importlib.util
import io
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
DIAGNOSTICS = ROOT / "python/iroha_python/tests/diagnostics"
sys.path.insert(0, str(DIAGNOSTICS))
import measurement_record as adapter

harness = adapter.harness
FIXTURES = ROOT / "crates/iroha_measurement/fixtures"
GOLDEN = harness.strict_json((FIXTURES / "measurement_record_v1.json").read_bytes())
MUTATIONS = harness.strict_json((FIXTURES / "record_mutations_v1.json").read_bytes())
CONTEXT = {key: GOLDEN["identity"][key] for key in adapter.DESCRIPTOR["fields"]["context"]
           if key != "schema"}
CONTEXT["schema"] = adapter.DESCRIPTOR["context_schema"]


def findings(record):
    decoded = harness.decode_record(adapter.DESCRIPTOR, record)
    return harness.record_findings(adapter.DESCRIPTOR, decoded)


def walk(value, path):
    for key in path:
        value = value[key]
    return value


class SchemaAndAdapterTests(unittest.TestCase):
    def assert_only_outcome(self, record, outcome):
        # Tiny adapter-only controls have real root setup/close overhead. Do not
        # fabricate timing or pretend they qualify the 128-call SDK workload.
        observed = [f for f in findings(record) if not f.startswith("unattributed_exceeds_limit:")]
        expected = [] if outcome == "succeeded" else ["run_not_succeeded:" + outcome]
        self.assertEqual(observed, expected)
        self.assertEqual(record["outcome"], outcome)
        self.assertEqual(harness.unclassified_numbers(adapter.DESCRIPTOR, record), [])

    def test_original_golden_and_entire_mutation_corpus(self):
        self.assertEqual(findings(copy.deepcopy(GOLDEN)), [])
        self.assertEqual(MUTATIONS["base"], "measurement_record_v1.json")
        self.assertGreaterEqual(len(MUTATIONS["cases"]), 50)
        for case in MUTATIONS["cases"]:
            with self.subTest(case=case["name"]):
                record = copy.deepcopy(GOLDEN)
                for operation in case.get("set", []):
                    *parents, last = operation["path"]
                    walk(record, parents)[last] = copy.deepcopy(operation["value"])
                for operation in case.get("append", []):
                    walk(record, operation["path"]).append(copy.deepcopy(operation["value"]))
                for path in case.get("remove", []):
                    *parents, last = path
                    del walk(record, parents)[last]
                if case.get("decode_error"):
                    with self.assertRaises(harness.RecordDecodeError):
                        harness.decode_record(adapter.DESCRIPTOR, record)
                else:
                    self.assertEqual(findings(record), case["findings"])

    def test_exact_return_object_and_public_counters_without_allocation_claim(self):
        expected = object()
        records = []
        def operation(session):
            for size in [7, 11, 9]:
                with session.phase("canonical_frame"):
                    session.bytes("signed_transaction_versioned", size)
                    session.work("canonical_frames", 1)
            return expected
        returned = adapter.observe(CONTEXT, "python.sdk.retry", operation, records.append)
        self.assertIs(returned, expected)
        self.assertEqual(len(records), 1)
        record = records[0]
        self.assert_only_outcome(record, "succeeded")
        counter = record["byte_counters"]["entries"][0]
        self.assertEqual((counter["count"], counter["min_bytes"], counter["max_bytes"], counter["total_bytes"]),
                         (3, 7, 11, 27))
        self.assertEqual(record["work_counters"]["entries"][0]["total_units"], 3)
        self.assertEqual(record["allocations"]["sources"], [])
        for node in record["phase_tree"]["nodes"]:
            self.assertEqual((node["allocations"], node["allocated_bytes"], node["live_bytes_high_water"]), (0, 0, 0))
        self.assertEqual(record["process"]["thermal_finish"], "unavailable")

    def test_exact_expected_exception_and_retry_have_separate_immutable_records(self):
        sentinel = ValueError("private text never recorded")
        records = []
        def operation(session):
            with session.phase("sign"):
                raise sentinel
        try:
            adapter.observe(CONTEXT, "python.sdk.signing_failure", operation, records.append,
                            expected_error=ValueError)
        except ValueError as returned:
            self.assertIs(returned, sentinel)
        else:
            self.fail("operation exception disappeared")
        failure = harness.control.canonical(records[0])
        self.assertNotIn(b"private text", failure)
        self.assert_only_outcome(records[0], "failed")
        self.assertEqual(records[0]["phase_tree"]["nodes"][0]["completed"], 1)
        self.assertEqual(records[0]["phase_tree"]["nodes"][-1]["interrupted"], 1)
        self.assertEqual(records[0]["phase_tree"]["nodes"][-1]["unwound"], 0)
        value = object()
        self.assertIs(adapter.observe(CONTEXT, "python.sdk.retry", lambda s: value, records.append), value)
        self.assertEqual(harness.control.canonical(records[0]), failure)
        self.assert_only_outcome(records[1], "succeeded")

    def test_base_exception_unwinds_and_sink_failure_never_replaces_it(self):
        sentinel = KeyboardInterrupt("private")
        records = []
        def operation(session):
            with session.phase("exception_boundary"):
                raise sentinel
        def sink(record):
            records.append(record)
            raise SystemExit("diagnostic sink interrupt")
        try:
            adapter.observe(CONTEXT, "python.sdk.exception", operation, sink)
        except KeyboardInterrupt as caught:
            self.assertIs(caught, sentinel)
        else:
            self.fail("original BaseException disappeared")
        self.assert_only_outcome(records[0], "unwound")
        self.assertTrue(all(n["unwound"] == 1 for n in records[0]["phase_tree"]["nodes"]))
        result = object()
        self.assertIs(adapter.observe(CONTEXT, "python.sdk.retry", lambda s: result, sink), result)

    def test_invalid_labels_and_counters_fail_observation_without_changing_result(self):
        records = []
        result = object()
        def operation(session):
            with session.phase("not_public_secret"):
                session.bytes("private_secret", 42)
                session.work("canonical_frames", True)
                session.bytes("signed_transaction_versioned", 1 << 64)
            return result
        self.assertIs(adapter.observe(CONTEXT, "python.sdk.retry", operation, records.append), result)
        self.assert_only_outcome(records[0], "failed")
        self.assertNotIn(b"secret", harness.control.canonical(records[0]))
        self.assertEqual(records[0]["failures"]["entries"][0]["code"], "observation_error")

    def test_observation_setup_fault_never_skips_or_replaces_operation(self):
        records, calls = [], []
        value = object()
        def operation(session):
            calls.append(1)
            with session.phase("sign"):
                session.bytes("signed_transaction_versioned", 7)
                session.work("signed_transactions", 1)
            return value
        with mock.patch.object(adapter, "_usage", side_effect=SystemExit("private probe fault")):
            self.assertIs(adapter.observe(CONTEXT, "python.sdk.retry", operation, records.append), value)
        self.assertEqual(calls, [1])
        self.assertEqual(records, [])  # Missing evidence must fail qualification.

    def test_serial_nested_repeated_phase_cpu_and_wall_accounting(self):
        records = []
        samples = iter([(i, i * 2, i * 3) for i in range(0, 1200, 100)])
        def operation(session):
            for _ in range(2):
                with session.phase("prepare"):
                    with session.phase("canonical_frame"):
                        pass
        with mock.patch.object(adapter, "_clocks", side_effect=lambda: next(samples)):
            adapter.observe(CONTEXT, "python.sdk.retry", operation, records.append)
        self.assert_only_outcome(records[0], "succeeded")
        nodes = records[0]["phase_tree"]["nodes"]
        self.assertEqual([n["calls"] for n in nodes], [1, 1, 2, 2])
        self.assertEqual([n["wall_inclusive_ns"] for n in nodes], [1100, 900, 600, 200])
        self.assertEqual([n["wall_exclusive_ns"] for n in nodes], [200, 300, 400, 200])
        self.assertEqual([n["process_cpu_window_inclusive_ns"] for n in nodes], [3300, 2700, 1800, 600])

    def test_clock_regression_is_explicitly_rejected(self):
        records = []
        ticks = iter([(100, 100, 100), (200, 200, 200), (150, 150, 150), (300, 300, 300)])
        with mock.patch.object(adapter, "_clocks", side_effect=lambda: next(ticks)):
            adapter.observe(CONTEXT, "python.sdk.retry", lambda s: None, records.append)
        self.assertIn("clock_anomaly", findings(records[0]))

    def test_context_original_codec_and_exclusive_public_json_delivery(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            context = root / "context.json"
            context.write_bytes(harness.control.canonical(CONTEXT))
            self.assertEqual(adapter.read_context(root), CONTEXT)
            sink = adapter.directory_sink(root, "sdk-python-success")
            result = object()
            self.assertIs(adapter.observe(CONTEXT, "python.sdk.retry", lambda s: result, sink), result)
            sink.check()
            path = root / "sdk-python-success.json"
            original = path.read_bytes()
            self.assert_only_outcome(harness.decode_record(adapter.DESCRIPTOR, harness.strict_json(original)), "succeeded")
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertIs(adapter.observe(CONTEXT, "python.sdk.retry", lambda s: result, sink), result)
            with self.assertRaises(RuntimeError):
                sink.check()
            self.assertEqual(path.read_bytes(), original)
            self.assertEqual({p.name for p in root.iterdir()}, {"context.json", "sdk-python-success.json"})

    def test_context_duplicate_keys_symlink_and_shared_directory_are_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            path = root / "context.json"
            path.write_bytes(b'{"schema":"a","schema":"b"}')
            with self.assertRaises(harness.RecordDecodeError):
                adapter.read_context(root)
            path.unlink()
            target = root / "other.json"
            target.write_bytes(harness.control.canonical(CONTEXT))
            path.symlink_to(target)
            with self.assertRaises(OSError):
                adapter.read_context(root)
            path.unlink()
            os.mkfifo(path)
            with self.assertRaises(ValueError):
                adapter.read_context(root)
            path.unlink()
            path.write_bytes(harness.control.canonical(CONTEXT))
            os.chmod(root, 0o755)
            with self.assertRaises(ValueError):
                adapter.read_context(root)

    def test_sdk_imports_are_deferred_and_original_apis_not_replaced(self):
        source = (DIAGNOSTICS / "sdk_transaction_workload.py").read_text()
        tree = ast.parse(source)
        for node in tree.body:
            if isinstance(node, ast.ImportFrom):
                self.assertFalse((node.module or "").startswith(("iroha_python", "iroha_native")))
        self.assertNotIn("unittest.mock", source)
        self.assertNotIn("monkeypatch", source)
        self.assertNotIn("sign_and_submit", source)
        for method in ("to_builder", "payload_json", "sign", "signed_transaction_versioned",
                       "signed_transaction_envelope_from_versioned_v1", "signed_transaction_envelope_from_json"):
            self.assertIn(method, source)

    def test_cli_missing_failure_record_cannot_succeed_with_only_retry_record(self):
        # CLI wiring only: inject delivery faults without importing/calling any
        # SDK function, signing implementation, envelope or native artifact.
        import sdk_transaction_workload as workload
        def control(context, failed_sink, retry_sink):
            failed_sink.errors += 1
            retry_sink(copy.deepcopy(GOLDEN))
        for mode, function in [("signing_failure", "run_signing_failure"),
                               ("exception", "run_exception")]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                (root / "context.json").write_bytes(harness.control.canonical(CONTEXT))
                stderr = io.StringIO()
                with mock.patch.dict(os.environ, {"IROHA_MEASUREMENT_OUTPUT_DIR": str(root)}), \
                     mock.patch.object(workload, function, side_effect=control), \
                     redirect_stderr(stderr):
                    self.assertEqual(workload.main([mode]), 1)
                self.assertTrue((root / "sdk-python-retry.json").is_file())
                self.assertEqual(stderr.getvalue(), "sdk diagnostic failed\n")
        self.assertFalse(any(name == "iroha_python" or name.startswith("iroha_native")
                             for name in sys.modules))


if __name__ == "__main__":
    unittest.main()
