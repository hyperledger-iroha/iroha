"""Tests for AtomicPrivateSettlementV1 benchmark evidence reporting."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import copy
import contextlib
import io
import tempfile
import sys
import unittest
from pathlib import Path
from typing import Any
from dataclasses import replace

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "private_settlement_benchmark_report.py"
SPEC = importlib.util.spec_from_file_location(
    "private_settlement_benchmark_report", SCRIPT
)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)
FIXTURE_SPEC = importlib.util.spec_from_file_location(
    "benchmark_accounting_protocol_fixtures", Path(__file__).with_name("private_settlement_campaign_accounting_test.py"))
assert FIXTURE_SPEC is not None and FIXTURE_SPEC.loader is not None
FIXTURES = importlib.util.module_from_spec(FIXTURE_SPEC)
FIXTURE_SPEC.loader.exec_module(FIXTURES)


def registered_fixture(groups, states=None):
    """Bind synthetic metric matrices to real reducer protocol fixtures.

    These are small JSON test records, not real network or release qualification.
    The shared canonical fixture supplies terminal field inventories; every new
    plan/request/attempt/response/sample binding is recomputed below.
    """
    states = states or [["succeeded"] * len(group) for group in groups]
    templates = {state: FIXTURES.fixture([[state]]) for state in {state for group in states for state in group}}
    plans = []
    for group in groups:
        plan = json.loads(next(iter(templates.values()))[1][0]["plan"])
        first = group[0]
        plan.update(commit=first.commit, requirements={"bootstrap_iterations": 100}, jobs=[])
        plan["hardware"].update(sha256=first.hardware_sha256, profile_sha256=first.hardware_profile_sha256)
        for value in group:
            job = {"kind": "benchmark", **{key: getattr(value, key) for key in
                   ("profile", "participants", "seed", "run", "warmup", "configuration_sha256")}}
            job["request_id"] = hashlib.sha256(MODULE.attempt_accounting.accounting_canonical_bytes(job)).hexdigest()
            plan["jobs"].append(job)
        plans.append(FIXTURES.raw(plan))
    scope = json.loads(next(iter(templates.values()))[0])
    scope["campaigns"] = [{"campaign_id": f"campaign-{index}", "plan": FIXTURES.binding(plan)} for index, plan in enumerate(plans)]
    scope_raw = FIXTURES.raw(scope)
    campaigns, samples = [], []
    for index, (group, statuses, plan_raw) in enumerate(zip(groups, states, plans)):
        plan = json.loads(plan_raw)
        base = {"scope_sha256": FIXTURES.binding(scope_raw)["sha256"], "campaign_id": f"campaign-{index}",
                "plan_sha256": FIXTURES.binding(plan_raw)["sha256"]}
        packets, started = [], []
        for ordinal, (value, state, job) in enumerate(zip(group, statuses, plan["jobs"]), 1):
            packet = {key: None if raw is None else json.loads(raw) for key, raw in
                      templates[state][1][0]["attempts"][0].items() if key != "request_id"}
            attempt_id = MODULE.attempt_accounting.registered_attempt_id(
                base["scope_sha256"], base["campaign_id"], base["plan_sha256"], job["request_id"])
            identity = {**base, "attempt_id": attempt_id, "request_id": job["request_id"],
                        "invocation_nonce": hashlib.sha256(f"{index}:{ordinal}".encode()).hexdigest()}
            if packet["started"] is None:
                packets.append({"request_id": job["request_id"], **packet})
                continue
            started.append(job["request_id"])
            request = packet["request"]
            request.update(**{key: job[key] for key in ("request_id", "participants", "seed", "run", "configuration_sha256")},
                           invocation_nonce=identity["invocation_nonce"], commit=value.commit,
                           hardware_sha256=value.hardware_sha256, hardware_profile_sha256=value.hardware_profile_sha256,
                           payload={"profile": value.profile, "warmup": value.warmup})
            request_binding = FIXTURES.binding(FIXTURES.raw(request))
            for key in ("started", "process", "validation"):
                if packet[key] is not None:
                    packet[key].update(identity)
            packet["started"].update(request=request_binding, harness=plan["harness"])
            if packet["validation"] is not None:
                packet["validation"]["ordinal"] = ordinal
            common = {key: request[key] for key in ("request_id", "invocation_nonce", "commit", "participants")}
            terminal = packet["rust_terminal"]
            if terminal is not None:
                terminal.update(common, request_sha256=request_binding["sha256"])
                if terminal["outcome"]["kind"] == "succeeded":
                    result = terminal["outcome"]["result"]
                    result.update(common, request_sha256=request_binding["sha256"])
                    result["payload"].update(stages_ms=dict(value.stages_ms), successful_leg_applications=value.participants,
                                             **dict(value.resources))
            if packet["adapter"] is not None:
                packet["adapter"].update(common, request_sha256=request_binding["sha256"],
                    rust_terminal=None if terminal is None else FIXTURES.binding(FIXTURES.raw(terminal)))
            if packet["response"] is not None:
                response = packet["response"]
                response.update(common, hardware_sha256=value.hardware_sha256,
                                hardware_profile_sha256=value.hardware_profile_sha256,
                                configuration_sha256=value.configuration_sha256,
                                payload=copy.deepcopy(terminal["outcome"]["result"]["payload"]))
                response_binding = FIXTURES.binding(FIXTURES.raw(response))
                packet["response_outcome"]["response"] = response_binding
                if packet["validation"] is not None and packet["validation"]["validation_kind"] == "accepted":
                    packet["validation"]["response"] = response_binding
            if packet["sample"] is not None:
                normalized = replace(value, attempt_id=attempt_id)
                sample_raw = {"version": 1, "protocol": MODULE.PROTOCOL, **vars(normalized)}
                sample_raw.update(sample_raw.pop("resources"))
                packet["sample"] = sample_raw
                packet["validation"]["sample"] = FIXTURES.binding(FIXTURES.raw(sample_raw))
                samples.append(normalized)
            packets.append({"request_id": job["request_id"], **{key: None if value is None else FIXTURES.raw(value)
                                                                for key, value in packet.items()}})
        closure = {"version": 1, "protocol": MODULE.PROTOCOL, **base, "closed_ns": 10_000,
                   "quiescent": True, "started_request_ids": started,
                   "reason": "completed" if all(state == "succeeded" for state in statuses) else "fail_fast"}
        campaigns.append({"campaign_id": base["campaign_id"], "plan": plan_raw,
                          "closure": FIXTURES.raw(closure), "attempts": packets})
    return samples, scope_raw, campaigns


def registered_report(samples):
    values, scope_raw, campaigns = registered_fixture([samples])
    return MODULE.build_report(values, 100, scope_raw=scope_raw, campaigns=campaigns)


def sample(
    profile: str,
    participants: int,
    seed: int,
    run: int,
    warmup: bool,
    scale: float = 1.0,
):
    private_stages = {
        stage: scale * (index + 1)
        for index, stage in enumerate(MODULE.REQUIRED_PRIVATE_STAGES)
    }
    stages = (
        private_stages
        if profile == "private"
        else {
            stage: private_stages[stage] for stage in ("global_finality", "end_to_end")
        }
    )
    return MODULE.Sample(
        attempt_id=hashlib.sha256(f"{profile}:{participants}:{seed}:{run}:{warmup}".encode()).hexdigest(),
        commit="a" * 40,
        hardware_sha256="b" * 64,
        hardware_profile_sha256="c" * 64,
        configuration_sha256=f"{participants:064x}",
        profile=profile,
        participants=participants,
        seed=seed,
        run=run,
        warmup=warmup,
        stages_ms=stages,
        resources={field: scale * 10 for field in MODULE.RESOURCE_FIELDS},
    )


def complete_matrix(scale: float = 1.0):
    samples: list[Any] = []
    for profile in MODULE.PROFILES:
        for participants in MODULE.REQUIRED_PARTICIPANTS:
            samples.extend(
                sample(profile, participants, 0, run, True, scale)
                for run in range(MODULE.MIN_WARMUPS)
            )
            samples.extend(
                sample(profile, participants, run % 2, run, False, scale)
                for run in range(MODULE.MIN_MEASURED)
            )
    return samples


class PrivateSettlementBenchmarkReportTests(unittest.TestCase):
    """Validate matrix enforcement, statistics, and regression policy."""

    def test_complete_matrix_reports_every_profile_and_participant(self) -> None:
        report = registered_report(complete_matrix())
        self.assertEqual(set(report["profiles"]), set(MODULE.PROFILES))
        self.assertEqual(
            set(report["profiles"]["private"]),
            {str(value) for value in MODULE.REQUIRED_PARTICIPANTS},
        )
        self.assertEqual(
            report["profiles"]["private"]["3"]["stages_ms"]["end_to_end"]["count"],
            MODULE.MIN_MEASURED,
        )

    def test_missing_real_network_participant_bucket_is_rejected(self) -> None:
        incomplete = [
            value
            for value in complete_matrix()
            if not (value.profile == "private" and value.participants == 16)
        ]
        with self.assertRaises(MODULE.EvidenceError):
            registered_report(incomplete)

    def test_baseline_policy_allows_small_shift_and_rejects_large_shift(self) -> None:
        baseline = registered_report(complete_matrix(1.0))
        small = registered_report(complete_matrix(1.05))
        large = registered_report(complete_matrix(1.25))
        self.assertEqual(MODULE.compare_baseline(small, baseline), [])
        regressions = MODULE.compare_baseline(large, baseline)
        self.assertTrue(regressions)
        self.assertTrue({item["quantile"] for item in regressions} >= {"p95", "p99"})

    def test_baseline_from_different_hardware_is_rejected(self) -> None:
        baseline = registered_report(complete_matrix(1.0))
        candidate = registered_report(complete_matrix(1.0))
        baseline["environment"]["hardware_profile_sha256"] = "e" * 64
        with self.assertRaisesRegex(
            MODULE.EvidenceError, "identical hardware profiles and configurations"
        ):
            MODULE.compare_baseline(candidate, baseline)

    def test_baseline_allows_new_commit_bound_hardware_artifact(self) -> None:
        baseline = registered_report(complete_matrix(1.0))
        candidate = registered_report(complete_matrix(1.0))
        candidate["commit"] = "d" * 40
        candidate["environment"]["hardware_sha256"] = "e" * 64
        self.assertEqual(MODULE.compare_baseline(candidate, baseline), [])

    def test_baseline_rejects_malformed_configuration_binding(self) -> None:
        baseline = registered_report(complete_matrix(1.0))
        candidate = registered_report(complete_matrix(1.0))
        baseline["environment"]["configuration_sha256_by_participants"] = []
        with self.assertRaisesRegex(MODULE.EvidenceError, "environment is malformed"):
            MODULE.compare_baseline(candidate, baseline)

    def test_percentile_and_mad_are_deterministic(self) -> None:
        first = MODULE.summarize_values(
            [1.0, 2.0, 3.0, 4.0], binding=b"fixed", bootstrap_iterations=100
        )
        second = MODULE.summarize_values(
            [1.0, 2.0, 3.0, 4.0], binding=b"fixed", bootstrap_iterations=100
        )
        self.assertEqual(first, second)
        self.assertEqual(first["p50"], 2.5)
        self.assertEqual(first["mad"], 1.0)

    def test_mixed_source_commits_are_rejected(self) -> None:
        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            attempt_id=original.attempt_id,
            commit="b" * 40,
            hardware_sha256=original.hardware_sha256,
            hardware_profile_sha256=original.hardware_profile_sha256,
            configuration_sha256=original.configuration_sha256,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(MODULE.EvidenceError, "one exact source commit"):
            registered_report(samples)

    def test_mixed_hardware_or_n_configuration_is_rejected(self) -> None:
        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            attempt_id=original.attempt_id,
            commit=original.commit,
            hardware_sha256="d" * 64,
            hardware_profile_sha256=original.hardware_profile_sha256,
            configuration_sha256=original.configuration_sha256,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(
            MODULE.EvidenceError, "one pinned hardware description"
        ):
            registered_report(samples)

        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            attempt_id=original.attempt_id,
            commit=original.commit,
            hardware_sha256=original.hardware_sha256,
            hardware_profile_sha256="e" * 64,
            configuration_sha256=original.configuration_sha256,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(MODULE.EvidenceError, "one pinned hardware profile"):
            registered_report(samples)

        samples = complete_matrix()
        original = samples[-1]
        samples[-1] = MODULE.Sample(
            attempt_id=original.attempt_id,
            commit=original.commit,
            hardware_sha256=original.hardware_sha256,
            hardware_profile_sha256=original.hardware_profile_sha256,
            configuration_sha256="d" * 64,
            profile=original.profile,
            participants=original.participants,
            seed=original.seed,
            run=original.run,
            warmup=original.warmup,
            stages_ms=original.stages_ms,
            resources=original.resources,
        )
        with self.assertRaisesRegex(MODULE.EvidenceError, "one pinned configuration"):
            registered_report(samples)


class RegisteredSampleTests(unittest.TestCase):
    """Check first-release metric parsing and cross-campaign sample identities."""

    def raw_sample(self):
        value = vars(sample("private", 3, 0, 0, False)).copy()
        value.update(value.pop("resources"))
        return {"version": 1, "protocol": MODULE.PROTOCOL, **value}

    def test_sample_requires_identity_while_inner_measurement_has_no_scope(self) -> None:
        raw = self.raw_sample()
        parsed = MODULE.parse_sample(raw, "synthetic")
        self.assertEqual(parsed.attempt_id, raw["attempt_id"])
        inner = {key: value for key, value in raw.items() if key != "attempt_id"}
        self.assertIsInstance(MODULE.parse_measurement(inner, "synthetic"), MODULE.Measurement)
        with self.assertRaises(MODULE.EvidenceError):
            MODULE.parse_sample(inner, "synthetic")
        with self.assertRaises(MODULE.EvidenceError):
            MODULE.parse_measurement(raw, "synthetic")
        for key, value in (("attempt_id", "bad"), ("version", True), ("participants", 3.0)):
            with self.subTest(field=key), self.assertRaises(MODULE.EvidenceError):
                MODULE.parse_sample({**raw, key: value}, "synthetic")

    def test_same_coordinates_from_distinct_attempts_survive_but_reused_ids_fail(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "samples.jsonl"
            raw = self.raw_sample()
            other = {**raw, "attempt_id": "f" * 64}
            path.write_text(json.dumps(raw) + "\n" + json.dumps(other) + "\n")
            self.assertEqual(len(MODULE.load_jsonl([path])), 2)
            path.write_text(json.dumps(raw) + "\n" + json.dumps(raw) + "\n")
            with self.assertRaisesRegex(MODULE.EvidenceError, "duplicate"):
                MODULE.load_jsonl([path])
            values = complete_matrix()
            with self.assertRaisesRegex(MODULE.EvidenceError, "duplicated"):
                MODULE.validate_matrix(values + [values[0]])

    def test_raw_jsonl_rejects_duplicate_fields_before_normalization(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "samples.jsonl"
            path.write_text('{"version":1,' + json.dumps(self.raw_sample())[1:] + "\n")
            with self.assertRaisesRegex(MODULE.EvidenceError, "JSON"):
                MODULE.load_jsonl([path])


class RegisteredReportTests(unittest.TestCase):
    """Exercise mandatory real reduction over clearly synthetic protocol inputs."""

    def campaign_fixture(self):
        predecessor = [sample("private", 3, index % 2, 700 + index, False) for index in range(3)]
        return registered_fixture([predecessor, complete_matrix()],
                                  [["succeeded", "failed", "not_started"], ["succeeded"] * 350])

    def test_failed_predecessor_retains_accepted_measurements_and_warmup_split(self):
        samples, scope_raw, campaigns = self.campaign_fixture()
        result = MODULE.build_report(samples, 100, scope_raw=scope_raw, campaigns=campaigns)
        self.assertEqual(result["accounting"]["counts"], {
            "planned": 353, "attempted": 352, "succeeded": 351, "failed": 1,
            "timed_out": 0, "not_started": 1, "incomplete": 0})
        cohorts = {(row["profile"], row["participants"], row["warmup"]): row["counts"]
                   for row in result["accounting"]["cohorts"]}
        self.assertEqual(cohorts[("private", 3, False)]["failed"], 1)
        self.assertEqual(cohorts[("private", 3, True)]["succeeded"], 5)
        self.assertEqual(result["profiles"]["private"]["3"]["measured_runs"], 31)
        self.assertEqual(result["profiles"]["private"]["3"]["stages_ms"]["end_to_end"]["count"], 31)

    def test_omitted_predecessor_or_retained_success_is_rejected(self):
        samples, scope_raw, campaigns = self.campaign_fixture()
        with self.assertRaisesRegex(MODULE.EvidenceError, "campaign inventory"):
            MODULE.build_report(samples, 100, scope_raw=scope_raw, campaigns=campaigns[1:])
        with self.assertRaisesRegex(MODULE.EvidenceError, "measurement was omitted"):
            MODULE.build_report(samples[1:], 100, scope_raw=scope_raw, campaigns=campaigns)

    def test_duplicate_or_failed_attempt_metrics_are_rejected(self):
        samples, scope_raw, campaigns = self.campaign_fixture()
        with self.assertRaisesRegex(MODULE.EvidenceError, "duplicated"):
            MODULE.build_report(samples + [samples[0]], 100, scope_raw=scope_raw, campaigns=campaigns)
        failed = json.loads(campaigns[0]["attempts"][1]["started"])["attempt_id"]
        with self.assertRaisesRegex(MODULE.EvidenceError, "unsuccessful"):
            MODULE.build_report(samples + [replace(samples[0], attempt_id=failed)], 100,
                                scope_raw=scope_raw, campaigns=campaigns)

    def test_changed_metric_and_missing_terminal_cannot_be_reported(self):
        samples, scope_raw, campaigns = self.campaign_fixture()
        altered = replace(samples[0], resources={**samples[0].resources, "cpu_seconds": 999.0})
        with self.assertRaisesRegex(MODULE.EvidenceError, "published successful row"):
            MODULE.build_report([altered, *samples[1:]], 100, scope_raw=scope_raw, campaigns=campaigns)
        samples, scope_raw, campaigns = registered_fixture(
            [complete_matrix(), [sample("private", 3, 0, 900, False)]],
            [["succeeded"] * 350, ["missing_terminal"]])
        with self.assertRaisesRegex(MODULE.EvidenceError, "accounting is incomplete"):
            MODULE.build_report(samples, 100, scope_raw=scope_raw, campaigns=campaigns)

    def test_registered_bootstrap_policy_is_mandatory(self):
        samples, scope_raw, campaigns = registered_fixture([complete_matrix()])
        with self.assertRaisesRegex(MODULE.EvidenceError, "bootstrap policy"):
            MODULE.build_report(samples, 101, scope_raw=scope_raw, campaigns=campaigns)
        with self.assertRaises(TypeError):
            MODULE.build_report(samples, 100)
        with self.assertRaises(MODULE.EvidenceError):
            MODULE.build_report(samples, True, scope_raw=scope_raw, campaigns=campaigns)

    def test_baseline_requires_exact_complete_accounting_and_matching_cohorts(self):
        original = registered_report(complete_matrix())
        for mutation in ("missing", "counter", "cohort", "private", "incomplete", "header", "summary"):
            baseline = copy.deepcopy(original)
            if mutation == "missing":
                baseline.pop("accounting")
            elif mutation == "counter":
                baseline["accounting"]["counts"]["failed"] = 1
            elif mutation == "cohort":
                baseline["profiles"]["private"]["3"]["measured_runs"] = 31
            elif mutation == "private":
                baseline["accounting"]["rows"][0]["private_key"] = "synthetic-forbidden-field"
            elif mutation == "incomplete":
                baseline["accounting"]["accounting_complete"] = False
            elif mutation == "header":
                baseline["version"] = True
            else:
                baseline["profiles"]["private"]["3"]["stages_ms"]["end_to_end"]["count"] = 31
            with self.subTest(mutation=mutation), self.assertRaises(MODULE.EvidenceError):
                MODULE.compare_baseline(original, baseline)

    def test_cli_requires_registered_scope(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            MODULE.parse_args(["--input", "raw.jsonl", "--output", "report.json"])
        arguments = MODULE.parse_args(["--input", "raw.jsonl", "--scope", "scope.json", "--output", "report.json"])
        self.assertEqual(arguments.scope, Path("scope.json"))
        with tempfile.TemporaryDirectory() as temporary, contextlib.redirect_stderr(io.StringIO()):
            root = Path(temporary)
            result = MODULE.main(["--input", str(root / "raw.jsonl"), "--scope", str(root / "scope.json"),
                                  "--output", str(root / "report.json")])
            self.assertEqual(result, 2)
            self.assertFalse((root / "report.json").exists())


if __name__ == "__main__":
    unittest.main()
