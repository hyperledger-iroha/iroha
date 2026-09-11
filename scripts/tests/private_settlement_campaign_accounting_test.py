"""Pure registered campaign accounting controls; no process or network execution.

The small records below are synthetic protocol fixtures. They exercise the real
canonical reducer, not release measurement qualification or a benchmark run.
"""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location(
    "campaign_accounting_under_test", Path(__file__).resolve().parents[1] / "private_settlement_attempt_accounting.py"
)
assert SPEC is not None and SPEC.loader is not None
ACCOUNTING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ACCOUNTING)


def raw(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, allow_nan=False) + "\n").encode()


def binding(value: bytes) -> dict:
    return {"sha256": hashlib.sha256(value).hexdigest(), "bytes": len(value)}


def changed(value: bytes, update) -> bytes:
    document = json.loads(value)
    update(document)
    return raw(document)


def fixture(campaign_states: list[list[str]], *, cohorts=None, fault_prefix=False, plan_updates=None):
    """Construct protocol-only records with exact actual producer field names."""

    policy = {"timeout_scope": ACCOUNTING.TIMEOUT_SCOPE, "outer_timeout_ms": 3_600_000,
              "rust_deadline_budgets_ms": {stage: 300_000 for stage in ACCOUNTING.DEADLINE_STAGES}}
    plans = []
    for campaign_index, states in enumerate(campaign_states):
        jobs = []
        if fault_prefix:
            body = {"kind": "fault", "participants": 3, "seed": 1, "run": 0,
                    "configuration_sha256": "e" * 64}
            jobs.append({"request_id": hashlib.sha256(ACCOUNTING.accounting_canonical_bytes(body)).hexdigest(), **body})
        for index, _ in enumerate(states):
            profile, participants, warmup = ("private", 3, False) if cohorts is None else cohorts[index]
            body = {"kind": "benchmark", "profile": profile, "participants": participants,
                    "seed": 1, "run": index, "warmup": warmup, "configuration_sha256": "e" * 64}
            jobs.append({"request_id": hashlib.sha256(ACCOUNTING.accounting_canonical_bytes(body)).hexdigest(), **body})
        plan = {
            "version": 1, "protocol": ACCOUNTING.PROTOCOL, "commit": "c" * 40,
            "worktree_clean": True, "publication_evidence": False, "execution_required": True,
            "harness": {"sha256": "f" * 64, "bytes": 200}, "harness_contract": {},
            "benchmark_baseline": None,
            "hardware": {"path": "hardware-description-v1.json", "sha256": "a" * 64,
                         "bytes": 100, "profile_sha256": "b" * 64},
            "canary_manifest": {}, "configuration_manifest": {}, "requirements": {},
            "jobs": jobs, "benchmark_accounting": policy,
        }
        if plan_updates and campaign_index in plan_updates:
            plan.update(copy.deepcopy(plan_updates[campaign_index]))
        plans.append(raw(plan))
    scope = raw({"version": 1, "protocol": ACCOUNTING.PROTOCOL, "scope_id": "1" * 64,
                 "previous_scope_sha256": None, "registered_ns": 1_000,
                 "stopping_policy": "fail_fast", "deadline_policy": policy,
                 "campaigns": [{"campaign_id": f"campaign-{index}", "plan": binding(plan)}
                               for index, plan in enumerate(plans)]})
    campaigns, successful = [], []
    for campaign_index, (plan_raw, states) in enumerate(zip(plans, campaign_states)):
        plan = json.loads(plan_raw)
        campaign_id = f"campaign-{campaign_index}"
        base = {"scope_sha256": binding(scope)["sha256"], "campaign_id": campaign_id,
                "plan_sha256": binding(plan_raw)["sha256"]}
        attempts, started_ids = [], []
        if fault_prefix:
            started_ids.append(plan["jobs"][0]["request_id"])
        for index, state in enumerate(states):
            ordinal = index + 1 + int(fault_prefix)
            job = plan["jobs"][ordinal - 1]
            packet = {key: None for key in ACCOUNTING.PACKET_FIELDS}
            packet["request_id"] = job["request_id"]
            attempts.append(packet)
            if state == "not_started":
                continue
            attempt_id = ACCOUNTING.registered_attempt_id(base["scope_sha256"], campaign_id, base["plan_sha256"], job["request_id"])
            nonce = hashlib.sha256(f"{campaign_id}:{ordinal}".encode()).hexdigest()
            identity = {**base, "attempt_id": attempt_id, "request_id": job["request_id"], "invocation_nonce": nonce}
            request = {"version": 1, "protocol": ACCOUNTING.PROTOCOL, "kind": "benchmark",
                       **{key: job[key] for key in ("request_id", "participants", "seed", "run", "configuration_sha256")},
                       "invocation_nonce": nonce, "commit": plan["commit"],
                       "hardware_sha256": plan["hardware"]["sha256"],
                       "hardware_profile_sha256": plan["hardware"]["profile_sha256"],
                       "payload": {"profile": job["profile"], "warmup": job["warmup"]}}
            packet["request"] = raw(request)
            request_binding = binding(packet["request"])
            start = {"version": 1, "protocol": ACCOUNTING.PROTOCOL, **identity,
                     "command": ["canonical-harness", "--aps-request", "request.json"],
                     "request": request_binding, "harness": plan["harness"],
                     "timeout_seconds": 3_600, "started_ns": 2_000 + ordinal}
            packet["started"] = raw(start)
            started_ids.append(job["request_id"])
            if state == "missing_process":
                continue
            process = {"version": 1, "protocol": ACCOUNTING.PROTOCOL, **identity,
                       "finished_ns": 4_000 + ordinal, "pid": 100 + ordinal, "exit_code": 0,
                       "timed_out": False, "error": None, "passed": True, "retained_files": [],
                       "completion_kind": "exited", "elapsed_ms": 300_010,
                       "owned_process_group_gone": True, "bindings_unchanged": True}
            if state in {"outer_deadline", "spawn_failed", "interrupted"}:
                process.update(passed=False, error="private diagnostic containing timed out")
                if state == "outer_deadline":
                    process.update(completion_kind="outer_deadline", elapsed_ms=3_600_001, timed_out=True, exit_code=-15)
                elif state == "spawn_failed":
                    process.update(completion_kind="spawn_failed", pid=None, exit_code=None)
                else:
                    process.update(completion_kind="interrupted", exit_code=None)
                packet["process"] = raw(process)
                continue
            if state in {"failed", "timed_out", "missing_terminal", "build_failed", "missing_adapter"}:
                process.update(exit_code=2, passed=False, error="private diagnostic containing timed out")
            packet["process"] = raw(process)
            if state == "missing_adapter":
                continue
            payload = {"stages_ms": {"end_to_end": 1.0, "global_finality": 0.5},
                       "throughput_bundles_per_second": 1.0, "cpu_seconds": 0.5,
                       "peak_rss_bytes": 100, "network_bytes": 200, "proof_bytes": 300,
                       "receipt_bytes": 400, "storage_growth_bytes": 500,
                       "finalized_receipt_observed": True, "successful_leg_applications": job["participants"],
                       "each_leg_applied_exactly_once": True, "partial_visible_observations": 0,
                       "partial_spendable_observations": 0}
            inner = {"version": 1, "protocol": ACCOUNTING.PROTOCOL,
                     **{key: request[key] for key in ("request_id", "invocation_nonce", "commit", "participants")},
                     "request_sha256": request_binding["sha256"], "mandatory_signed_rs16_da_rbc": True,
                     "signed_rs16_da_observations": 16, "authenticated_message_control": True,
                     "process_inventory": [], "payload": payload}
            kind = state if state in {"failed", "timed_out"} else "succeeded"
            outcome = ({"kind": "succeeded", "result": inner} if kind == "succeeded" else
                       {"kind": "failed", "stage": "benchmark_worker", "reason": "execution_error"}
                       if kind == "failed" else
                       {"kind": "timed_out", "stage": "private_receipt", "budget_ms": 300_000, "elapsed_ms": 300_001})
            terminal = {"version": 1, "protocol": ACCOUNTING.PROTOCOL,
                        **{key: request[key] for key in ("request_id", "invocation_nonce", "commit", "participants")},
                        "request_sha256": request_binding["sha256"], "elapsed_ms": 300_002, "outcome": outcome}
            adapter = {"version": 1, "protocol": ACCOUNTING.PROTOCOL,
                       **{key: request[key] for key in ("request_id", "invocation_nonce", "commit", "participants")},
                       "request_sha256": request_binding["sha256"], "elapsed_ms": 300_005,
                       "phase": "measurement_validation" if kind == "succeeded" else "terminal_validation",
                       "exit_code": 0 if kind == "succeeded" else 101,
                       "status": kind, "reason": "rust_" + kind, "rust_terminal": None}
            if state == "missing_terminal":
                adapter.update(status="incomplete", reason="rust_terminal_missing", phase="terminal_validation", exit_code=101)
            elif state == "build_failed":
                adapter.update(status="failed", reason="validator_build_failed", phase="validator_build", exit_code=101)
            else:
                packet["rust_terminal"] = raw(terminal)
                adapter["rust_terminal"] = binding(packet["rust_terminal"])
            packet["adapter"] = raw(adapter)
            if state in {"failed", "timed_out", "missing_terminal", "build_failed"}:
                continue
            response = {key: value for key, value in inner.items() if key != "request_sha256"}
            response.update({"kind": "benchmark", "passed": True,
                             **{key: request[key] for key in ("hardware_sha256", "hardware_profile_sha256", "configuration_sha256")}})
            packet["response"] = raw(response)
            packet["response_outcome"] = raw({"version": 1, "protocol": ACCOUNTING.PROTOCOL,
                                               "passed": True, "response": binding(packet["response"])})
            if state == "missing_validation":
                continue
            validation = {"version": 1, "protocol": ACCOUNTING.PROTOCOL, **identity,
                          "ordinal": ordinal, "kind": "benchmark", "finished_ns": 5_000 + ordinal,
                          "passed": True, "validation_kind": "accepted", "response": binding(packet["response"])}
            if state == "publication_failed":
                validation.pop("response")
                validation.update(passed=False, validation_kind="publication_failed", stage="publication", error="private IO diagnostic")
            else:
                sample = {"version": 1, "protocol": ACCOUNTING.PROTOCOL, "attempt_id": attempt_id,
                          "commit": plan["commit"], "hardware_sha256": plan["hardware"]["sha256"],
                          "hardware_profile_sha256": plan["hardware"]["profile_sha256"],
                          **{key: job[key] for key in ("profile", "participants", "seed", "run", "warmup", "configuration_sha256")},
                          "stages_ms": payload["stages_ms"],
                          **{key: float(payload[key]) for key in ("throughput_bundles_per_second", "cpu_seconds", "peak_rss_bytes",
                                                              "network_bytes", "proof_bytes", "receipt_bytes", "storage_growth_bytes")}}
                packet["sample"] = raw(sample)
                validation["sample"] = binding(packet["sample"])
                successful.append(packet["sample"])
            packet["validation"] = raw(validation)
        closure = {"version": 1, "protocol": ACCOUNTING.PROTOCOL, **base, "closed_ns": 10_000,
                   "quiescent": True, "started_request_ids": started_ids,
                   "reason": "completed" if all(state == "succeeded" for state in states) else "fail_fast"}
        campaigns.append({"campaign_id": campaign_id, "plan": plan_raw, "closure": raw(closure), "attempts": attempts})
    return scope, campaigns, successful


class RegisteredCampaignAccountingTests(unittest.TestCase):
    """Exercise the real pure reducer across the eight design acceptance groups."""

    def reduce(self, value):
        return ACCOUNTING.reduce_registered_scope(*value)

    def test_disjoint_outcomes_use_typed_deadlines_and_keep_failures(self):
        result = self.reduce(fixture([["succeeded"], ["failed"], ["timed_out"], ["outer_deadline"]]))
        self.assertEqual(result["counts"], {"planned": 4, "attempted": 4, "not_started": 0,
                                         "succeeded": 1, "failed": 1, "timed_out": 2, "incomplete": 0})
        self.assertEqual(result["timeout_scope"], ACCOUNTING.TIMEOUT_SCOPE)
        self.assertNotIn("private diagnostic", json.dumps(result))

    def test_stage_deadlines_must_match_the_registered_budget(self):
        for stage in ACCOUNTING.DEADLINE_STAGES:
            value = fixture([["timed_out"]])
            packet = value[1][0]["attempts"][0]
            packet["rust_terminal"] = changed(packet["rust_terminal"], lambda row: row["outcome"].update(stage=stage))
            packet["adapter"] = changed(packet["adapter"], lambda row: row.update(rust_terminal=binding(packet["rust_terminal"])))
            self.assertEqual(self.reduce(value)["counts"]["timed_out"], 1)
        for budget in (0, 299_999):
            value = fixture([["timed_out"]])
            packet = value[1][0]["attempts"][0]
            packet["rust_terminal"] = changed(packet["rust_terminal"], lambda row: row["outcome"].update(budget_ms=budget))
            packet["adapter"] = changed(packet["adapter"], lambda row: row.update(rust_terminal=binding(packet["rust_terminal"])))
            with self.assertRaises(ACCOUNTING.AccountingError):
                self.reduce(value)

    def test_build_spawn_and_worker_failure_remain_job_failures(self):
        value = fixture([["build_failed"], ["spawn_failed"], ["failed"], ["publication_failed"]])
        self.assertEqual(self.reduce(value)["counts"]["failed"], 4)
        self.assertEqual(self.reduce(value)["counts"]["timed_out"], 0)
        packet = value[1][2]["attempts"][0]
        packet["rust_terminal"] = changed(packet["rust_terminal"], lambda row: row["outcome"].update(reason="worker_panic"))
        packet["adapter"] = changed(packet["adapter"], lambda row: row.update(rust_terminal=binding(packet["rust_terminal"])))
        self.assertEqual(self.reduce(value)["counts"]["failed"], 4)

    def test_missing_terminals_and_interruption_are_incomplete(self):
        value = fixture([["missing_process"], ["missing_adapter"], ["missing_terminal"], ["missing_validation"], ["interrupted"]])
        result = self.reduce(value)
        self.assertEqual(result["counts"]["incomplete"], 5)
        self.assertFalse(result["accounting_complete"])
        value = fixture([["succeeded"]])
        packet = value[1][0]["attempts"][0]
        packet["process"] = changed(packet["process"], lambda row: row.update(owned_process_group_gone=False))
        value[1][0]["closure"] = changed(value[1][0]["closure"], lambda row: row.update(reason="recovered_interruption"))
        value[2].clear()
        self.assertEqual(self.reduce(value)["counts"]["incomplete"], 1)
        value = fixture([["timed_out"]])
        value[1][0]["attempts"][0]["rust_terminal"] = None
        self.assertEqual(self.reduce(value)["counts"]["incomplete"], 1)

    def test_failfast_and_pre_start_failure_keep_the_planned_denominator(self):
        result = self.reduce(fixture([["succeeded", "failed", "not_started"]]))
        self.assertEqual((result["counts"]["planned"], result["counts"]["attempted"], result["counts"]["not_started"]), (3, 2, 1))
        value = fixture([["not_started", "not_started"]], fault_prefix=True)
        result = self.reduce(value)
        self.assertEqual((result["counts"]["planned"], result["counts"]["attempted"], result["counts"]["not_started"]), (2, 0, 2))
        value[1][0]["closure"] = changed(value[1][0]["closure"], lambda row: row.update(quiescent=False))
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.reduce(value)
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.reduce(fixture([["failed", "succeeded"]]))

    def test_plan_registration_inventory_and_precomputed_counters_are_rejected(self):
        for mutation in ("plan", "campaign_omitted", "job_omitted", "job_duplicate", "counter", "predecessor"):
            value = list(fixture([["succeeded"], ["failed"]]))
            if mutation == "plan":
                value[1][0]["plan"] += b" "
            elif mutation == "campaign_omitted":
                value[1].pop()
            elif mutation == "job_omitted":
                value[1][0]["attempts"].clear()
            elif mutation == "job_duplicate":
                value[1][0]["attempts"] *= 2
            elif mutation == "counter":
                value[0] = changed(value[0], lambda row: row.update(attempted=0))
            else:
                value[0] = changed(value[0], lambda row: row.update(previous_scope_sha256="a" * 64))
            with self.subTest(mutation=mutation), self.assertRaises(ACCOUNTING.AccountingError):
                self.reduce(value)

    def test_fail_fast_also_rejects_a_nonbenchmark_tail_after_unsuccessful_benchmark(self):
        for state in ("succeeded", "failed", "timed_out", "missing_process", "publication_failed"):
            with self.subTest(state=state):
                value = fixture([[state]])
                jobs = json.loads(value[1][0]["plan"])["jobs"]
                body = {"kind": "leakage", "participants": 3, "seed": 1,
                        "run": 0, "configuration_sha256": "e" * 64}
                tail = {"request_id": hashlib.sha256(ACCOUNTING.accounting_canonical_bytes(body)).hexdigest(), **body}
                value = fixture([[state]], plan_updates={0: {"jobs": [*jobs, tail]}})
                value[1][0]["closure"] = changed(value[1][0]["closure"],
                    lambda row: row["started_request_ids"].append(tail["request_id"]))
                if state == "succeeded":
                    self.assertEqual(self.reduce(value)["counts"]["succeeded"], 1)
                else:
                    with self.assertRaisesRegex(ACCOUNTING.AccountingError, "full-plan job after fail-fast"):
                        self.reduce(value)

    def test_mixed_failed_predecessor_source_is_rejected(self):
        baseline = self.reduce(fixture([["failed"], ["succeeded"]]))
        self.assertEqual((baseline["counts"]["failed"], baseline["counts"]["succeeded"]), (1, 1))
        value = fixture([["failed"], ["succeeded"]], plan_updates={0: {"commit": "d" * 40}})
        with self.assertRaisesRegex(ACCOUNTING.AccountingError, "scope campaign source/hardware"):
            self.reduce(value)

    def test_mixed_failed_predecessor_hardware_is_rejected(self):
        hardware = {"path": "hardware-description-v1.json", "sha256": "d" * 64,
                    "bytes": 101, "profile_sha256": "e" * 64}
        value = fixture([["failed"], ["succeeded"]], plan_updates={0: {"hardware": hardware}})
        with self.assertRaisesRegex(ACCOUNTING.AccountingError, "scope campaign source/hardware"):
            self.reduce(value)

    def test_request_nonce_identity_and_changed_inputs_are_rejected(self):
        for field in ("scope_sha256", "campaign_id", "plan_sha256", "attempt_id", "invocation_nonce", "request_id"):
            value = fixture([["failed"]])
            packet = value[1][0]["attempts"][0]
            packet["process"] = changed(packet["process"], lambda row: row.update({field: "different"}))
            with self.subTest(field=field), self.assertRaises(ACCOUNTING.AccountingError):
                self.reduce(value)
        value = fixture([["failed"]])
        packet = value[1][0]["attempts"][0]
        packet["request"] += b" "
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.reduce(value)

    def test_reused_nonce_is_rejected_after_all_local_bindings_are_updated(self):
        value = fixture([["succeeded"], ["succeeded"]])
        first, second = value[1][0]["attempts"][0], value[1][1]["attempts"][0]
        nonce = json.loads(first["request"])["invocation_nonce"]
        for key in ("request", "adapter", "rust_terminal", "response", "response_outcome"):
            second[key] = first[key]
        second["started"] = changed(second["started"], lambda row: row.update(
            invocation_nonce=nonce, request=binding(second["request"])))
        second["process"] = changed(second["process"], lambda row: row.update(invocation_nonce=nonce))
        second["validation"] = changed(second["validation"], lambda row: row.update(
            invocation_nonce=nonce, response=binding(second["response"])))
        with self.assertRaisesRegex(ACCOUNTING.AccountingError, "nonce is reused"):
            self.reduce(value)
        value = fixture([["failed"]])
        packet = value[1][0]["attempts"][0]
        packet["process"] = changed(packet["process"], lambda row: row.update(bindings_unchanged=False))
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.reduce(value)

    def test_successful_rows_join_exactly_once_with_no_unsuccessful_metrics(self):
        for mutation in ("omitted", "duplicate", "substituted", "unsuccessful"):
            value = fixture([["succeeded"], ["failed"]])
            if mutation == "omitted":
                value[2].clear()
            elif mutation == "duplicate":
                value[2].append(value[2][0])
            elif mutation == "substituted":
                value[2][0] = changed(value[2][0], lambda row: row.update(cpu_seconds=999.0))
            else:
                failed_id = ACCOUNTING.registered_attempt_id(binding(value[0])["sha256"], "campaign-1",
                    binding(value[1][1]["plan"])["sha256"], value[1][1]["attempts"][0]["request_id"])
                value[2].append(changed(value[2][0], lambda row: row.update(attempt_id=failed_id)))
            with self.subTest(mutation=mutation), self.assertRaises(ACCOUNTING.AccountingError):
                self.reduce(value)

    def test_warmup_profile_and_participant_cohorts_remain_separate(self):
        cohorts = [("private", 3, True), ("private", 3, False), ("transparent_control", 2, False)]
        result = self.reduce(fixture([["succeeded"] * 3, ["succeeded", "failed", "not_started"]], cohorts=cohorts))
        counts = {(row["profile"], row["participants"], row["warmup"]): row["counts"] for row in result["cohorts"]}
        self.assertEqual(counts[("private", 3, True)]["succeeded"], 2)
        self.assertEqual(counts[("private", 3, False)]["failed"], 1)
        self.assertEqual(counts[("transparent_control", 2, False)]["not_started"], 1)
        self.assertEqual(len({row["attempt_id"] for row in result["rows"]}), 6)

    def test_outer_deadline_precedes_unvalidated_early_success(self):
        value = fixture([["missing_validation"]])
        packet = value[1][0]["attempts"][0]
        packet["process"] = changed(packet["process"], lambda row: row.update(
            completion_kind="outer_deadline", elapsed_ms=3_600_001, timed_out=True, passed=False, exit_code=-15))
        self.assertEqual(self.reduce(value)["counts"]["timed_out"], 1)
        value = fixture([["succeeded"]])
        packet = value[1][0]["attempts"][0]
        packet["process"] = changed(packet["process"], lambda row: row.update(
            completion_kind="outer_deadline", elapsed_ms=3_600_001, timed_out=True, passed=False, exit_code=-15))
        with self.assertRaises(ACCOUNTING.AccountingError):
            self.reduce(value)

    def test_malformed_and_contradictory_evidence_is_not_incomplete(self):
        for mutation in ("invalid_adapter", "contradictory_exit", "unknown_completion", "duplicate_json", "orphan_bad_terminal"):
            value = fixture([["failed"]])
            packet = value[1][0]["attempts"][0]
            if mutation == "invalid_adapter":
                packet["adapter"] = changed(packet["adapter"], lambda row: row.update(status="invalid", reason="rust_terminal_invalid"))
            elif mutation == "contradictory_exit":
                packet["process"] = changed(packet["process"], lambda row: row.update(exit_code=0, passed=True))
            elif mutation == "unknown_completion":
                packet["process"] = changed(packet["process"], lambda row: row.update(completion_kind="stderr says timeout"))
            elif mutation == "duplicate_json":
                packet["process"] = b'{"version":1,"version":1}'
            else:
                packet["adapter"] = None
                packet["rust_terminal"] = raw({"unexpected": True})
            with self.subTest(mutation=mutation), self.assertRaises(ACCOUNTING.AccountingError):
                self.reduce(value)

    def test_reduction_is_deterministic_and_does_not_mutate_inputs(self):
        value = fixture([["succeeded"], ["failed"]])
        before = copy.deepcopy(value)
        first = self.reduce(value)
        self.assertEqual(first, self.reduce(value))
        self.assertEqual(value, before)
        # Public report/verifier callers receive the same pure projection.
        self.assertEqual(ACCOUNTING.accounting_canonical_bytes(first),
                         ACCOUNTING.accounting_canonical_bytes(self.reduce(copy.deepcopy(value))))


if __name__ == "__main__":
    unittest.main()
