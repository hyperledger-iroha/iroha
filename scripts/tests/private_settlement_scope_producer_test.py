"""Exercise durable scope production with synthetic records and no real processes."""

from __future__ import annotations

import copy
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

TESTS = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("scope_producer_fixtures", TESTS / "private_settlement_release_runner_test.py")
assert SPEC is not None and SPEC.loader is not None
FIXTURES = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = FIXTURES
SPEC.loader.exec_module(FIXTURES)
RUNNER = FIXTURES.MODULE


class ScopeProducerTests(unittest.TestCase):
    """Check policy freezing, named scope integrity, and authoritative closure."""

    def test_deadline_policy_is_exact_and_never_coerces_input(self) -> None:
        policy = RUNNER.benchmark_deadline_policy(7200)
        self.assertEqual(policy["outer_timeout_ms"], 7_200_000)
        self.assertEqual(set(policy["rust_deadline_budgets_ms"]), RUNNER.attempt_accounting.DEADLINE_STAGES)
        self.assertEqual(set(policy["rust_deadline_budgets_ms"].values()), {300_000})
        for invalid in (True, 1.0, 0, -1, 1 << 64):
            with self.subTest(invalid=invalid), self.assertRaises(RUNNER.RunnerError):
                RUNNER.benchmark_deadline_policy(invalid)

    def test_scope_registration_freezes_all_names_and_exact_plan_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source"
            source.mkdir()
            plan = {"commit": "a" * 40, "hardware": {"sha256": "b" * 64},
                    "benchmark_accounting": RUNNER.benchmark_deadline_policy(7200)}
            plan_path = root / "plan.json"
            RUNNER.private_record(plan_path, plan)
            scope_path = root / "scope.json"
            with mock.patch.object(RUNNER, "load_plan", return_value=(plan, root)):
                RUNNER.register_benchmark_scope(scope_path, source_root=source,
                    campaign_plans={"second": plan_path, "first": plan_path})
                with self.assertRaises(FileExistsError):
                    RUNNER.register_benchmark_scope(scope_path, source_root=source, campaign_plans={"first": plan_path})
            scope, binding = RUNNER.load_benchmark_scope(scope_path, campaign_id="second",
                plan_binding=RUNNER.file_binding(plan_path), deadline_policy=plan["benchmark_accounting"])
            self.assertEqual([item["campaign_id"] for item in scope["campaigns"]], ["first", "second"])
            self.assertEqual(binding, RUNNER.file_binding(scope_path))
            self.assertIsNone(scope["previous_scope_sha256"])
            for name, value in (("wrong", RUNNER.file_binding(plan_path)),
                                ("second", {"sha256": "f" * 64, "bytes": 1})):
                with self.assertRaises(RUNNER.RunnerError):
                    RUNNER.load_benchmark_scope(scope_path, campaign_id=name, plan_binding=value,
                                               deadline_policy=plan["benchmark_accounting"])

    def test_scope_registration_rejects_mixed_policy_and_malformed_names(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            source = root / "source"
            source.mkdir()
            one = {"commit": "a" * 40, "hardware": {}, "benchmark_accounting": RUNNER.benchmark_deadline_policy(7200)}
            two = {**one, "benchmark_accounting": RUNNER.benchmark_deadline_policy(7201)}
            paths = [root / "one.json", root / "two.json"]
            for path, value in zip(paths, (one, two)):
                RUNNER.private_record(path, value)
            with mock.patch.object(RUNNER, "load_plan", side_effect=[(one, root), (two, root)]), self.assertRaisesRegex(
                    RUNNER.RunnerError, "share source"):
                RUNNER.register_benchmark_scope(root / "scope.json", source_root=source,
                                               campaign_plans={"one": paths[0], "two": paths[1]})
            self.assertFalse((root / "scope.json").exists())
            for name in ("../outside", "Capital", "", "a" * 65):
                with self.subTest(name=name), self.assertRaises(RUNNER.RunnerError):
                    RUNNER.register_benchmark_scope(root / "scope.json", source_root=source,
                                                   campaign_plans={name: paths[0]})

    def closure_fixture(self, root: Path):
        """Produce one bound synthetic start followed by a second planned job."""
        jobs = [{"request_id": "1" * 64, "kind": "benchmark"}, {"request_id": "2" * 64, "kind": "benchmark"}]
        identity = {"scope_sha256": "a" * 64, "campaign_id": "first", "plan_sha256": "b" * 64}
        start = {**identity, "attempt_id": "c" * 64, "request_id": jobs[0]["request_id"], "invocation_nonce": "d" * 64}
        (root / "attempts").mkdir()
        attempt = root / "attempts" / ("00001-" + jobs[0]["request_id"])
        attempt.mkdir()
        RUNNER.private_record(attempt / "started.json", start)
        RUNNER.private_record(attempt / "process-outcome.json", {**start, "pid": 654321, "owned_process_group_gone": True})
        return {"jobs": jobs}, identity, attempt

    def test_campaign_closure_binds_actual_durable_starts_without_fabricating_success(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            plan, identity, _ = self.closure_fixture(root)
            with mock.patch.object(RUNNER, "_process_group_exists", return_value=False):
                path = RUNNER.close_benchmark_campaign(root, plan=plan, reason="fail_fast", **identity)
            closure = json.loads(path.read_text())
            self.assertEqual(closure["started_request_ids"], [plan["jobs"][0]["request_id"]])
            self.assertEqual(closure["reason"], "fail_fast")
            self.assertNotIn("succeeded", closure)
            with mock.patch.object(RUNNER, "_process_group_exists", return_value=False), self.assertRaises(FileExistsError):
                RUNNER.close_benchmark_campaign(root, plan=plan, reason="fail_fast", **identity)

    def test_closure_refuses_missing_terminal_live_group_and_substituted_identity(self) -> None:
        for mutation in ("missing", "live", "identity", "extra", "completed"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                plan, identity, attempt = self.closure_fixture(root)
                path = attempt / "process-outcome.json"
                if mutation == "missing":
                    path.unlink()
                elif mutation == "identity":
                    value = json.loads(path.read_text()); value["attempt_id"] = "e" * 64
                    path.write_text(json.dumps(value))
                elif mutation == "extra":
                    (root / "attempts" / "undeclared").mkdir()
                with mock.patch.object(RUNNER, "_process_group_exists", return_value=mutation == "live"), self.assertRaises(RUNNER.RunnerError):
                    RUNNER.close_benchmark_campaign(root, plan=plan,
                        reason="completed" if mutation == "completed" else "fail_fast", **identity)
                self.assertFalse((root / "campaign-closure.json").exists())

    def test_benchmark_invocation_requires_scope_before_creating_attempt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with mock.patch.object(RUNNER.subprocess, "Popen") as spawn, self.assertRaisesRegex(RUNNER.RunnerError, "registered"):
                RUNNER.invoke_harness(root / "harness", {"kind": "benchmark"}, attempt_dir=root / "attempt", timeout_seconds=1)
            spawn.assert_not_called()
            self.assertFalse((root / "attempt").exists())

    def test_spawn_failure_retains_typed_completion_and_quiescence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            identity = {"scope_sha256": "a" * 64, "campaign_id": "test", "plan_sha256": "b" * 64, "attempt_id": RUNNER.attempt_accounting.registered_attempt_id("a" * 64, "test", "b" * 64, "d" * 64)}
            with mock.patch.object(RUNNER.subprocess, "Popen", side_effect=OSError("synthetic spawn failure")), self.assertRaises(RUNNER.RunnerError):
                RUNNER.invoke_harness(root / "harness", {"kind": "benchmark", "request_id": "d" * 64, "invocation_nonce": "e" * 64},
                    attempt_dir=root / "attempt", timeout_seconds=1, accounting_identity=identity)
            outcome = json.loads((root / "attempt" / "process-outcome.json").read_text())
            self.assertEqual(outcome["completion_kind"], "spawn_failed")
            self.assertIsNone(outcome["pid"])
            self.assertTrue(outcome["owned_process_group_gone"])
            self.assertTrue(outcome["bindings_unchanged"])
            self.assertFalse(outcome["passed"])
            self.assertEqual(outcome["attempt_id"], identity["attempt_id"])

    def test_later_failure_retains_previous_sample_and_exact_acceptance_binding(self) -> None:
        owner = FIXTURES.PrivateSettlementFailureRetentionTests()
        with tempfile.TemporaryDirectory() as temporary, owner.execution_fixture(Path(temporary).resolve()) as fixture:
            fixture["leakage"].side_effect = RUNNER.RunnerError("synthetic later audit failure")
            with self.assertRaisesRegex(RUNNER.RunnerError, "later audit failure"):
                fixture["execute"]()
            attempts = sorted((fixture["output"] / "attempts").iterdir())
            sample = attempts[1] / "benchmark-sample.json"
            validation = json.loads((attempts[1] / "validation-outcome.json").read_text())
            self.assertEqual(validation["validation_kind"], "accepted")
            self.assertEqual(validation["sample"], RUNNER.file_binding(sample))
            self.assertEqual(json.loads(sample.read_text())["attempt_id"], validation["attempt_id"])
            closure = json.loads((fixture["output"] / "campaign-closure.json").read_text())
            self.assertEqual(len(closure["started_request_ids"]), 4)
            self.assertFalse((fixture["output"] / "release-artifact-fragment-v1.json").exists())

    def test_unused_campaign_closure_is_exclusive_and_retains_original_plan_inputs(self) -> None:
        owner = FIXTURES.PrivateSettlementFailureRetentionTests()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with owner.execution_fixture(root) as fixture:
                output = RUNNER.close_unstarted_campaign(root / "scope.json", campaign_id="test",
                    plan_path=root / "plan.json", source_root=root / "source")
                closure = json.loads(output.read_text())
                self.assertEqual(closure["reason"], "not_run")
                self.assertEqual(closure["started_request_ids"], [])
                self.assertFalse((output.parent / "attempts").exists())
                self.assertEqual((output.parent / "frozen-plan.json").read_bytes(), (root / "plan.json").read_bytes())
                for key in ("hardware", "canary_manifest", "configuration_manifest"):
                    relative = fixture["plan"][key]["path"]
                    self.assertEqual((output.parent / relative).read_bytes(), (root / relative).read_bytes())
                with self.assertRaises(FileExistsError):
                    RUNNER.close_unstarted_campaign(root / "scope.json", campaign_id="test",
                        plan_path=root / "plan.json", source_root=root / "source")
                fixture["process"].assert_not_called()

    def test_frozen_input_binding_failure_preserves_unclosed_claim_without_launching(self) -> None:
        owner = FIXTURES.PrivateSettlementFailureRetentionTests()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with owner.execution_fixture(root) as fixture:
                (root / fixture["plan"]["hardware"]["path"]).write_bytes(b"changed input")
                with self.assertRaises(RUNNER.RunnerError):
                    RUNNER.close_unstarted_campaign(root / "scope.json", campaign_id="test",
                        plan_path=root / "plan.json", source_root=root / "source")
                self.assertTrue(fixture["output"].is_dir())
                self.assertFalse((fixture["output"] / "campaign-closure.json").exists())
                fixture["process"].assert_not_called()


class ScopeCollectionTests(unittest.TestCase):
    """Read authentic byte fixtures through the filesystem-to-reducer boundary."""

    def write_fixture(self, root: Path):
        """Materialize the pure reducer's clearly synthetic protocol records."""
        spec = importlib.util.spec_from_file_location("collection_fixtures", TESTS / "private_settlement_campaign_accounting_test.py")
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        scope, campaigns, samples = module.fixture([["succeeded", "failed", "not_started"], ["timed_out"]])
        (root / "scope.json").write_bytes(scope)
        names = {"request": "request.json", "started": "started.json", "process": "process-outcome.json",
                 "adapter": "evidence/benchmark-protocol/adapter-outcome.json", "rust_terminal": "evidence/benchmark-protocol/rust-result.json",
                 "response": "response.json", "response_outcome": "response-outcome.json", "validation": "validation-outcome.json", "sample": "benchmark-sample.json"}
        for campaign in campaigns:
            directory = root / "campaigns" / campaign["campaign_id"]
            directory.mkdir(parents=True)
            for name, raw in (("registered-scope.json", scope), ("frozen-plan.json", campaign["plan"]), ("campaign-closure.json", campaign["closure"])):
                (directory / name).write_bytes(raw)
            for ordinal, packet in enumerate(campaign["attempts"], 1):
                for key, name in names.items():
                    if packet[key] is not None:
                        destination = directory / "attempts" / f"{ordinal:05}-{packet['request_id']}" / name
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        destination.write_bytes(packet[key])
        return scope, campaigns, samples

    def test_complete_scope_reduction_preserves_failed_predecessors_and_successes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            expected = self.write_fixture(root)
            collected = RUNNER.collect_benchmark_scope(root / "scope.json")
            self.assertEqual(collected, expected)
            source = root / "source"; source.mkdir()
            output = RUNNER.write_benchmark_scope_accounting(root / "scope.json", root / "accounting.json", source_root=source)
            result = json.loads(output.read_text())
            self.assertEqual(result["counts"], {"planned": 4, "attempted": 3, "not_started": 1,
                                              "succeeded": 1, "failed": 1, "timed_out": 1, "incomplete": 0})
            self.assertTrue(result["accounting_complete"])
            self.assertNotIn("release_qualified", result)
            with self.assertRaises(FileExistsError):
                RUNNER.write_benchmark_scope_accounting(root / "scope.json", output, source_root=source)

    def test_collection_rejects_omitted_closure_changed_scope_and_symlink_records(self) -> None:
        for mutation in ("closure", "scope", "symlink", "extra"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                self.write_fixture(root)
                campaign = root / "campaigns" / "campaign-0"
                if mutation == "closure":
                    (campaign / "campaign-closure.json").unlink()
                elif mutation == "scope":
                    (campaign / "registered-scope.json").write_bytes(b"{}")
                elif mutation == "symlink":
                    path = campaign / "campaign-closure.json"; raw = path.read_bytes(); path.unlink()
                    (root / "outside.json").write_bytes(raw); path.symlink_to(root / "outside.json")
                else:
                    (campaign / "attempts" / "undeclared").mkdir()
                with self.assertRaises((OSError, RUNNER.RunnerError)):
                    RUNNER.collect_benchmark_scope(root / "scope.json")

    def test_collection_rejects_changed_earlier_record_and_late_terminal_appearance(self) -> None:
        for mutation in ("changed", "appeared", "extra_protocol"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                _, campaigns, _ = self.write_fixture(root)
                first = root / "campaigns" / "campaign-0" / "attempts"
                original = RUNNER.retained_accounting_bytes
                fired = False
                def read(path, **kwargs):
                    nonlocal fired
                    if path == root / "campaigns" / "campaign-1" / "frozen-plan.json" and not fired:
                        fired = True
                        if mutation == "changed":
                            target = first / ("00001-" + campaigns[0]["attempts"][0]["request_id"]) / "request.json"
                            target.write_bytes(target.read_bytes() + b" ")
                        elif mutation == "appeared":
                            target = first / ("00003-" + campaigns[0]["attempts"][2]["request_id"]) / "evidence/benchmark-protocol/rust-result.json"
                            target.parent.mkdir(parents=True, exist_ok=True); target.write_bytes(b"{}")
                        else:
                            target = first / ("00001-" + campaigns[0]["attempts"][0]["request_id"]) / "evidence/benchmark-protocol/undeclared.json"
                            target.write_bytes(b"{}")
                    return original(path, **kwargs)
                with mock.patch.object(RUNNER, "retained_accounting_bytes", side_effect=read), self.assertRaises(RUNNER.RunnerError):
                    RUNNER.collect_benchmark_scope(root / "scope.json")
                self.assertTrue(fired)

    def test_full_plan_fault_start_requires_exact_quiescent_process_closure(self) -> None:
        spec = importlib.util.spec_from_file_location("prefix_fixtures", TESTS / "private_settlement_campaign_accounting_test.py")
        assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        for mutation in (None, "missing", "identity", "live"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                scope, campaigns, _ = module.fixture([["not_started"]], fault_prefix=True)
                campaign = campaigns[0]; plan = json.loads(campaign["plan"]); job = plan["jobs"][0]
                (root / "scope.json").write_bytes(scope)
                directory = root / "campaigns" / "campaign-0"; directory.mkdir(parents=True)
                for name, raw in (("registered-scope.json", scope), ("frozen-plan.json", campaign["plan"]), ("campaign-closure.json", campaign["closure"])):
                    (directory / name).write_bytes(raw)
                attempt = directory / "attempts" / ("00001-" + job["request_id"]); attempt.mkdir(parents=True)
                identity = {"scope_sha256": module.binding(scope)["sha256"], "campaign_id": "campaign-0",
                            "plan_sha256": module.binding(campaign["plan"])["sha256"]}
                identity["attempt_id"] = RUNNER.attempt_accounting.registered_attempt_id(
                    identity["scope_sha256"], "campaign-0", identity["plan_sha256"], job["request_id"])
                identity.update(request_id=job["request_id"], invocation_nonce="a" * 64)
                request = module.raw(job); (attempt / "request.json").write_bytes(request)
                start = {"version": 1, "protocol": RUNNER.PROTOCOL, **identity, "command": ["synthetic"],
                         "request": module.binding(request), "harness": plan["harness"], "timeout_seconds": 3600, "started_ns": 2000}
                (attempt / "started.json").write_bytes(module.raw(start))
                process = {"version": 1, "protocol": RUNNER.PROTOCOL, **identity, "finished_ns": 4000,
                           "pid": 1234, "exit_code": 2, "timed_out": False, "error": "synthetic failure", "passed": False,
                           "retained_files": [], "completion_kind": "exited", "elapsed_ms": 100,
                           "owned_process_group_gone": mutation != "live", "bindings_unchanged": True}
                if mutation == "identity": process["invocation_nonce"] = "b" * 64
                if mutation != "missing": (attempt / "process-outcome.json").write_bytes(module.raw(process))
                if mutation is None:
                    collected = RUNNER.collect_benchmark_scope(root / "scope.json")
                    result = RUNNER.attempt_accounting.reduce_registered_scope(*collected)
                    self.assertEqual(result["counts"]["not_started"], 1)
                else:
                    with self.assertRaises(RUNNER.RunnerError):
                        RUNNER.collect_benchmark_scope(root / "scope.json")

    def test_oversized_accounting_record_is_rejected_before_hashing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve() / "large.json"
            with path.open("wb") as stream:
                stream.truncate(RUNNER.MAX_HARNESS_RESPONSE_BYTES + 1)
            with mock.patch.object(RUNNER, "file_binding") as hashing, self.assertRaisesRegex(RUNNER.RunnerError, "bounded protocol size"):
                RUNNER.retained_accounting_bytes(path)
            hashing.assert_not_called()

    def test_csv_preserves_the_registered_attempt_join(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            _, _, samples = self.write_fixture(root)
            sample = json.loads(samples[0])
            output = root / "raw.csv"
            RUNNER.write_benchmark_csv(output, [sample])
            with output.open() as stream:
                rows = list(RUNNER.csv.DictReader(stream))
            self.assertEqual(rows[0]["attempt_id"], sample["attempt_id"])

    def test_destination_io_is_typed_without_reclassifying_source_binding_errors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); source = root / "source.json"; source.write_bytes(b"{}")
            with mock.patch.object(Path, "mkdir", side_effect=OSError("synthetic destination error")), self.assertRaises(RUNNER.OutputPublicationError):
                RUNNER.copy_bound_file(source, root / "output.json")
            with self.assertRaises(RUNNER.RunnerError) as result:
                RUNNER.copy_bound_file(source, root / "output.json", expected={"sha256": "f" * 64, "bytes": 2})
            self.assertNotIsInstance(result.exception, RUNNER.OutputPublicationError)

    def test_sample_publication_failure_is_a_job_failure_without_accepted_metric(self) -> None:
        owner = FIXTURES.PrivateSettlementFailureRetentionTests()
        with tempfile.TemporaryDirectory() as temporary, owner.execution_fixture(Path(temporary).resolve()) as fixture:
            original = RUNNER.private_record
            def write(path, value):
                if path.name == "benchmark-sample.json":
                    raise OSError("synthetic disk full")
                return original(path, value)
            with mock.patch.object(RUNNER, "private_record", side_effect=write), self.assertRaises(RUNNER.OutputPublicationError):
                fixture["execute"]()
            attempt = sorted((fixture["output"] / "attempts").iterdir())[1]
            outcome = json.loads((attempt / "validation-outcome.json").read_text())
            self.assertEqual(outcome["validation_kind"], "publication_failed")
            self.assertFalse(outcome["passed"])
            self.assertFalse((attempt / "benchmark-sample.json").exists())


if __name__ == "__main__":
    unittest.main()
