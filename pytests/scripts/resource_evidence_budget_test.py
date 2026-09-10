"""Pure admission controls; no capture, writer, process, or bundle scan execution."""
from dataclasses import FrozenInstanceError, replace
import builtins
import importlib.util
from pathlib import Path
import sys

import pytest

SOURCE = Path(__file__).resolve().parents[2] / "scripts/nexus/resource_evidence_budget.py"
SPEC = importlib.util.spec_from_file_location("private_resource_evidence_budget", SOURCE)
budget = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = budget
SPEC.loader.exec_module(budget)


def geometry(**changes):
    return budget.CaptureGeometry(**dict(peers=4, interval_ns=budget.NS,
        measurement_ns=20 * budget.NS, drain_ns=budget.NS) | changes)


def inputs(**changes):
    runs = []
    for pair in range(1, 6):
        for variant in ("one_lane", "four_lane"):
            prefix = f"pair{pair}.{variant}"
            files = tuple(budget.FileBudget(f"{prefix}.{name}", size * budget.MIB)
                          for name, size in (("journal", 4), ("trace", 4), ("proof", 4),
                                             ("log", 1), ("raw", 1)))
            runs.append(budget.RunBudget(pair, variant, geometry(), *files,
                (budget.FileBudget(f"{prefix}.support", budget.MIB),)))
    return dict(policy=budget.CapturePolicy(), runs=tuple(runs),
                static_files=(budget.StaticFile("software", 16 * budget.MIB),),
                manifest=budget.FileBudget("manifest", budget.MIB),
                report=budget.FileBudget("report", budget.MIB), other_control=()) | changes


def fail(code, operation):
    with pytest.raises(budget.BudgetError, match=f"^{code}$"):
        operation()


def all_geometry(config, value):
    return config | {"runs": tuple(replace(run, geometry=value) for run in config["runs"])}


def test_default_policy_and_complete_ten_run_reservation_fit():
    config = inputs()
    result = budget.admit_experiment(**config)
    assert result.policy.status_body_bytes == 128 * 1024
    assert result.policy.metrics_body_bytes == budget.MIB
    assert result.geometry.sample_count == 22
    assert result.geometry.captures_per_run == 23
    assert result.resource_capture_count == 230
    assert (result.members_per_capture, result.members_per_run, result.resource_member_count) == (9, 207, 2070)
    assert result.bytes_per_capture == budget.MIB + 4 * (128 * 1024 + budget.MIB)
    assert result.resource_bytes_per_run == 23 * result.bytes_per_capture
    assert result.resource_bytes == 10 * 23 * result.bytes_per_capture == 1326448640
    assert result.static_bytes == 16 * budget.MIB
    assert result.dynamic_bytes == 152 * budget.MIB
    assert result.control_file_count == 63
    assert result.total_bytes == 1433 * budget.MIB
    assert result.remaining_bytes == 615 * budget.MIB
    assert result.runs == config["runs"] and result.static_files == config["static_files"]


@pytest.mark.parametrize("field,ceiling", [("status_body_bytes", budget.MAX_STATUS_BODY_BYTES),
                                          ("metrics_body_bytes", budget.MAX_METRICS_BODY_BYTES)])
@pytest.mark.parametrize("value", [False, True, 0, -1, 1.0, "1", None, 1 << 128])
def test_body_caps_reject_wrong_types_zero_and_overflow(field, ceiling, value):
    fail("integer_outside_bounds", lambda: budget.CapturePolicy(**{field: value}))


@pytest.mark.parametrize("field,ceiling", [("status_body_bytes", budget.MAX_STATUS_BODY_BYTES),
                                          ("metrics_body_bytes", budget.MAX_METRICS_BODY_BYTES)])
def test_body_cap_exact_minimum_and_ceiling(field, ceiling):
    assert getattr(budget.CapturePolicy(**{field: 1}), field) == 1
    assert getattr(budget.CapturePolicy(**{field: ceiling}), field) == ceiling
    fail("integer_outside_bounds", lambda: budget.CapturePolicy(**{field: ceiling + 1}))


@pytest.mark.parametrize("field", ["peers", "interval_ns", "measurement_ns", "drain_ns"])
@pytest.mark.parametrize("value", [True, False, 1.0, "4", None, -1, 1 << 128])
def test_geometry_rejects_non_exact_integer_inputs(field, value):
    fail("integer_outside_bounds", lambda: geometry(**{field: value}))


@pytest.mark.parametrize("changes,code", [
    ({"peers": 3}, "integer_outside_bounds"),
    ({"peers": 65}, "integer_outside_bounds"),
    ({"interval_ns": 1_000_000}, "integer_outside_bounds"),
    ({"interval_ns": 60 * budget.NS + 1}, "integer_outside_bounds"),
    ({"interval_ns": 2_000_001}, "cadence_not_milliseconds"),
    ({"measurement_ns": 20 * budget.NS + 1}, "geometry_not_divisible"),
    ({"drain_ns": budget.NS + 1}, "geometry_not_divisible"),
    ({"measurement_ns": 19 * budget.NS}, "measurement_intervals_below_minimum"),
    ({"drain_ns": 0}, "integer_outside_bounds"),
    ({"drain_ns": 300 * budget.NS + 1}, "integer_outside_bounds"),
    ({"measurement_ns": 99_999 * budget.NS}, "sample_count_outside_bounds"),
])
def test_geometry_protocol_and_inclusive_endpoint_boundaries(changes, code):
    fail(code, lambda: geometry(**changes))


def test_exact_protocol_geometry_extremes_are_distinct_from_global_budget():
    assert geometry(interval_ns=2_000_000, measurement_ns=40_000_000,
                    drain_ns=2_000_000).sample_count == 22
    assert geometry(interval_ns=60 * budget.NS, measurement_ns=1200 * budget.NS,
                    drain_ns=300 * budget.NS).sample_count == 26
    maximum = geometry(measurement_ns=99_998 * budget.NS)
    assert maximum.sample_count == 100_000
    assert maximum.captures_per_run == 100_001
    fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(
        **all_geometry(inputs(policy=budget.CapturePolicy(1, 1)), maximum)))
    tiny64 = budget.admit_experiment(**all_geometry(inputs(policy=budget.CapturePolicy(1, 1)), geometry(peers=64)))
    assert tiny64.members_per_capture == 129
    assert tiny64.resource_member_count == 10 * 23 * 129
    assert tiny64.bytes_per_capture == budget.MIB + 128


def test_timing_overflow_is_checked_before_sample_admission():
    # Every field itself fits i64 and exact cadence, but the endpoint does not.
    measurement = budget.MAX_I64 // budget.NS * budget.NS
    fail("timing_overflow", lambda: geometry(measurement_ns=measurement))


@pytest.mark.parametrize("kind", ["missing", "eleven", "duplicate", "list", "none", "wrong_item"])
def test_ten_runs_and_pair_variant_coverage_are_mandatory(kind):
    config = inputs()
    runs = config["runs"]
    replacements = {"missing": runs[:-1], "eleven": (*runs, runs[0]),
                    "duplicate": (*runs[:-1], runs[0]), "list": list(runs),
                    "none": None, "wrong_item": (*runs[:-1], None)}
    code = {"missing": "exact_ten_runs_required", "duplicate": "pair_variant_coverage_invalid",
            "wrong_item": "item_type_invalid"}.get(kind, "bounded_tuple_required")
    fail(code, lambda: budget.admit_experiment(**(config | {"runs": replacements[kind]})))


@pytest.mark.parametrize("changes", [{"peers": 5}, {"interval_ns": 500_000_000},
                                     {"measurement_ns": 21 * budget.NS}, {"drain_ns": 2 * budget.NS}])
def test_any_single_run_geometry_difference_rejects_both_pair_and_cross_pair(changes):
    config = inputs()
    for index in (1, 8, 9):
        runs = list(config["runs"])
        runs[index] = replace(runs[index], geometry=geometry(**changes))
        fail("run_geometry_mismatch", lambda: budget.admit_experiment(**(config | {"runs": tuple(runs)})))


def test_run_order_does_not_replace_identity_and_same_values_are_accepted():
    config = inputs()
    result = budget.admit_experiment(**(config | {"runs": tuple(reversed(config["runs"]))}))
    assert result.resource_member_count == 2070
    assert result.total_bytes == budget.admit_experiment(**config).total_bytes


@pytest.mark.parametrize("field,value,code", [
    ("pair_index", True, "integer_outside_bounds"), ("pair_index", 0, "integer_outside_bounds"),
    ("pair_index", 6, "integer_outside_bounds"), ("variant", True, "variant_invalid"),
    ("variant", "unsampled", "variant_invalid"), ("geometry", None, "geometry_type_invalid"),
    ("collector_journal", None, "artifact_budget_required"),
    ("transaction_trace", 1, "artifact_budget_required"),
    ("canonical_proof", None, "artifact_budget_required"),
    ("trial_log", None, "artifact_budget_required"), ("raw_run", None, "artifact_budget_required"),
    ("support", [], "bounded_tuple_required"), ("support", (None,), "item_type_invalid"),
])
def test_each_run_requires_typed_allocations_and_no_unsampled_form(field, value, code):
    fail(code, lambda: replace(inputs()["runs"][0], **{field: value}))


@pytest.mark.parametrize("constructor", [budget.FileBudget, budget.StaticFile])
@pytest.mark.parametrize("size", [True, False, -1, 1.0, "1", None, budget.MAX_FILE_BYTES + 1, 1 << 128])
def test_every_physical_file_allocation_has_exact_type_and_bound(constructor, size):
    fail("integer_outside_bounds", lambda: constructor("file", size))


def test_positive_dynamic_zero_static_and_exact_per_file_limit():
    fail("integer_outside_bounds", lambda: budget.FileBudget("empty", 0))
    empty = budget.StaticFile("empty", 0)
    config = inputs(static_files=(empty,))
    result = budget.admit_experiment(**(config | {"report": budget.FileBudget("report", budget.MAX_FILE_BYTES)}))
    assert result.static_bytes == 0
    assert result.control_file_count == 63
    assert result.control_budgets[1].max_bytes == budget.MAX_FILE_BYTES
    assert budget.StaticFile("maximum", budget.MAX_FILE_BYTES).size_bytes == budget.MAX_FILE_BYTES


@pytest.mark.parametrize("label", [None, True, "", "A", "../x", "x/y", "x y", "x\n", "x" * 129])
def test_labels_are_bounded_ledger_identities_not_runtime_paths(label):
    fail("label_invalid", lambda: budget.FileBudget(label, 1))


@pytest.mark.parametrize("where", ["same_run", "cross_run", "static_dynamic", "root_control", "static_static"])
def test_duplicate_allocations_never_hide_or_double_count_one_declared_file(where):
    config = inputs()
    runs = list(config["runs"])
    if where == "same_run": runs[0] = replace(runs[0], trial_log=runs[0].raw_run)
    elif where == "cross_run": runs[1] = replace(runs[1], raw_run=runs[0].raw_run)
    elif where == "static_dynamic": config["static_files"] = (budget.StaticFile("report", 1),)
    elif where == "root_control": config["other_control"] = (config["report"],)
    else: config["static_files"] = config["static_files"] * 2
    fail("duplicate_file_allocation", lambda: budget.admit_experiment(**(config | {"runs": tuple(runs)})))


def test_exact256_control_files_leave_all2070_raw_members_separately_counted():
    config = inputs()
    base = budget.admit_experiment(**config)
    extra = tuple(budget.FileBudget(f"extra{i}", 1) for i in range(256 - base.control_file_count))
    result = budget.admit_experiment(**(config | {"other_control": extra}))
    assert result.control_file_count == 256
    assert result.resource_member_count == 2070
    assert result.total_bytes == base.total_bytes + len(extra)
    fail("control_file_count_exceeded", lambda: budget.admit_experiment(**(
        config | {"other_control": (*extra, budget.FileBudget("too_many", 1))})))


def test_exact_global2gib_is_accepted_and_one_additional_byte_fails():
    config = inputs()
    remaining = budget.admit_experiment(**config).remaining_bytes
    chunks = []
    while remaining:
        amount = min(remaining, budget.MAX_FILE_BYTES)
        chunks.append(budget.FileBudget(f"allocation{len(chunks)}", amount))
        remaining -= amount
    exact = config | {"other_control": tuple(chunks)}
    result = budget.admit_experiment(**exact)
    assert result.total_bytes == 2 * 1024 * budget.MIB
    assert result.remaining_bytes == 0
    chunks[-1] = replace(chunks[-1], max_bytes=chunks[-1].max_bytes + 1)
    fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(**(exact | {"other_control": tuple(chunks)})))


def test_hard_protocol_caps_and_default64peer_runs_do_not_get_compression_discount():
    fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(
        **inputs(policy=budget.CapturePolicy(budget.MIB, 16 * budget.MIB))))
    fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(
        **all_geometry(inputs(), geometry(peers=64))))
    config = inputs()
    huge_journals = tuple(replace(run, collector_journal=replace(run.collector_journal,
                            max_bytes=budget.MAX_FILE_BYTES)) for run in config["runs"])
    fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(**(config | {"runs": huge_journals})))


def test_exact_wire_clamp_and_neighboring_feasible_body_policies():
    for delta in (-1, 0, 1):
        policy = budget.CapturePolicy(budget.MIB, 15 * budget.MIB + delta)
        assert budget._bytes_per_capture(policy, 4) == 65 * budget.MIB + min(4 * delta, 0)
    # The clamp limits stored-body reservation; framing is not counted twice.
    assert budget._bytes_per_capture(budget.CapturePolicy(budget.MIB, 16 * budget.MIB), 64) == 65 * budget.MIB


def test_eight_peers_already_exceed_global_budget_with_default_caps():
    policy = budget.CapturePolicy()
    assert 10 * 23 * budget._bytes_per_capture(policy, 8) == 2411724800
    fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(
        **all_geometry(inputs(), geometry(peers=8))))


@pytest.mark.parametrize("left,right", [(budget.MAX_U64, 1), (budget.MAX_U64 - 1, 2)])
def test_checked_add_rejects_unsigned_overflow(left, right):
    fail("arithmetic_overflow", lambda: budget._add(left, right))


@pytest.mark.parametrize("left,right", [(budget.MAX_U64, 2), (1 << 63, 2), (1 << 32, 1 << 32)])
def test_checked_multiply_rejects_unsigned_overflow(left, right):
    fail("arithmetic_overflow", lambda: budget._multiply(left, right))


def test_arithmetic_exact_unsigned_boundaries_and_zero_are_supported():
    assert budget._add(budget.MAX_U64 - 1, 1) == budget.MAX_U64
    assert budget._multiply(budget.MAX_U64, 1) == budget.MAX_U64
    assert budget._multiply(0, budget.MAX_U64) == budget._multiply(budget.MAX_U64, 0) == 0
    fail("arithmetic_overflow", lambda: budget._sum((budget.MAX_U64, 1)))
    for operation in (budget._add, budget._multiply):
        fail("integer_outside_bounds", lambda: operation(True, 1))
        fail("integer_outside_bounds", lambda: operation(1, budget.MAX_U64 + 1))


@pytest.mark.parametrize("field,value,code", [
    ("policy", None, "capture_policy_required"), ("policy", {}, "capture_policy_required"),
    ("static_files", (), "pinned_static_files_required"),
    ("static_files", [], "bounded_tuple_required"), ("static_files", (None,), "item_type_invalid"),
    ("manifest", None, "control_budget_required"), ("report", None, "control_budget_required"),
    ("other_control", None, "bounded_tuple_required"),
    ("other_control", (1,), "item_type_invalid"),
])
def test_root_allocations_are_mandatory_and_bounded(field, value, code):
    fail(code, lambda: budget.admit_experiment(**(inputs() | {field: value})))


def test_oversized_input_tuples_reject_before_processing_members():
    config = inputs()
    for field in ("static_files", "other_control"):
        fail("bounded_tuple_required", lambda: budget.admit_experiment(**(config | {field: (None,) * 257})))
    fail("bounded_tuple_required", lambda: replace(config["runs"][0], support=(None,) * 257))


def test_all_admitted_objects_and_collections_are_immutable():
    result = budget.admit_experiment(**inputs())
    for item, field in ((result, "total_bytes"), (result.policy, "status_body_bytes"),
                        (result.geometry, "peers"), (result.runs[0], "variant"),
                        (result.static_files[0], "size_bytes"), (result.control_budgets[0], "max_bytes")):
        with pytest.raises(FrozenInstanceError): setattr(item, field, 0)
        assert not hasattr(item, "__dict__")
    assert type(result.runs) is type(result.static_files) is type(result.control_budgets) is tuple
    assert type(result.runs[0].support) is tuple


@pytest.mark.parametrize("owner", ["policy", "geometry", "static", "file", "run"])
def test_subclasses_cannot_override_exact_ledger_owner_semantics(owner):
    config = inputs()
    if owner == "policy":
        class Derived(budget.CapturePolicy): pass
        fail("capture_policy_required", lambda: budget.admit_experiment(**(config | {"policy": Derived()})))
    elif owner == "geometry":
        class Derived(budget.CaptureGeometry): pass
        child = Derived(4, budget.NS, 20 * budget.NS, budget.NS)
        fail("geometry_type_invalid", lambda: replace(config["runs"][0], geometry=child))
    elif owner == "static":
        class Derived(budget.StaticFile): pass
        fail("item_type_invalid", lambda: budget.admit_experiment(**(config | {"static_files": (Derived("input", 1),)})))
    elif owner == "file":
        class Derived(budget.FileBudget): pass
        fail("control_budget_required", lambda: budget.admit_experiment(**(config | {"report": Derived("report", 1)})))
    else:
        class Derived(budget.RunBudget): pass
        original = config["runs"][0]
        child = Derived(original.pair_index, original.variant, original.geometry,
                        *original.files[:5], original.support)
        fail("item_type_invalid", lambda: budget.admit_experiment(**(config | {"runs": (child, *config["runs"][1:])})))


def test_budget_has_no_io_or_sampling_dependency_and_needs_no_preflight(monkeypatch):
    import ast
    import os
    import socket
    import subprocess
    source = SOURCE.read_text()
    imports = {node.module if isinstance(node, ast.ImportFrom) else name.name
               for node in ast.walk(ast.parse(source)) if isinstance(node, (ast.Import, ast.ImportFrom))
               for name in node.names}
    assert imports == {"__future__", "dataclasses", "re", "hashlib", "json"}
    def forbidden(*args, **kwargs): raise AssertionError("admission attempted external work")
    with monkeypatch.context() as patch:
        patch.setattr(builtins, "open", forbidden)
        patch.setattr(os, "stat", forbidden)
        patch.setattr(os, "listdir", forbidden)
        patch.setattr(socket, "socket", forbidden)
        patch.setattr(subprocess, "Popen", forbidden)
        assert budget.admit_experiment(**inputs()).total_bytes == 1433 * budget.MIB
        selected = budget.select_run_budget(budget.admit_experiment(**inputs()), 1, 'one_lane')
        assert budget.canonical_run_budget_bytes(selected).startswith(b'{"experiment":')
        assert len(budget.run_budget_sha256(selected)) == 64
        fail("global_byte_reservation_exceeded", lambda: budget.admit_experiment(
            **inputs(policy=budget.CapturePolicy(budget.MIB, 16 * budget.MIB))))



def test_canonical_public_budget_roundtrip_has_one_ascii_no_newline_encoding():
    import hashlib
    import json
    selected = budget.select_run_budget(budget.admit_experiment(**inputs()), 1, 'one_lane')
    raw = budget.canonical_run_budget_bytes(selected)
    assert type(raw) is bytes and raw.isascii()
    assert raw.startswith(b'{"experiment":') and raw.endswith(b'"variant":"one_lane"}')
    assert b'\n' not in raw and b': ' not in raw and b', ' not in raw
    decoded = json.loads(raw)
    assert decoded == budget.run_budget_inputs(selected)
    assert budget.canonical_run_budget_bytes(budget.parse_run_budget(decoded)) == raw
    assert budget.run_budget_sha256(selected) == hashlib.sha256(raw).hexdigest()
    assert len(budget.run_budget_sha256(selected)) == 64


def test_equivalent_public_object_key_order_keeps_budget_identity():
    import json
    def reverse_keys(value):
        if type(value) is dict:
            return {key: reverse_keys(item) for key, item in reversed(tuple(value.items()))}
        if type(value) is list:
            return [reverse_keys(item) for item in value]
        return value
    selected = budget.select_run_budget(budget.admit_experiment(**inputs()), 2, 'four_lane')
    original = budget.run_budget_inputs(selected)
    reordered = reverse_keys(original)
    assert json.dumps(original) != json.dumps(reordered)
    assert original == reordered
    restored = budget.parse_run_budget(reordered)
    assert budget.canonical_run_budget_bytes(restored) == budget.canonical_run_budget_bytes(selected)
    assert budget.run_budget_sha256(restored) == budget.run_budget_sha256(selected)


@pytest.mark.parametrize('change', ['pair', 'variant', 'policy', 'geometry', 'journal_cap', 'trace_cap', 'label'])
def test_changed_public_allocation_has_a_different_admitted_identity(change):
    original = inputs()
    baseline = budget.select_run_budget(budget.admit_experiment(**original), 1, 'one_lane')
    config = inputs()
    pair, variant = 1, 'one_lane'
    if change == 'pair': pair = 2
    if change == 'variant': variant = 'four_lane'
    if change == 'policy': config['policy'] = replace(config['policy'], status_body_bytes=128*1024+1)
    if change == 'geometry': config = all_geometry(config, geometry(peers=5))
    if change in ('journal_cap', 'trace_cap', 'label'):
        name = 'transaction_trace' if change == 'trace_cap' else 'collector_journal'
        run = config['runs'][0]
        field = getattr(run, name)
        altered = replace(field, label='different_journal') if change == 'label' else replace(field, max_bytes=field.max_bytes+1)
        config['runs'] = (replace(run, **{name: altered}), *config['runs'][1:])
    changed = budget.select_run_budget(budget.admit_experiment(**config), pair, variant)
    assert budget.run_budget_sha256(changed) != budget.run_budget_sha256(baseline)
    assert budget.canonical_run_budget_bytes(changed) != budget.canonical_run_budget_bytes(baseline)


@pytest.mark.parametrize('owner', [budget.canonical_run_budget_bytes, budget.run_budget_sha256])
@pytest.mark.parametrize('mutation', ['summary', 'missing_run', 'changed_policy', 'geometry', 'selected_run', 'wrong_type'])
def test_canonical_hash_owner_re_admits_before_returning_identity(owner, mutation):
    selected = budget.select_run_budget(budget.admit_experiment(**inputs()), 1, 'one_lane')
    if mutation == 'summary': object.__setattr__(selected.experiment, 'total_bytes', 1)
    if mutation == 'missing_run': object.__setattr__(selected.experiment, 'runs', selected.experiment.runs[:-1])
    if mutation == 'changed_policy': object.__setattr__(selected.policy, 'status_body_bytes', True)
    if mutation == 'geometry': object.__setattr__(selected.geometry, 'measurement_ns', 1)
    if mutation == 'selected_run': object.__setattr__(selected, 'run', replace(selected.run, pair_index=2))
    if mutation == 'wrong_type': selected = budget.run_budget_inputs(selected)
    with pytest.raises(budget.BudgetError):
        owner(selected)
