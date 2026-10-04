"""Unit tests of the multilane scaling gate (``scripts/sumeragi_scaling_gate.py``).

Pure parts only, on synthetic inputs: exact decimal handling and option validation, the
ten-run evidence ledger, trace measurement (throughput in the measurement window, offer to
Applied latency, nearest-rank p95), capture reductions, lane usage, the lane policy and the
verdict arithmetic against the F13 thresholds (3/2 throughput, 5/4 p95 latency), and the
``--analyze`` path over a synthetic finished run directory. No network is started.
"""

from __future__ import annotations

import dataclasses
import hashlib
import importlib.util
import json
import sys
from fractions import Fraction
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "sumeragi_scaling_gate.py"
SPEC = importlib.util.spec_from_file_location("sumeragi_scaling_gate_under_test", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
gate = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = gate
SPEC.loader.exec_module(gate)

NS = gate.NS


def options(**changes):
    values = dict(
        pairs=1,
        accounts=8,
        offered_load_tps="20",
        warmup_s="8",
        measurement_s="40",
        drain_s="40",
        max_submission_lag_ms="12.5",
        resource_interval_ms=2000,
        resource_timeout_ms=1000,
        resource_max_start_lag_ms=500,
        status_body_bytes=32 * 1024,
        metrics_body_bytes=512 * 1024,
        journal_bytes=48 * gate.MIB,
        trace_bytes=8 * gate.MIB,
        receipt_bytes=gate.MIB,
        manifest_bytes=gate.MIB,
        report_bytes=4 * gate.MIB,
        min_latency_samples=100,
        seed_namespace="unit",
        fund="1000",
        base_api_port=19080,
        base_p2p_port=12337,
        storage_budget_mb=1024,
        preparation_lookahead=256,
        preparation_concurrency=8,
        preparation_ahead_ms=2000,
        max_submissions=256,
        max_in_flight=4096,
        max_status_requests=64,
        poll_interval_ms=100,
        journal_capacity=4096,
    )
    values.update(changes)
    return gate.Options(**values)


# ---------------------------------------------------------------------------------------------
# Exact arithmetic
# ---------------------------------------------------------------------------------------------


def test_p95_is_nearest_rank():
    assert gate.p95_nearest_rank([5]) == 5
    assert gate.p95_nearest_rank(list(range(1, 101))) == 95
    assert gate.p95_nearest_rank(list(range(1, 102))) == 96
    assert gate.p95_nearest_rank(list(range(100, 0, -1))) == 95
    assert gate.p95_nearest_rank([1] * 19 + [1000]) == 1
    assert gate.p95_nearest_rank([1] * 18 + [1000, 1000]) == 1000
    with pytest.raises(ValueError):
        gate.p95_nearest_rank([])


def test_nearest_rank_percentiles():
    values = list(range(1, 201))
    assert gate.percentile_nearest_rank(values, 50) == 100
    assert gate.percentile_nearest_rank(values, 99) == 198
    assert gate.percentile_nearest_rank(values, 100) == 200
    for percent in (0, 101):
        with pytest.raises(ValueError):
            gate.percentile_nearest_rank(values, percent)


def test_median_of_odd_and_even_samples():
    assert gate.median([Fraction(3), Fraction(1), Fraction(2)]) == 2
    assert gate.median([Fraction(4), Fraction(1), Fraction(2), Fraction(3)]) == Fraction(5, 2)
    assert gate.median([Fraction(7, 3)]) == Fraction(7, 3)
    with pytest.raises(ValueError):
        gate.median([])


def test_exact_decimals_and_nanoseconds():
    assert gate.exact_decimal("12.5", "x") == Fraction(25, 2)
    assert gate.ns_of_seconds("0.000000001", "x") == 1
    assert gate.ns_of_seconds("12.5", "x", 1_000_000) == 12_500_000
    for bad in ("", "-1", "1e3", "01", "1.", " 1", "1.0000000001"):
        with pytest.raises(ValueError):
            gate.exact_decimal(bad, "x")
    assert gate.decimal_text(Fraction(25, 2), 6) == "12.5"
    assert gate.decimal_text(Fraction(8333333, 1_000_000), 6) == "8.333333"
    assert gate.decimal_text(Fraction(40), 6) == "40"
    with pytest.raises(ValueError):
        gate.decimal_text(Fraction(1, 3), 6)


def test_collector_schedule_rules():
    # ceil(duration * rate): the collector's cohort count.
    assert gate.cohort_count(40 * NS, Fraction(20)) == 800
    assert gate.cohort_count(NS, Fraction(3, 2)) == 2
    assert gate.cohort_count(0, Fraction(20)) == 0
    # One quarter of the arrival period, floored to whole nanoseconds.
    assert gate.submission_lag_bound_ns(Fraction(20)) == 12_500_000
    assert gate.submission_lag_bound_ns(Fraction(30)) == 8_333_333


def test_pair_seeds_are_shared_by_the_two_variants_and_order_alternates():
    assert gate.pair_seed("ns", 1) == hashlib.sha256(b"ns:1").hexdigest()
    assert gate.pair_seed("ns", 1) != gate.pair_seed("ns", 2)
    assert gate.run_order(3) == [
        (1, "one_lane"), (1, "four_lane"),
        (2, "four_lane"), (2, "one_lane"),
        (3, "one_lane"), (3, "four_lane"),
    ]


# ---------------------------------------------------------------------------------------------
# Options and the evidence ledger
# ---------------------------------------------------------------------------------------------


def test_default_short_plan_is_valid_and_admitted():
    plan = options()
    gate.validate_options(plan)
    ledger = gate.evidence_budget(plan, (gate.budget.StaticFile("source.worker", 1000),))
    assert len(ledger.runs) == 10
    assert ledger.geometry.sample_count == (40 + 40) // 2 + 1
    selected = gate.budget.select_run_budget(ledger, 1, "four_lane")
    assert selected.run.run_receipt.max_bytes == gate.MIB
    assert selected.journal.label == "pair1.four_lane.journal"
    assert ledger.total_bytes <= gate.budget.MAX_TOTAL_BYTES


@pytest.mark.parametrize(
    "changes,message",
    [
        ({"pairs": 0}, "--pairs"),
        ({"pairs": 6}, "--pairs"),
        ({"accounts": 6}, "multiple of four"),
        ({"accounts": 68}, "multiple of four"),
        ({"offered_load_tps": "0"}, "positive"),
        ({"drain_s": "301"}, "--drain"),
        ({"max_submission_lag_ms": "12.500001"}, "quarter"),
        ({"offered_load_tps": "22.5", "max_submission_lag_ms": "10"}, "whole account rounds"),
        ({"measurement_s": "4", "resource_interval_ms": 200}, "latency minimum"),
        ({"resource_timeout_ms": 1001}, "resource timing"),
        ({"measurement_s": "38"}, "geometry"),
        ({"fund": "-1"}, "--fund"),
    ],
)
def test_invalid_plans_are_rejected_before_any_run(changes, message):
    with pytest.raises(ValueError, match=message):
        gate.validate_options(options(**changes))


def test_per_account_effect_bound_is_enforced():
    # 8 accounts x 1024 effects: 60 s x 150 tps = 9000 requests do not fit.
    with pytest.raises(ValueError, match="effects per account"):
        gate.validate_options(
            options(offered_load_tps="150", max_submission_lag_ms="1", warmup_s="20", measurement_s="40")
        )


def test_oversized_allocations_fail_the_ledger():
    with pytest.raises(gate.budget.BudgetError):
        gate.evidence_budget(options(journal_bytes=256 * gate.MIB), (gate.budget.StaticFile("source.worker", 1),))


def test_bounded_json_refuses_to_exceed_its_allocation(tmp_path):
    path = tmp_path / "run.json"
    assert gate.write_bounded_json(path, {"a": 1}, 100) == len(path.read_bytes())
    with pytest.raises(gate.GateError, match="allocation"):
        gate.write_bounded_json(tmp_path / "big.json", {"a": "x" * 200}, 100)
    assert not (tmp_path / "big.json").exists()


# ---------------------------------------------------------------------------------------------
# Trace measurement
# ---------------------------------------------------------------------------------------------


def trace_row(cohort, sequence, offer_ns, latency_ns, height=5):
    digest = hashlib.sha256(f"{cohort}:{sequence}".encode()).hexdigest()
    return {
        "cohort": cohort,
        "sequence": sequence,
        "logical_id": digest,
        "hash": digest.upper(),
        "scheduled_offset_ns": offer_ns,
        "offer_offset_ns": offer_ns,
        "submission_lag_ns": 0,
        "acknowledgment": {"offset_ns": offer_ns + 1, "hash": digest.upper(), "status": "Accepted", "rejection": None},
        "applied": {
            "offset_ns": offer_ns + latency_ns,
            "hash": digest.upper(),
            "scope": "global",
            "resolved_from": "state",
            "status": "Applied",
            "block_height": height,
        },
    }


def synthetic_trace(pair=1, variant="one_lane", seed="ab" * 32, warmup=2, latencies=(), period_ns=NS // 10):
    rows = [trace_row("warmup", index + 1, -10 * NS + index * period_ns, NS) for index in range(warmup)]
    rows += [
        trace_row("measurement", index + 1, index * period_ns, latency, height=10 + index // 10)
        for index, latency in enumerate(latencies)
    ]
    return {
        "schema": gate.TRACE_SCHEMA,
        "pair_index": pair,
        "variant": variant,
        "seed": seed,
        "clock": "monotonic_nanoseconds_relative_to_measurement_start",
        "transactions": rows,
    }


def measure(trace, measurement_ns=10 * NS, warmup=2, requests=None, pair=1, variant="one_lane", seed="ab" * 32):
    return gate.measure_trace(
        trace,
        pair_index=pair,
        variant=variant,
        seed=seed,
        measurement_ns=measurement_ns,
        expected_warmup=warmup,
        expected_measurement=len(trace["transactions"]) - warmup if requests is None else requests,
    )


def test_trace_measurement_counts_only_requests_applied_inside_the_window():
    # 100 requests offered every 100 ms over 10 s; the last 5 apply after the window closes.
    latencies = [NS // 2] * 95 + [2 * NS] * 5
    measured = measure(synthetic_trace(latencies=latencies))
    assert measured.measurement_requests == 100
    # Offers at 9.5..9.9 s with 2 s latency apply at 11.5..11.9 s, after the 10 s window.
    assert measured.committed_in_window == 95
    assert measured.drain_committed == 5
    assert measured.throughput_tps == Fraction(95, 10)
    assert measured.p95_latency_ns == NS // 2
    assert measured.max_latency_ns == 2 * NS
    assert (measured.first_height, measured.last_height) == (10, 19)
    assert len(measured.latencies_ns) == 100
    assert measured.to_json()["throughput_tps"] == {"exact": "19/2", "decimal": 9.5}
    assert measured.to_json()["p50_latency_ms"] == 500.0
    assert measured.to_json()["p99_latency_ms"] == 2000.0


def test_trace_measurement_p95_uses_every_measurement_request_and_no_warmup():
    latencies = [NS] * 94 + [3 * NS] * 6
    measured = measure(synthetic_trace(latencies=latencies, warmup=4), warmup=4)
    assert measured.variant == "one_lane" and measured.pair_index == 1
    assert measured.p95_latency_ns == 3 * NS
    assert measured.warmup_requests == 4


@pytest.mark.parametrize(
    "mutation,message",
    [
        (lambda t: t.update(schema="iroha.sumeragi.other"), "not a scaling"),
        (lambda t: t.update(variant="four_lane"), "another pair"),
        (lambda t: t.update(seed="cd" * 32), "another pair"),
        (lambda t: t["transactions"].pop(), "rows"),
        (lambda t: t["transactions"].reverse(), "order"),
        (lambda t: t["transactions"][3].update(hash=t["transactions"][2]["hash"]), "hash"),
        (lambda t: t["transactions"][3]["applied"].update(scope="local"), "global"),
        (lambda t: t["transactions"][3]["applied"].update(resolved_from="cache"), "global"),
        (lambda t: t["transactions"][3]["acknowledgment"].update(status="Rejected"), "accepted"),
        (lambda t: t["transactions"][3]["applied"].update(offset_ns=t["transactions"][3]["offer_offset_ns"]), "before"),
        (lambda t: t["transactions"][3]["applied"].update(block_height=0), "block_height"),
        (lambda t: t["transactions"][3].update(offer_offset_ns=True), "offer_offset_ns"),
    ],
)
def test_malformed_or_foreign_traces_are_not_measurements(mutation, message):
    trace = synthetic_trace(latencies=[NS] * 10)
    requests = 10
    mutation(trace)
    with pytest.raises(gate.TraceError, match=message):
        measure(trace, requests=requests)


# ---------------------------------------------------------------------------------------------
# Captures, node counters, lanes
# ---------------------------------------------------------------------------------------------


def manifest(kind, sequence, rss=100, queue=3, storage=1000, entries=10, available=True):
    peers = [
        {
            "peer_id": f"peer{index}",
            "status": {"body": {"name": f"{kind}-{sequence:010}-peer-{index:04}-status.body"}, "queue_size": queue},
            "metrics": {"body": {"name": f"{kind}-{sequence:010}-peer-{index:04}-metrics.body"}},
            "process_before": {"rss_bytes": rss + index},
            "process_after": {"rss_bytes": rss + index + 1},
        }
        for index in range(4)
    ]
    return {
        "schema": gate.CAPTURE_SCHEMA,
        "kind": kind,
        "sequence": sequence,
        "available": available,
        "peers": peers,
        "aggregates": {
            "rss_before_bytes": 4 * rss,
            "rss_after_bytes": 4 * rss + 4,
            "queue_size_sum": 4 * queue,
            "queue_size_max": queue,
            "inventory": {"storage_bytes": storage, "represented_entries": entries} if available else None,
        },
    }


def test_resource_maxima_cover_preflight_and_every_sample_through_drain():
    manifests = [manifest("preflight", 0)] + [manifest("sample", seq, rss=100 * seq, queue=seq) for seq in range(1, 5)]
    result = gate.resource_maxima(manifests, expected_samples=4)
    assert result["complete"] is True
    assert result["captures"] == 5
    assert result["maxima"] == {
        "rss_bytes_sum": 1604,
        "rss_bytes_peer": 404,
        "queue_size_sum": 16,
        "queue_size_peer": 4,
        "kura_storage_bytes_sum": 1000,
        "kura_represented_entries_sum": 10,
    }


@pytest.mark.parametrize(
    "manifests",
    [
        [manifest("sample", seq) for seq in range(1, 5)],
        [manifest("preflight", 0)] + [manifest("sample", seq) for seq in (1, 2, 4)],
        [manifest("preflight", 0)] + [manifest("sample", seq, available=seq != 3) for seq in range(1, 5)],
        [manifest("preflight", 0)] + [manifest("sample", seq) for seq in range(1, 4)],
    ],
)
def test_incomplete_capture_series_are_not_complete(manifests):
    assert gate.resource_maxima(manifests, expected_samples=4)["complete"] is False


def test_capture_manifests_are_read_in_protocol_order(tmp_path):
    for kind, sequence in (("sample", 2), ("preflight", 0), ("sample", 1)):
        (tmp_path / f"{kind}-{sequence:010}.json").write_text(json.dumps(manifest(kind, sequence)))
    (tmp_path / "sample-0000000001-peer-0000-status.body").write_text("{}")
    (tmp_path / "notes.json").write_text("{}")
    assert [(item["kind"], item["sequence"]) for item in gate.capture_manifests(tmp_path)] == [
        ("preflight", 0), ("sample", 1), ("sample", 2)
    ]


def test_node_counters_come_from_the_window_edge_captures(tmp_path):
    manifests = [manifest("sample", seq) for seq in (1, 2, 3)]
    for seq, approved, blocks in ((1, 100, 10), (3, 350, 22)):
        for index in range(4):
            name = f"sample-{seq:010}-peer-{index:04}-status.body"
            (tmp_path / name).write_text(json.dumps({"txs_approved": approved + index, "blocks": blocks, "queue_size": 0}))
    window = gate.node_counter_window(tmp_path, manifests, (1, 3))
    assert window == {
        "window_samples": [1, 3],
        "peers": [{"peer": f"peer{index}", "txs_approved": 250, "blocks": 12} for index in range(4)],
    }
    assert gate.node_counter_window(tmp_path, manifests, (1, 9)) is None


def test_global_blocks_are_counted_on_the_slowest_validator():
    before = {f"peer{index}": {"committed_height": 10 + index} for index in range(4)}
    after = {f"peer{index}": {"committed_height": 50 - index} for index in range(4)}
    assert gate.committed_blocks(before, after) == 47 - 10
    after["peer2"] = None
    assert gate.committed_blocks(before, after) is None
    assert gate.committed_blocks({}, after) is None


def lanes_snapshot(heights, rescued=0, halted=False, instance="ab" * 32):
    return {
        f"peer{peer}": [
            {
                "record": {"lane": lane, "merged": {"height": height}, "rescued": rescued},
                "instance": None if halted else {"instance": instance, "halted": None},
            }
            for lane, height in heights.items()
        ]
        for peer in range(4)
    }


def test_four_lane_run_must_advance_every_lane_frontier_and_apply_lane_blocks():
    before = lanes_snapshot({1: 0, 2: 0, 3: 0})
    after = lanes_snapshot({1: 7, 2: 9, 3: 8})
    blocks = {"1": 28, "2": 36, "3": 32}
    assert gate.lanes_used("four_lane", before, after, blocks) == (True, [])
    used, problems = gate.lanes_used("four_lane", before, lanes_snapshot({1: 7, 2: 0, 3: 8}), blocks)
    assert not used and any("lane 2 merged no new block" in item for item in problems)
    used, problems = gate.lanes_used("four_lane", before, after, {"1": 1, "2": 1})
    assert not used and any("lane 3" in item for item in problems)
    used, problems = gate.lanes_used("four_lane", before, lanes_snapshot({1: 7, 2: 9, 3: 8}, halted=True), blocks)
    assert not used and any("halted" in item for item in problems)
    used, problems = gate.lanes_used("four_lane", before, lanes_snapshot({1: 7, 2: 9}), blocks)
    assert not used and any("differ from the expected" in item for item in problems)


def test_lane_instances_run_only_when_every_validator_runs_every_fixed_lane():
    running = lanes_snapshot({1: 0, 2: 0, 3: 0})
    assert gate.lane_instances_running(running, 4) is True
    assert gate.lane_instances_running(lanes_snapshot({1: 0, 2: 0}), 4) is False
    assert gate.lane_instances_running(lanes_snapshot({1: 0, 2: 0, 3: 0}, halted=True), 4) is False
    running["peer2"][1]["instance"]["halted"] = "Storage"
    assert gate.lane_instances_running(running, 4) is False
    unanswered = lanes_snapshot({1: 0, 2: 0, 3: 0})
    unanswered["peer1"] = None
    assert gate.lane_instances_running(unanswered, 4) is False
    assert gate.lane_instances_running({}, 4) is False


def test_lane_activation_commits_pings_until_the_instances_run(monkeypatch):
    before = lanes_snapshot({1: 0, 2: 0, 3: 0}, halted=True)
    after = lanes_snapshot({1: 0, 2: 0, 3: 0})
    snapshots = iter([before, before, after])
    pings = []
    net = gate.GateNetwork.__new__(gate.GateNetwork)
    monkeypatch.setattr(net, "lanes", lambda **_: next(snapshots), raising=False)
    monkeypatch.setattr(net, "advance_global_chain", lambda message, _: pings.append(message), raising=False)
    result = net.wait_for_lane_instances(4, timeout_s=60)
    assert result == {"lanes": after, "pings": 2}
    assert pings == ["scaling-lane-activation-0", "scaling-lane-activation-1"]


def test_lane_activation_does_not_accept_late_readiness_or_submit_after_expiry(monkeypatch):
    clock = [0.0]
    monkeypatch.setattr(gate.time, "monotonic", lambda: clock[0])
    net = gate.GateNetwork.__new__(gate.GateNetwork)
    pings = []

    def late_lanes(*, deadline):
        assert deadline == 5.0
        clock[0] = deadline
        return lanes_snapshot({1: 0, 2: 0, 3: 0})

    monkeypatch.setattr(net, "lanes", late_lanes)
    monkeypatch.setattr(net, "advance_global_chain", lambda *args: pings.append(args))
    with pytest.raises(gate.GateError, match="after 0 pings"):
        net.wait_for_lane_instances(4, timeout_s=5)
    assert not pings


def test_lane_activation_ping_uses_only_remaining_deadline(monkeypatch):
    clock = [0.0]
    monkeypatch.setattr(gate.time, "monotonic", lambda: clock[0])
    net = gate.GateNetwork.__new__(gate.GateNetwork)

    def lanes(*, deadline):
        clock[0] = 3.0
        return lanes_snapshot({1: 0, 2: 0}, halted=True)

    def ping(message, timeout_s):
        assert message == "scaling-lane-activation-0"
        assert timeout_s == 2.0
        clock[0] += timeout_s

    monkeypatch.setattr(net, "lanes", lanes)
    monkeypatch.setattr(net, "advance_global_chain", ping)
    with pytest.raises(gate.GateError, match="after 1 pings"):
        net.wait_for_lane_instances(4, timeout_s=5)


def test_lane_snapshot_stops_dispatching_reads_when_deadline_expires(monkeypatch):
    clock = [0.0]
    monkeypatch.setattr(gate.time, "monotonic", lambda: clock[0])
    net = gate.GateNetwork.__new__(gate.GateNetwork)
    net.validators = 4
    monkeypatch.setattr(net, "node", lambda index: f"peer{index}")
    monkeypatch.setattr(net, "api_port", lambda index: 9000 + index)
    reads = []

    def read(url, timeout):
        reads.append((url, timeout))
        clock[0] += timeout
        return []

    monkeypatch.setattr(gate, "http_json_any", read)
    assert net.lanes(deadline=2.0) == {"peer0": [], "peer1": None, "peer2": None, "peer3": None}
    assert reads == [("http://127.0.0.1:9000/v1/sumeragi/lanes", 2.0)]


def test_lane_readiness_rejects_malformed_record():
    snapshot = lanes_snapshot({1: 0, 2: 0, 3: 0})
    snapshot["peer0"][0]["record"] = "invalid"
    assert not gate.lane_instances_running(snapshot, 4)


def test_one_lane_run_has_no_lane_records():
    assert gate.lanes_used("one_lane", {f"peer{i}": [] for i in range(4)}, {f"peer{i}": [] for i in range(4)}, {}) == (True, [])
    used, _ = gate.lanes_used("one_lane", {}, lanes_snapshot({1: 3}), {})
    assert not used


def test_lane_frontier_is_the_lowest_merged_height_over_validators():
    snapshot = lanes_snapshot({1: 5})
    snapshot["peer2"][0]["record"]["merged"]["height"] = 3
    snapshot["peer1"] = None
    frontier = gate.lane_frontiers(snapshot)
    assert frontier["1"]["merged_height"] == 3
    assert frontier["1"]["peers"] == ["peer0", "peer2", "peer3"]


# ---------------------------------------------------------------------------------------------
# Lane policy
# ---------------------------------------------------------------------------------------------


def test_lane_policy_routes_each_account_to_its_lane_with_the_whole_committee():
    members = [{"peer": f"bls{index}", "pop": f"cG9w{index}"} for index in range(4)]
    accounts = [f"account{index}" for index in range(8)]
    policy = gate.lane_policy({"block_time_ms": 1000}, {"data_shards": 4}, members, accounts, 4)
    assert [lane["lane"] for lane in policy["fixed"]] == [1, 2, 3]
    assert all(lane["committee"] == members and lane["dataspace"] == 0 for lane in policy["fixed"])
    assert policy["routes"] == [
        {"lane": index % 4, "account": f"account{index}", "instruction": None}
        for index in range(8)
        if index % 4
    ]
    assert (policy["anchor_freshness"], policy["max_merge_blocks"], policy["stall_window"]) == (16, 16, 64)
    assert policy["autoscale"] is None
    assert policy["lane_params"] == {"block_time_ms": 1000}
    assert gate.lane_policy_parameter(policy) == {"Custom": {"id": "sumeragi_lane_policy", "payload": policy}}
    with pytest.raises(ValueError):
        gate.lane_policy({}, {}, members, accounts[:6], 4)
    with pytest.raises(ValueError):
        gate.lane_policy({}, {}, [], accounts, 4)


def test_account_lanes_split_the_workload_evenly():
    lanes = [gate.lane_of_account(index, 4) for index in range(8)]
    assert sorted(lanes) == [0, 0, 1, 1, 2, 2, 3, 3]
    assert {gate.lane_of_account(index, 1) for index in range(8)} == {0}


def test_generated_accounts_inherit_the_localnet_account_network_context():
    client = (
        'chain = "c"\nchain_discriminant = 1\n\n[transaction]\nnonce = false\n\n'
        '[account]\nchain_discriminant = 753  # node default\nprivate_key = "secret"\n'
        'public_key  = "ed0120AB"\n\n[basic_auth]\nweb_login = "w"\n'
    )
    assert gate.account_network_context(client) == "chain_discriminant = 753\n"
    profiled = '[account]\nprofile = "taira"\nchain_discriminant = 369\n'
    assert gate.account_network_context(profiled) == 'profile = "taira"\nchain_discriminant = 369\n'
    for missing in ('chain_discriminant = 753\n[account]\npublic_key = "k"\n', ""):
        with pytest.raises(gate.GateError):
            gate.account_network_context(missing)


# ---------------------------------------------------------------------------------------------
# Verdict
# ---------------------------------------------------------------------------------------------


def run_record(pair, variant, *, status="complete", complete=True, used=True, maxima=None):
    return {
        "pair_index": pair,
        "variant": variant,
        "status": status,
        "failure": None if status == "complete" else "transaction load failed (1)",
        "resources": {"complete": complete, "maxima": maxima or {"rss_bytes_sum": 1000 * pair}},
        "lanes": {"used": used, "problems": [] if used else ["lane 2 merged no new block"]},
        "node_counters": None,
    }


def measurement(pair, variant, throughput, latencies, requests=None):
    return gate.RunMeasurement(
        pair_index=pair,
        variant=variant,
        warmup_requests=16,
        measurement_requests=requests if requests is not None else len(latencies),
        committed_in_window=0,
        drain_committed=0,
        throughput_tps=Fraction(throughput),
        p95_latency_ns=gate.p95_nearest_rank(latencies),
        max_latency_ns=max(latencies),
        latencies_ns=tuple(latencies),
        first_height=1,
        last_height=2,
    )


def experiment(one_tps, four_tps, one_latency, four_latency, pairs=5):
    runs, measured = [], {}
    for pair in range(1, pairs + 1):
        for variant, tps, latency in (("one_lane", one_tps, one_latency), ("four_lane", four_tps, four_latency)):
            runs.append(run_record(pair, variant))
            measured[(pair, variant)] = measurement(pair, variant, tps[pair - 1], [latency] * 100)
    return runs, measured


def test_passing_experiment_meets_both_thresholds_and_qualifies():
    runs, measured = experiment([10, 11, 12, 9, 10], [16, 18, 17, 15, 19], NS, NS)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["ok"] is True and verdict["release_qualifying"] is True
    assert verdict["failures"] == []
    assert verdict["one_lane"]["median_throughput_tps"]["exact"] == "10/1"
    assert verdict["four_lane"]["median_throughput_tps"]["exact"] == "17/1"
    assert verdict["throughput_ratio"]["exact"] == "17/10"
    assert verdict["p95_latency_ratio"]["exact"] == "1/1"
    assert verdict["criteria"] == {"runs_complete": True, "throughput": True, "latency": True}
    assert verdict["one_lane"]["resource_maxima"] == {"rss_bytes_sum": 5000}
    assert [run["pair_index"] for run in verdict["runs"]] == [1, 1, 2, 2, 3, 3, 4, 4, 5, 5]
    json.dumps(verdict)


def test_exact_threshold_boundaries_pass_and_just_below_fails():
    runs, measured = experiment([10] * 5, [15] * 5, 4 * NS, 5 * NS)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["throughput_ratio"]["exact"] == "3/2" and verdict["p95_latency_ratio"]["exact"] == "5/4"
    assert verdict["ok"] is True
    runs, measured = experiment([10] * 5, [15] * 4 + [Fraction(1499, 100)], 4 * NS, 5 * NS)
    # The median ignores one slow four-lane run.
    assert gate.evaluate(runs, measured, 5, 100)["ok"] is True
    runs, measured = experiment([10] * 5, [Fraction(1499, 100)] * 5, 4 * NS, 5 * NS)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["ok"] is False and verdict["criteria"]["throughput"] is False
    assert any("throughput ratio" in item for item in verdict["failures"])
    runs, measured = experiment([10] * 5, [15] * 5, 4 * NS, 5 * NS + 1)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["ok"] is False and verdict["criteria"]["latency"] is False
    assert any("p95 latency ratio" in item for item in verdict["failures"])


def test_pooled_p95_spans_every_run_of_a_variant():
    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    # One four-lane run with a slow tail moves the pooled p95 of 500 samples: 25 slow samples
    # sit exactly at rank 476 and above, so the 475th value (p95) stays fast.
    measured[(3, "four_lane")] = measurement(3, "four_lane", 20, [NS] * 75 + [9 * NS] * 25)
    assert gate.evaluate(runs, measured, 5, 100)["four_lane"]["pooled_p95_latency_ms"] == 1000.0
    measured[(3, "four_lane")] = measurement(3, "four_lane", 20, [NS] * 74 + [9 * NS] * 26)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["four_lane"]["pooled_p95_latency_ms"] == 9000.0
    assert verdict["ok"] is False


def test_incomplete_or_partial_experiments_do_not_pass():
    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    runs[3] = run_record(2, "four_lane", status="failed")
    del measured[(2, "four_lane")]
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["ok"] is False and verdict["throughput_ratio"] is None
    assert verdict["criteria"]["runs_complete"] is False
    assert any("pair 2 four_lane" in item for item in verdict["failures"])

    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    runs[1] = run_record(1, "four_lane", used=False)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["ok"] is False and any("lane 2" in item for item in verdict["failures"])

    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    runs[0] = run_record(1, "one_lane", complete=False)
    assert gate.evaluate(runs, measured, 5, 100)["ok"] is False

    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    measured[(4, "one_lane")] = measurement(4, "one_lane", 10, [NS] * 99)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["ok"] is False and any("99 latency samples" in item for item in verdict["failures"])

    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    verdict = gate.evaluate(runs[:-2], {k: v for k, v in measured.items() if k[0] < 5}, 5, 100)
    assert verdict["ok"] is False and "pair 5 four_lane: not run" in verdict["failures"]


def test_one_pair_verdict_is_partial_and_never_release_qualifying():
    runs, measured = experiment([10], [20], NS, NS, pairs=1)
    verdict = gate.evaluate(runs, measured, 1, 100)
    assert verdict["ok"] is True and verdict["release_qualifying"] is False


def test_a_lowered_latency_sample_minimum_never_qualifies():
    runs, measured = experiment([10] * 5, [20] * 5, NS, NS)
    verdict = gate.evaluate(runs, measured, 5, 50)
    assert verdict["ok"] is True and verdict["release_qualifying"] is False


def test_zero_one_lane_throughput_has_no_ratio():
    runs, measured = experiment([0] * 5, [20] * 5, NS, NS)
    verdict = gate.evaluate(runs, measured, 5, 100)
    assert verdict["throughput_ratio"] is None and verdict["ok"] is False


# ---------------------------------------------------------------------------------------------
# --analyze over a finished run directory
# ---------------------------------------------------------------------------------------------


def write_finished_run(out: Path, plan_options, one_latency, four_latency, four_tail=0):
    rate = plan_options.rate
    warmup = gate.cohort_count(plan_options.warmup_ns, rate)
    requests = gate.cohort_count(plan_options.measurement_ns, rate)
    period = NS * rate.denominator // rate.numerator
    (out).mkdir(parents=True, exist_ok=True)
    (out / "plan.json").write_text(json.dumps({"schema": gate.PLAN_SCHEMA, "options": dataclasses.asdict(plan_options)}))
    for pair, variant in gate.run_order(plan_options.pairs):
        directory = gate.run_directory(out, pair, variant)
        directory.mkdir(parents=True)
        latency = one_latency if variant == "one_lane" else four_latency
        latencies = [latency] * requests
        if variant == "four_lane" and four_tail:
            latencies[-four_tail:] = [plan_options.drain_ns] * four_tail
        trace = synthetic_trace(pair, variant, gate.pair_seed(plan_options.seed_namespace, pair), warmup, latencies, period)
        (directory / "trace.json").write_text(json.dumps(trace))
        record = run_record(pair, variant)
        record["schema"] = gate.RUN_SCHEMA
        (directory / "run.json").write_text(json.dumps(record))


def test_analyze_recomputes_the_verdict_from_files(tmp_path, capsys):
    plan_options = options()
    write_finished_run(tmp_path, plan_options, NS // 2, NS // 2)
    assert gate.main(["--analyze", str(tmp_path)]) == gate.EXIT_FAIL
    verdict = json.loads((tmp_path / "verdict.json").read_text())
    # Same latency and throughput in both variants: the throughput criterion fails.
    assert verdict["throughput_ratio"]["exact"] == "1/1"
    assert verdict["criteria"] == {"runs_complete": True, "throughput": False, "latency": True}
    assert verdict["plan"]["offered_load_tps"] == "20"
    summary = json.loads(capsys.readouterr().out)
    assert summary["ok"] is False and summary["release_qualifying"] is False


def test_analyze_rejects_a_trace_from_another_seed(tmp_path):
    plan_options = options()
    write_finished_run(tmp_path, plan_options, NS // 2, NS // 2)
    trace_path = gate.run_directory(tmp_path, 1, "four_lane") / "trace.json"
    trace = json.loads(trace_path.read_text())
    trace["seed"] = "00" * 32
    trace_path.write_text(json.dumps(trace))
    assert gate.main(["--analyze", str(tmp_path)]) == gate.EXIT_FAIL
    verdict = json.loads((tmp_path / "verdict.json").read_text())
    assert any("trace rejected" in item for item in verdict["failures"])


def test_analyze_of_a_missing_plan_is_a_harness_error(tmp_path):
    assert gate.main(["--analyze", str(tmp_path)]) == gate.EXIT_HARNESS


def test_load_command_carries_the_exact_schedule_and_probe_inputs(tmp_path):
    plan_options = options()
    command = gate.load_command(
        Path("/bin/iroha"),
        tmp_path / "client0.toml",
        plan_options,
        invocation_id="1" * 64,
        pair=2,
        variant="four_lane",
        seed="ab" * 32,
        accounts=[tmp_path / f"load{index}.toml" for index in range(8)],
        observer_config=tmp_path / "client3.toml",
        trace=tmp_path / "trace.json",
        journal=tmp_path / "journal.jsonl",
        probe_config_path=tmp_path / "probe.json",
        budget_sha256="cd" * 32,
        capture_dir=tmp_path / "captures",
        python=Path("/usr/bin/python3"),
    )
    assert command[:7] == ["/bin/iroha", "--config", str(tmp_path / "client0.toml"), "--fee-payer", "authority", "tx", "load"]

    def value(flag):
        return command[command.index(flag) + 1]

    assert value("--pair-index") == "2" and value("--variant") == "four_lane"
    assert value("--offered-load-tps") == "20" and value("--max-submission-lag-ms") == "12.5"
    assert value("--measurement-seconds") == "40" and value("--drain-seconds") == "40"
    assert value("--resource-worker") == str(gate.NEXUS_DIR / "resource_probe_worker.py")
    assert value("--resource-interval-ms") == "2000" and value("--poll-interval-ms") == "100"
    assert command.count("--account-config") == 8


def test_parse_load_receipt_requires_the_terminal_receipt():
    receipt = {"version": 1, "operation": "transaction_load", "trace_sha256": "ab" * 32}
    assert gate.parse_load_receipt("noise\n" + json.dumps(receipt) + "\n") == receipt
    with pytest.raises(gate.GateError):
        gate.parse_load_receipt("")
    with pytest.raises(gate.GateError):
        gate.parse_load_receipt(json.dumps({"version": 2, "operation": "transaction_load"}))
