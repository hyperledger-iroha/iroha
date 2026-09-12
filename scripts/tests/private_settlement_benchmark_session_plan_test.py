"""Pure prospective benchmark session planning; no executed warmup qualification."""
from __future__ import annotations

import copy
import hashlib
import importlib.util
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location(
    "session_plan_under_test", Path(__file__).resolve().parents[1] / "private_settlement_attempt_accounting.py")
assert SPEC is not None and SPEC.loader is not None
ACCOUNTING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ACCOUNTING)


def inputs():
    return {
        "configuration_sha256": {n: hashlib.sha256(f"config:{n}".encode()).hexdigest() for n in (2, 3, 4, 8, 16)},
        "workload_sha256": {n: hashlib.sha256(f"workload:{n}".encode()).hexdigest() for n in (2, 3, 4, 8, 16)},
        "seeds": list(range(10)), "warmups_per_session": 5, "measured_per_profile": 30,
    }


def identified(body):
    return {"request_id": hashlib.sha256(ACCOUNTING.accounting_canonical_bytes(body)).hexdigest(), **body}


def dispatch_case(started_benchmarks, state="succeeded"):
    params = inputs()
    plan = ACCOUNTING.build_benchmark_session_plan(**params)
    prefix = identified({"kind": "fault", "participants": 3, "seed": 0, "run": 0})
    suffix = identified({"kind": "leakage", "participants": 3, "seed": 0, "run": 0})
    jobs = [prefix, *plan["jobs"], suffix]
    states = [{"request_id": job["request_id"],
               "state": "succeeded" if index < started_benchmarks else "not_started"}
              for index, job in enumerate(plan["jobs"])]
    if started_benchmarks:
        states[started_benchmarks - 1]["state"] = state
    return {**params, "full_jobs": jobs,
            "started_request_ids": [job["request_id"] for job in jobs[:1 + started_benchmarks]],
            "job_states": [{"request_id": prefix["request_id"], "state": "succeeded"},
                           *states, {"request_id": suffix["request_id"], "state": "not_started"}]}


class BenchmarkSessionPlanTests(unittest.TestCase):
    def test_each_seed_has_its_own_ordered_warmups_and_measurements(self):
        plan = ACCOUNTING.build_benchmark_session_plan(**inputs())
        self.assertEqual((len(plan["sessions"]), len(plan["jobs"])), (100, 800))
        offset = 0
        for session in plan["sessions"]:
            jobs = plan["jobs"][offset:offset + 8]
            self.assertEqual([job["session_attempt_index"] for job in jobs], list(range(8)))
            self.assertEqual([job["warmup"] for job in jobs], [True] * 5 + [False] * 3)
            self.assertTrue(all(job["session_id"] == session["session_id"] for job in jobs))
            offset += 8
        self.assertEqual(len({job["request_id"] for job in plan["jobs"]}), 800)

    def test_profile_pairs_are_adjacent_with_counterbalanced_first_profile(self):
        for seed_count in (10, 11):
            params = inputs(); params["seeds"] = list(range(seed_count)); params["measured_per_profile"] = 3 * seed_count
            plan = ACCOUNTING.build_benchmark_session_plan(**params)
            for topology_index, participants in enumerate((2, 3, 4, 8, 16)):
                sessions = plan["sessions"][topology_index * 2 * seed_count:(topology_index + 1) * 2 * seed_count]
                first_profiles = []
                for seed_index in range(seed_count):
                    left, right = sessions[2 * seed_index:2 * seed_index + 2]
                    self.assertEqual((left["participants"], right["participants"]), (participants, participants))
                    self.assertEqual((left["seed"], right["seed"]), (seed_index, seed_index))
                    self.assertEqual(left["workload_sha256"], right["workload_sha256"])
                    self.assertEqual({left["profile"], right["profile"]}, {"private", "transparent_control"})
                    first_profiles.append(left["profile"])
                self.assertTrue(all(a != b for a, b in zip(first_profiles, first_profiles[1:])))
                self.assertLessEqual(abs(first_profiles.count("private") - first_profiles.count("transparent_control")), 1)
                self.assertEqual(first_profiles[0], "private")

    def test_unequal_allocation_and_profile_pairing_are_explicit(self):
        params = inputs(); params["measured_per_profile"] = 31
        plan = ACCOUNTING.build_benchmark_session_plan(**params)
        pairs = {}
        for session in plan["sessions"]:
            key = (session["participants"], session["seed"])
            pair = {name: session[name] for name in ("seed", "workload_sha256", "warmup_attempts", "measured_attempts")}
            pairs.setdefault(key, []).append(pair)
        self.assertEqual(len(pairs), 50)
        for (_, seed), values in pairs.items():
            self.assertEqual(values[0], values[1])
            self.assertEqual(values[0]["measured_attempts"], 4 if seed == 0 else 3)
        self.assertEqual(len(plan["jobs"]), 810)

    def test_deterministic_ids_bind_coordinates_and_registered_attempt_identity(self):
        params = inputs(); retained = copy.deepcopy(params)
        first = ACCOUNTING.build_benchmark_session_plan(**params)
        self.assertEqual(first, ACCOUNTING.build_benchmark_session_plan(**params))
        self.assertEqual(params, retained)
        for session in first["sessions"]:
            body = {key: value for key, value in session.items() if key != "session_id"}
            self.assertEqual(session["session_id"], hashlib.sha256(ACCOUNTING.accounting_canonical_bytes({
                "domain": "iroha:private-settlement:benchmark-session-plan:v1", **body})).hexdigest())
        for job in first["jobs"]:
            self.assertEqual(job, identified({key: value for key, value in job.items() if key != "request_id"}))
        request_id = first["jobs"][0]["request_id"]
        a = ACCOUNTING.registered_attempt_id("a" * 64, "one", "b" * 64, request_id)
        self.assertNotEqual(a, ACCOUNTING.registered_attempt_id("a" * 64, "two", "b" * 64, request_id))

    def test_invalid_counts_seeds_and_allocations_reject(self):
        changes = [("warmups_per_session", x) for x in (0, True, 4, 1001)]
        changes += [("measured_per_profile", x) for x in (0, True, 29, 1001)]
        changes += [("seeds", x) for x in (list(range(9)), [1, 0, *range(2, 10)],
                    [0, *range(9)], [True, *range(1, 10)], [*range(9), 1 << 64], list(range(31)))]
        for key, value in changes:
            params = inputs(); params[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ACCOUNTING.AccountingError):
                ACCOUNTING.build_benchmark_session_plan(**params)
        params = inputs(); params["warmups_per_session"] = 1000
        with self.assertRaisesRegex(ACCOUNTING.AccountingError, "bounded benchmark job"):
            ACCOUNTING.build_benchmark_session_plan(**params)

    def test_invalid_and_changed_configuration_or_workload_bindings_reject(self):
        expected = ACCOUNTING.build_benchmark_session_plan(**inputs())
        for field in ("configuration_sha256", "workload_sha256"):
            for mutation in ("zero", "missing", "string_key", "changed"):
                params = inputs()
                if mutation == "zero": params[field][3] = "0" * 64
                elif mutation == "missing": del params[field][3]
                elif mutation == "string_key": params[field]["3"] = params[field].pop(3)
                else: params[field][3] = "f" * 64
                with self.subTest(field=field, mutation=mutation), self.assertRaises(ACCOUNTING.AccountingError):
                    ACCOUNTING.validate_benchmark_session_plan(expected, **params)

    def test_missing_reordered_relabelled_and_rehashed_attempts_reject(self):
        for mutation in ("omit_warmup", "reorder", "relabel", "duplicate", "omit_session", "unknown"):
            params = inputs(); plan = ACCOUNTING.build_benchmark_session_plan(**params)
            if mutation == "omit_warmup": del plan["jobs"][0]
            elif mutation == "reorder": plan["jobs"][0], plan["jobs"][5] = plan["jobs"][5], plan["jobs"][0]
            elif mutation == "relabel":
                body = {key: value for key, value in plan["jobs"][0].items() if key != "request_id"}
                body["warmup"] = False; plan["jobs"][0] = identified(body)
            elif mutation == "duplicate": plan["jobs"][1] = plan["jobs"][0]
            elif mutation == "omit_session": del plan["sessions"][0]
            else: plan["unexpected"] = True
            with self.subTest(mutation=mutation), self.assertRaises(ACCOUNTING.AccountingError):
                ACCOUNTING.validate_benchmark_session_plan(plan, **params)
        ACCOUNTING.validate_benchmark_session_plan(ACCOUNTING.build_benchmark_session_plan(**inputs()), **inputs())

    def test_failed_warmup_or_measured_attempt_preserves_prior_successes_and_suffix(self):
        for count in (2, 7):
            for state in ("failed", "timed_out", "incomplete"):
                case = dispatch_case(count, state)
                ACCOUNTING.validate_benchmark_session_dispatch_prefix(**case)
                self.assertEqual(sum(row["state"] == "succeeded" for row in case["job_states"][1:-1]), count - 1)
                self.assertEqual(sum(row["state"] == "not_started" for row in case["job_states"][1:-1]), 800 - count)

    def test_every_later_full_campaign_start_after_unsuccessful_attempt_rejects(self):
        for count in (2, 7, 8, 16, 160, 800):
            for state in ("failed", "timed_out", "incomplete"):
                case = dispatch_case(count, state)
                case["started_request_ids"].append(case["full_jobs"][1 + count]["request_id"])
                case["job_states"][1 + count]["state"] = "succeeded"
                with self.subTest(count=count, state=state), self.assertRaisesRegex(ACCOUNTING.AccountingError, "after its unsuccessful attempt"):
                    ACCOUNTING.validate_benchmark_session_dispatch_prefix(**case)

    def test_unsuccessful_nonbenchmark_prefix_cannot_start_a_session(self):
        for state in ("failed", "timed_out", "incomplete"):
            case = dispatch_case(1)
            case["job_states"][0]["state"] = state
            with self.subTest(state=state), self.assertRaisesRegex(ACCOUNTING.AccountingError, "after its unsuccessful attempt"):
                ACCOUNTING.validate_benchmark_session_dispatch_prefix(**case)
        unopened = dispatch_case(0)
        unopened["started_request_ids"] = []
        for row in unopened["job_states"]: row["state"] = "not_started"
        ACCOUNTING.validate_benchmark_session_dispatch_prefix(**unopened)

    def test_full_prefix_and_exact_reduced_outcome_inventory_are_required(self):
        for mutation in ("missing_start", "reorder_start", "missing_state", "reorder_state", "duplicate_state", "unknown_state", "false_not_started", "changed_job", "missing_warmup"):
            case = dispatch_case(7)
            if mutation == "missing_start": del case["started_request_ids"][1]
            elif mutation == "reorder_start": case["started_request_ids"][0], case["started_request_ids"][1] = case["started_request_ids"][1], case["started_request_ids"][0]
            elif mutation == "missing_state": case["job_states"].pop()
            elif mutation == "reorder_state": case["job_states"][0], case["job_states"][1] = case["job_states"][1], case["job_states"][0]
            elif mutation == "duplicate_state": case["job_states"][1] = case["job_states"][0]
            elif mutation == "unknown_state": case["job_states"][0]["state"] = "probably_succeeded"
            elif mutation == "false_not_started": case["job_states"][0]["state"] = "not_started"
            elif mutation == "changed_job": case["full_jobs"][1]["session_attempt_index"] = 42
            else: del case["full_jobs"][1]
            with self.subTest(mutation=mutation), self.assertRaises(ACCOUNTING.AccountingError):
                ACCOUNTING.validate_benchmark_session_dispatch_prefix(**case)
        completed = dispatch_case(800)
        completed["started_request_ids"].append(completed["full_jobs"][-1]["request_id"])
        completed["job_states"][-1]["state"] = "succeeded"
        ACCOUNTING.validate_benchmark_session_dispatch_prefix(**completed)


if __name__ == "__main__":
    unittest.main()
