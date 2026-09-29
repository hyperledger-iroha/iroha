"""Registered campaign accounting over authenticated retained-session fixtures.

These synthetic records exercise the actual canonical reducer and independent
semantic replay callback. They launch no process and claim no qualification.
"""
from __future__ import annotations

import copy
import json
import unittest

from retained_accounting_fixture import Fixture, accounting, control


def scope_fixture(states, overrides=None):
    """Bind every campaign to one complete scope before encoding its records."""
    overrides = overrides or {}
    slots = [
        {"campaign_id": f"campaign-{index}", "plan": accounting.accounting_file_binding(
            Fixture(setup_failure=True, plan_overrides=overrides.get(index)).plan_raw)}
        for index in range(len(states))
    ]
    return [Fixture(kinds, campaign_id=f"campaign-{index}", registered_campaigns=slots,
                    plan_overrides=overrides.get(index), worker_birth=10 + index)
            for index, kinds in enumerate(states)]


def reduce_scope(fixtures, *, packets=None, samples=None, scope=None):
    """Retain mandatory source image, command and recomputed sample ownership."""
    owners = {f.base["campaign_id"]: f for f in fixtures}
    return accounting.reduce_registered_scope(
        fixtures[0].scope_raw if scope is None else scope,
        [f.packet for f in fixtures] if packets is None else packets,
        [sample for f in fixtures for sample in f.samples] if samples is None else samples,
        worker_command=fixtures[0].command,
        worker_image=fixtures[0].image,
        validate_success=lambda identity, attempt, bound, records:
            owners[identity["campaign_id"]].recompute(identity, attempt, bound, records),
    )


class RegisteredCampaignAccountingTests(unittest.TestCase):
    """Cross-campaign boundaries supplement the retained attempt/lifetime suite."""

    def reject(self, fixtures, **kwargs):
        with self.assertRaises((accounting.AccountingError, control.SessionProtocolError)):
            reduce_scope(fixtures, **kwargs)

    def test_disjoint_outcomes_preserve_complete_planned_denominator(self):
        fixtures = scope_fixture([("succeeded",), ("failed",), ("timed_out",), ("incomplete",)])
        result = reduce_scope(fixtures)
        self.assertEqual(result["counts"], dict(planned=3200, attempted=4, not_started=3196,
                                               succeeded=1, failed=1, timed_out=1, incomplete=1))
        self.assertEqual(result["timeout_scope"], accounting.TIMEOUT_SCOPE)
        self.assertFalse(result["accounting_complete"])
        self.assertNotIn("private diagnostic", json.dumps(result))

    def test_complete_session_and_failed_successor_keep_every_sample(self):
        fixtures = scope_fixture([("succeeded",) * 8, ("failed",)])
        result = reduce_scope(fixtures)
        self.assertEqual(result["counts"]["succeeded"], 8)
        self.assertEqual(result["counts"]["failed"], 1)
        self.assertEqual(sum(f.replays for f in fixtures), 8)

    def test_missing_or_duplicate_registered_campaign_is_rejected(self):
        for operation in ("missing", "duplicate", "reverse"):
            fixtures = scope_fixture([("succeeded",), ("failed",)])
            packets = [f.packet for f in fixtures]
            if operation == "missing": packets.pop()
            elif operation == "duplicate": packets.append(packets[0])
            else: packets.reverse()
            self.reject(fixtures, packets=packets)

    def test_exact_plan_bytes_and_unknown_packet_fields_are_rejected(self):
        for mutation in ("plan", "session_missing", "counter", "one_shot"):
            fixtures = scope_fixture([("succeeded",)])
            packet = fixtures[0].packet
            if mutation == "plan": packet["plan"] += b" "
            elif mutation == "session_missing": packet["sessions"].pop()
            elif mutation == "counter": packet["counts"] = {"succeeded": 100}
            else: packet["attempts"] = []
            self.reject(fixtures)

    def test_mixed_failed_predecessor_source_is_rejected(self):
        fixtures = scope_fixture([("failed",), ("succeeded",)], {1: {"commit": "f" * 40}})
        with self.assertRaisesRegex(accounting.AccountingError, "scope campaign environment"):
            reduce_scope(fixtures)

    def test_mixed_failed_predecessor_hardware_is_rejected(self):
        fixtures = scope_fixture([("failed",), ("succeeded",)],
            {1: {"hardware": {"sha256": "f" * 64, "profile_sha256": "c" * 64}}})
        with self.assertRaisesRegex(accounting.AccountingError, "scope campaign environment"):
            reduce_scope(fixtures)

    def test_source_admitted_worker_identity_is_required(self):
        for key, value in (("sha256", "1" * 64), ("bytes", 101)):
            fixtures = scope_fixture([("succeeded",)])
            fixtures[0].image[key] = value
            self.reject(fixtures)
        fixtures = scope_fixture([("succeeded",)])
        fixtures[0].command = ["substituted-worker"]
        self.reject(fixtures)

    def test_scope_cannot_relabel_bound_request_or_predecessor(self):
        for key, value in (("scope_id", "e" * 64), ("previous_scope_sha256", "e" * 64),
                           ("registered_ns", 0), ("stopping_policy", "continue")):
            fixtures = scope_fixture([("succeeded",)])
            scope = copy.deepcopy(fixtures[0].scope)
            scope[key] = value
            self.reject(fixtures, scope=accounting.accounting_canonical_bytes(scope))

    def test_success_rows_join_exactly_once_and_are_recomputed(self):
        for mutation in ("missing", "duplicate", "changed", "unsuccessful"):
            fixtures = scope_fixture([("succeeded",), ("failed",)])
            samples = [sample for f in fixtures for sample in f.samples]
            if mutation == "missing": samples.clear()
            elif mutation == "duplicate": samples.append(samples[0])
            elif mutation == "changed": samples[0] = b"{}"
            else: samples.append(accounting.accounting_canonical_bytes({"campaign_id": "campaign-1"}))
            self.reject(fixtures, samples=samples)

    def test_warmup_and_measured_cohorts_remain_separate(self):
        result = reduce_scope(scope_fixture([("succeeded",) * 6 + ("failed",)]))
        warm = next(c for c in result["cohorts"] if c["profile"] == "private" and c["participants"] == 2 and c["warmup"])
        measured = next(c for c in result["cohorts"] if c["profile"] == "private" and c["participants"] == 2 and not c["warmup"])
        self.assertEqual(warm["counts"]["succeeded"], 5)
        self.assertEqual(measured["counts"]["succeeded"], 1)
        self.assertEqual(measured["counts"]["failed"], 1)

    def test_deadlines_require_the_registered_policy(self):
        fixtures = scope_fixture([("timed_out",)])
        policy = copy.deepcopy(fixtures[0].policy)
        policy["outer_timeout_ms"] += 1
        self.reject(scope_fixture([("timed_out",)], {0: {"benchmark_accounting": policy}}))

    def test_reduction_is_deterministic_without_input_mutation(self):
        fixtures = scope_fixture([("succeeded",), ("failed",)])
        before = [(dict(f.store.values), f.plan_raw, f.scope_raw, list(f.samples)) for f in fixtures]
        first = reduce_scope(fixtures)
        self.assertEqual(first, reduce_scope(fixtures))
        self.assertEqual(before, [(dict(f.store.values), f.plan_raw, f.scope_raw, list(f.samples)) for f in fixtures])


if __name__ == "__main__":
    unittest.main()
