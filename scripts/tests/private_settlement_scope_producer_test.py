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
import retained_scope_foundation as RETAINED
from scripts.tests.private_settlement_registered_accounting_fixture import fixture_admission

CONTROL = RETAINED.control
ACCOUNTING = RETAINED.accounting


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

    def scope_fixture(self):
        """Use current retained owners with explicitly synthetic process facts."""
        fixture = RETAINED.RegisteredScopeIntegrationTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        return fixture

    def closure_fixture(self):
        """Retain one actual session acceptance and its unstarted suffix."""
        fixture = self.scope_fixture()
        fixture.accepted_sample_graph()
        root = fixture.root / "campaigns" / "campaign-0"
        closure = json.loads((root / "campaign-closure.json").read_bytes())
        (root / "campaign-closure.json").unlink()
        arguments = dict(
            plan=fixture.plans["campaign-0"], scope_path=fixture.scope_path,
            scope_sha256=RUNNER.file_binding(fixture.scope_path)["sha256"],
            campaign_id="campaign-0", plan_sha256=fixture.scope["campaigns"][0]["plan"]["sha256"],
            validate_success=fixture.callback,
        )
        return fixture, root, closure, arguments

    def unstarted_fixture(self, admission):
        """Freeze the full registered plan before claiming its output directory."""
        fixture = self.scope_fixture()
        fixture.harness = RUNNER.verify_harness(admission["plan_harness"])
        fixture.materialize()
        output = fixture.root / "campaigns" / "campaign-0"
        inputs = fixture.root / "frozen-input"
        output.rename(inputs)
        arguments = dict(
            campaign_id="campaign-0", plan_path=inputs / "frozen-plan.json",
            source_root=admission["source_root"], harness=admission["plan_harness"],
            smoke_campaign=admission["smoke_campaign"], worker_path=admission["worker_path"],
            validator_path=admission["validator_path"],
        )
        return fixture, inputs, output, arguments

    def test_campaign_closure_binds_actual_durable_starts_without_fabricating_success(self) -> None:
        fixture, root, original, arguments = self.closure_fixture()
        with mock.patch.object(RUNNER, "_process_group_exists", side_effect=AssertionError("historical PID reused")) as groups:
            result = RUNNER.close_benchmark_campaign(root, reason="fail_fast", **arguments)
        groups.assert_not_called()
        raw = result["path"].read_bytes()
        closure = json.loads(raw)
        self.assertEqual(closure["started_request_ids"], original["started_request_ids"])
        self.assertEqual(closure["reason"], "fail_fast")
        self.assertNotIn("succeeded", closure)
        self.assertEqual(result["campaign"]["counts"]["succeeded"], 1)
        self.assertEqual(result["campaign"]["counts"]["not_started"], 799)
        self.assertFalse(result["release_qualified"])
        with self.assertRaisesRegex(CONTROL.SessionProtocolError, "closure already exists"):
            RUNNER.close_benchmark_campaign(root, reason="fail_fast", **arguments)
        self.assertEqual(result["path"].read_bytes(), raw)

    def test_closure_refuses_missing_terminal_live_group_and_substituted_identity(self) -> None:
        for mutation in ("missing", "live", "identity", "extra", "completed"):
            with self.subTest(mutation=mutation):
                fixture, root, _, arguments = self.closure_fixture()
                path = next((root / "attempts").glob("*/process-outcome.json"))
                if mutation == "missing":
                    (root / fixture.sample_fixture.prefix / "session-closure.json").unlink()
                elif mutation == "live":
                    value = json.loads(path.read_bytes()); value["owned_process_group_gone"] = False
                    path.write_bytes(CONTROL.canonical(value))
                elif mutation == "identity":
                    value = json.loads(path.read_text()); value["attempt_id"] = "e" * 64
                    path.write_bytes(CONTROL.canonical(value))
                elif mutation == "extra":
                    (root / "attempts" / "undeclared").mkdir(mode=0o700)
                with mock.patch.object(RUNNER, "_process_group_exists", side_effect=AssertionError("historical PID reused")) as groups, self.assertRaises((CONTROL.SessionProtocolError, ACCOUNTING.AccountingError)):
                    RUNNER.close_benchmark_campaign(root,
                        reason="completed" if mutation == "completed" else "fail_fast", **arguments)
                groups.assert_not_called()
                self.assertFalse((root / "campaign-closure.json").exists())

    def test_benchmark_invocation_requires_scope_before_creating_attempt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with mock.patch.object(RUNNER.subprocess, "Popen") as spawn, self.assertRaisesRegex(RUNNER.RunnerError, "retained session execution owner"):
                RUNNER.invoke_harness(root / "harness", {"kind": "benchmark"}, attempt_dir=root / "attempt", timeout_seconds=1)
            spawn.assert_not_called()
            self.assertFalse((root / "attempt").exists())

    def test_spawn_failure_retains_typed_completion_and_quiescence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            identity = {"scope_sha256": "a" * 64, "campaign_id": "test", "plan_sha256": "b" * 64, "attempt_id": RUNNER.attempt_accounting.registered_attempt_id("a" * 64, "test", "b" * 64, "d" * 64)}
            with mock.patch.object(RUNNER.subprocess, "Popen", side_effect=OSError("synthetic spawn failure")), self.assertRaises(RUNNER.RunnerError):
                RUNNER.invoke_harness(root / "harness", {"kind": "fault", "request_id": "d" * 64, "invocation_nonce": "e" * 64},
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
            with self.assertRaises(FIXTURES.EXECUTION.CampaignExecutionIncomplete) as failure:
                fixture["execute"]()
            self.assertIn("later audit failure", str(failure.exception.owner.error))
            ordinal, job = next((i, job) for i, job in enumerate(fixture["plan"]["jobs"], 1)
                                if job["kind"] == "benchmark")
            attempt = fixture["output"] / "attempts" / f"{ordinal:05}-{job['request_id']}"
            sample = attempt / "benchmark-sample.json"
            validation = json.loads((attempt / "validation-outcome.json").read_text())
            self.assertEqual(validation["validation_kind"], "accepted")
            self.assertEqual(validation["sample"], RUNNER.file_binding(sample, relative_to=fixture["output"]))
            self.assertEqual(json.loads(sample.read_text())["attempt_id"], validation["attempt_id"])
            closure = json.loads((fixture["output"] / "campaign-closure.json").read_text())
            self.assertEqual(closure["started_request_ids"],
                             [job["request_id"] for job in fixture["plan"]["jobs"]])
            self.assertFalse((fixture["output"] / "release-artifact-fragment-v1.json").exists())
            self.assertFalse((fixture["output"] / "campaign-artifacts.json").exists())

    def test_unused_campaign_closure_is_exclusive_and_retains_original_plan_inputs(self) -> None:
        with fixture_admission() as admission, mock.patch.object(RUNNER.subprocess, "Popen") as spawn:
            fixture, inputs, _, arguments = self.unstarted_fixture(admission)
            output = RUNNER.close_unstarted_campaign(fixture.scope_path, **arguments)
            closure = json.loads(output.read_text())
            self.assertEqual(closure["reason"], "not_run")
            self.assertEqual(closure["started_request_ids"], [])
            self.assertFalse((output.parent / "attempts").exists())
            self.assertEqual((output.parent / "frozen-plan.json").read_bytes(),
                             (inputs / "frozen-plan.json").read_bytes())
            for key in ("hardware", "canary_manifest", "configuration_manifest"):
                relative = fixture.plans["campaign-0"][key]["path"]
                self.assertEqual((output.parent / relative).read_bytes(), (inputs / relative).read_bytes())
            with self.assertRaises(FileExistsError):
                RUNNER.close_unstarted_campaign(fixture.scope_path, **arguments)
            spawn.assert_not_called()

    def test_frozen_input_binding_failure_preserves_unclosed_claim_without_launching(self) -> None:
        with fixture_admission() as admission, mock.patch.object(RUNNER.subprocess, "Popen") as spawn:
            fixture, inputs, output, arguments = self.unstarted_fixture(admission)
            retain = RUNNER.retain_frozen_plan_inputs
            def substitute(plan_path, destination, **kwargs):
                self.assertTrue(destination.is_dir(), "the exclusive output claim already exists")
                (inputs / fixture.plans["campaign-0"]["hardware"]["path"]).write_bytes(b"changed input")
                return retain(plan_path, destination, **kwargs)
            with mock.patch.object(RUNNER, "retain_frozen_plan_inputs", side_effect=substitute) as frozen, self.assertRaises(RUNNER.RunnerError):
                RUNNER.close_unstarted_campaign(fixture.scope_path, **arguments)
            frozen.assert_called_once()
            self.assertTrue(output.is_dir())
            self.assertFalse((output / "campaign-closure.json").exists())
            spawn.assert_not_called()


class ScopeCollectionTests(unittest.TestCase):
    """Read synthetic native records through the actual retained scope owners."""

    def write_fixture(self, root: Path):
        """Keep failed predecessors and a complete session in the full denominator."""
        from scripts.tests.private_settlement_registered_accounting_fixture import (
            admitted_images, build_registered_accounting_fixture,
        )
        foundation = RETAINED.RegisteredScopeIntegrationTests()
        foundation.setUp()
        self.addCleanup(foundation.doCleanups)
        seed = root / "frozen-inputs"
        plan = foundation.plan(seed)
        configurations = json.loads((seed / plan["configuration_manifest"]["path"]).read_bytes())
        fixture = build_registered_accounting_fixture(
            root, commit=foundation.commit,
            hardware_path=Path(plan["hardware"]["path"]),
            hardware_payload=(seed / plan["hardware"]["path"]).read_bytes(),
            configuration_manifest_path=Path(plan["configuration_manifest"]["path"]),
            configuration_manifest_payload=(seed / plan["configuration_manifest"]["path"]).read_bytes(),
            configuration_payloads={Path(row["path"]): (seed / row["path"]).read_bytes()
                                    for row in configurations["configurations"]},
            validator_sha256=admitted_images()["validator"]["sha256"],
            complete_session_count=1,
        )
        fixture["scope_path"] = root / "accounting" / "scope.json"
        fixture["campaign"] = root / "accounting" / "campaigns" / "campaign-0-failed"
        return fixture

    def test_complete_scope_reduction_preserves_failed_predecessors_and_successes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, fixture_admission() as admission:
            root = Path(temporary).resolve()
            fixture = self.write_fixture(root)
            with RUNNER.collect_benchmark_scope(fixture["scope_path"], **admission) as held:
                self.assertEqual(held.result["accounting"], fixture["accounting"])
                self.assertEqual([json.loads(raw) for raw in held.successful_rows], fixture["rows"])
                self.assertFalse(held.result["source_and_smoke_admitted"])
                self.assertFalse(held.result["release_qualified"])
                held.validate()
            self.assertTrue(held.result["source_and_smoke_admitted"])
            output = RUNNER.write_benchmark_scope_accounting(
                fixture["scope_path"], root / "accounting.json", **admission)
            result = json.loads(output.read_bytes())
            self.assertEqual(result["counts"], dict(planned=1600, attempted=10, not_started=1590,
                                                    succeeded=9, failed=1, timed_out=0, incomplete=0))
            self.assertTrue(result["accounting_complete"])
            self.assertNotIn("release_qualified", result)
            with self.assertRaises(FileExistsError):
                RUNNER.write_benchmark_scope_accounting(fixture["scope_path"], output, **admission)

    def test_retained_timeout_is_counted_without_a_censored_latency_sample(self) -> None:
        import retained_accounting_fixture as small
        fixture = small.Fixture(("timed_out",))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve() / "campaign-a"
            root.mkdir(mode=0o700)
            values = {**fixture.store.values, "frozen-plan.json": fixture.plan_raw,
                      "registered-scope.json": fixture.scope_raw,
                      "campaign-closure.json": fixture.packet["closure"]}
            for name, raw in values.items():
                path = root / name
                path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                path.write_bytes(raw)
                path.chmod(0o600)
            for path in root.rglob("*"):
                if path.is_dir():
                    path.chmod(0o700)
            with RETAINED.replay.collection.collect_closed_campaign(
                    root, scope_raw=fixture.scope_raw, campaign_id="campaign-a",
                    plan_binding=fixture.scope["campaigns"][0]["plan"]) as held:
                result = ACCOUNTING.reduce_retained_campaign(
                    fixture.scope_raw, held.packet, held.successful_rows,
                    worker_command=fixture.command, worker_image=fixture.image,
                    validate_success=fixture.recompute)
                held.validate()
                self.assertEqual(held.successful_rows, [])
            self.assertEqual(result["counts"], dict(planned=800, attempted=1, not_started=799,
                                                    succeeded=0, failed=0, timed_out=1, incomplete=0))

    def test_collection_rejects_omitted_closure_changed_scope_and_symlink_records(self) -> None:
        for mutation in ("closure", "scope", "symlink", "scope_symlink", "extra"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary, fixture_admission() as admission:
                root = Path(temporary).resolve()
                fixture = self.write_fixture(root)
                campaign = fixture["campaign"]
                if mutation == "closure":
                    (campaign / "campaign-closure.json").unlink()
                elif mutation == "scope":
                    (campaign / "registered-scope.json").write_bytes(b"{}")
                elif mutation in {"symlink", "scope_symlink"}:
                    path = fixture["scope_path"] if mutation == "scope_symlink" else campaign / "campaign-closure.json"
                    raw = path.read_bytes()
                    path.unlink()
                    (root / "outside.json").write_bytes(raw)
                    path.symlink_to(root / "outside.json")
                else:
                    (campaign / "attempts" / "undeclared").mkdir(mode=0o700)
                with self.assertRaises((OSError, ValueError, RUNNER.RunnerError)):
                    with RUNNER.collect_benchmark_scope(fixture["scope_path"], **admission):
                        self.fail("changed scope was admitted")

    def test_collection_rejects_changed_earlier_record_and_late_terminal_appearance(self) -> None:
        for mutation in ("changed", "appeared", "extra_protocol"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary, fixture_admission() as admission:
                fixture = self.write_fixture(Path(temporary).resolve())
                jobs = [(ordinal, job) for ordinal, job in enumerate(fixture["plan"]["jobs"], 1)
                        if job["kind"] == "benchmark"]
                first = fixture["campaign"] / "attempts" / f"{jobs[0][0]:05}-{jobs[0][1]['request_id']}"
                unstarted = fixture["campaign"] / "attempts" / f"{jobs[2][0]:05}-{jobs[2][1]['request_id']}"
                collection = RETAINED.replay.collection
                original = collection.collect_closed_campaign
                fired = False
                def collect(path, **kwargs):
                    nonlocal fired
                    if path.name == "campaign-1-complete" and not fired:
                        fired = True
                        if mutation == "changed":
                            target = first / "request.json"
                            target.write_bytes(target.read_bytes() + b" ")
                        elif mutation == "appeared":
                            target = unstarted / "evidence/benchmark-protocol/rust-result.json"
                            target.parent.mkdir(parents=True, mode=0o700)
                            target.write_bytes(b"{}")
                            target.chmod(0o600)
                        else:
                            target = first / "evidence/benchmark-protocol/undeclared.json"
                            target.write_bytes(b"{}")
                            target.chmod(0o600)
                    return original(path, **kwargs)
                with mock.patch.object(collection, "collect_closed_campaign", side_effect=collect), self.assertRaises((OSError, ValueError, RUNNER.RunnerError)):
                    with RUNNER.collect_benchmark_scope(fixture["scope_path"], **admission):
                        self.fail("earlier retained owner changed during collection")
                self.assertTrue(fired)

    def test_full_plan_fault_start_requires_exact_quiescent_process_closure(self) -> None:
        for mutation in (None, "missing", "identity", "live"):
            with self.subTest(mutation=mutation):
                fixture = RETAINED.RegisteredScopeIntegrationTests()
                fixture.setUp()
                self.addCleanup(fixture.doCleanups)
                fixture.accepted_sample_graph()
                path = next((fixture.root / "campaigns/campaign-0/attempts").glob("*/process-outcome.json"))
                value = json.loads(path.read_bytes())
                if mutation == "missing":
                    path.unlink()
                elif mutation == "identity":
                    value["invocation_nonce"] = "b" * 64
                    path.write_bytes(CONTROL.canonical(value))
                elif mutation == "live":
                    value["owned_process_group_gone"] = False
                    path.write_bytes(CONTROL.canonical(value))
                if mutation is None:
                    result = fixture.invoke()
                    self.assertEqual(result["accounting"]["counts"]["not_started"], 799)
                    self.assertEqual(result["accounting"]["counts"]["succeeded"], 1)
                    self.assertFalse(result["release_qualified"])
                else:
                    with self.assertRaises((OSError, ValueError, RUNNER.RunnerError)):
                        fixture.invoke()

    def test_oversized_accounting_record_is_rejected_before_hashing(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary).resolve() / "large.json"
            with path.open("wb") as stream:
                stream.truncate(RUNNER.MAX_HARNESS_RESPONSE_BYTES + 1)
            with mock.patch.object(RUNNER, "file_binding") as hashing, self.assertRaisesRegex(RUNNER.RunnerError, "bounded protocol size"):
                RUNNER.retained_accounting_bytes(path)
            hashing.assert_not_called()

    def test_csv_preserves_the_registered_attempt_join(self) -> None:
        with tempfile.TemporaryDirectory() as temporary, fixture_admission() as admission:
            root = Path(temporary).resolve()
            fixture = self.write_fixture(root)
            with RUNNER.collect_benchmark_scope(fixture["scope_path"], **admission) as held:
                samples = [json.loads(raw) for raw in held.successful_rows]
                output = root / "raw.csv"
                RUNNER.write_benchmark_csv(output, samples)
                with output.open() as stream:
                    rows = list(RUNNER.csv.DictReader(stream))
                self.assertEqual([row["attempt_id"] for row in rows],
                                 [sample["attempt_id"] for sample in samples])
                self.assertEqual(len(rows), 9)

    def test_destination_io_is_typed_without_reclassifying_source_binding_errors(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); source = root / "source.json"; source.write_bytes(b"{}")
            with mock.patch.object(Path, "mkdir", side_effect=OSError("synthetic destination error")), self.assertRaises(RUNNER.OutputPublicationError):
                RUNNER.copy_bound_file(source, root / "output.json")
            with self.assertRaises(RUNNER.RunnerError) as result:
                RUNNER.copy_bound_file(source, root / "output.json", expected={"sha256": "f" * 64, "bytes": 2})
            self.assertNotIsInstance(result.exception, RUNNER.OutputPublicationError)

    def test_sample_publication_failure_is_a_job_failure_without_accepted_metric(self) -> None:
        from private_settlement_session_runtime_test import RunnerCallbackControls
        fixture = RunnerCallbackControls()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        bound = fixture.completed()
        original = fixture.records.publish
        attempted = []
        def publish(path, raw):
            attempted.append(path)
            if path.endswith("/benchmark-sample.json"):
                raise OSError("synthetic disk full")
            return original(path, raw)
        with mock.patch.object(fixture.records, "publish", side_effect=publish), self.assertRaisesRegex(OSError, "synthetic disk full"):
            fixture.callbacks.validate_completion(0, bound)
        attempt = fixture.records.path / fixture.row["output_directory"]
        self.assertEqual(attempted, [fixture.row["output_directory"] + "/benchmark-sample.json"])
        self.assertFalse((attempt / "benchmark-sample.json").exists())
        self.assertFalse((attempt / "validation-outcome.json").exists())
        self.assertEqual(list(fixture.records.path.glob("sessions/*/control/ack-*")), [])
        self.assertEqual(fixture.records.read(fixture.records.locate(
            fixture.row["output_directory"] + "/evidence/benchmark-protocol/rust-result.json")),
            bound["rust_terminal"])

if __name__ == "__main__":
    unittest.main()
