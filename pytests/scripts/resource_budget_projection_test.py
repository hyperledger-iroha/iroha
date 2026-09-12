"""Re-admission and strict public input projection; no runtime configuration I/O."""
from copy import deepcopy
from dataclasses import FrozenInstanceError, replace
import json

import pytest

from resource_evidence_budget_test import budget, inputs, geometry


def selected():
    return budget.select_run_budget(budget.admit_experiment(**inputs()), 1, "one_lane")


def test_projection_reuses_full_admission_and_exact_selected_journal():
    allocation = selected()
    assert allocation.capture_count == 23
    assert allocation.members_per_capture == 9
    assert allocation.member_count == 207
    assert allocation.bytes_per_capture == 11 * budget.MIB // 2
    assert allocation.resource_bytes == 23 * allocation.bytes_per_capture
    assert allocation.journal == allocation.run.collector_journal
    assert allocation.journal.max_bytes == 4 * budget.MIB
    assert allocation.policy == budget.CapturePolicy()
    assert allocation.geometry == geometry()
    assert budget.validate_run_budget(allocation) == allocation
    with pytest.raises(FrozenInstanceError): allocation.run = allocation.experiment.runs[1]


def test_canonical_full_inputs_roundtrip_select_every_pair_and_variant():
    experiment = selected().experiment
    for run in experiment.runs:
        allocation = budget.select_run_budget(experiment, run.pair_index, run.variant)
        value = budget.run_budget_inputs(allocation)
        assert set(value) == {"experiment", "pair_index", "variant"}
        assert set(value["experiment"]) == {"capture_policy", "runs", "static_files", "manifest", "report", "other_control"}
        assert type(value["experiment"]["runs"][0]["support"]) is list
        assert budget.parse_run_budget(value) == allocation
        assert budget.parse_run_budget(json.loads(json.dumps(value))) == allocation
    assert len(json.dumps(value).encode()) < budget.MIB


@pytest.mark.parametrize("field", ["members_per_capture", "members_per_run", "resource_member_count",
    "resource_capture_count", "bytes_per_capture", "resource_bytes_per_run", "resource_bytes",
    "control_file_count", "static_bytes", "dynamic_bytes", "total_bytes"])
@pytest.mark.parametrize("value", [0, 1, True, -1, 1 << 128])
def test_claimed_evidence_summaries_are_recomputed_not_capabilities(field, value):
    forged = replace(selected().experiment, **{field: value})
    with pytest.raises(budget.BudgetError, match="admitted_experiment_mismatch"):
        budget.select_run_budget(forged, 1, "one_lane")


@pytest.mark.parametrize("kind", ["missing_control", "mutable_controls", "null_policy", "missing_run",
                                    "wrong_geometry", "extra_journal", "wrong_selected_run"])
def test_projection_rejects_invalid_parent_or_substituted_run(kind):
    allocation = selected()
    if kind == "missing_control": allocation = replace(allocation, experiment=replace(allocation.experiment, control_budgets=()))
    if kind == "mutable_controls": allocation = replace(allocation, experiment=replace(allocation.experiment, control_budgets=[]))
    if kind == "null_policy": allocation = replace(allocation, experiment=replace(allocation.experiment, policy=None))
    if kind == "missing_run": allocation = replace(allocation, experiment=replace(allocation.experiment, runs=allocation.experiment.runs[:-1]))
    if kind == "wrong_geometry": allocation = replace(allocation, run=replace(allocation.run, geometry=geometry(peers=5)))
    if kind == "extra_journal": allocation = replace(allocation, run=replace(allocation.run,
        collector_journal=replace(allocation.journal, max_bytes=allocation.journal.max_bytes - 1)))
    if kind == "wrong_selected_run": allocation = replace(allocation, run=None)
    with pytest.raises(budget.BudgetError): budget.validate_run_budget(allocation)


def test_bypassed_constructor_is_rejected_before_geometry_division_or_limits():
    allocation = selected()
    object.__setattr__(allocation.run.geometry, "interval_ns", 0)
    with pytest.raises(budget.BudgetError, match="integer_outside_bounds"):
        budget.validate_run_budget(allocation)
    allocation = selected()
    object.__setattr__(allocation.policy, "status_body_bytes", True)
    with pytest.raises(budget.BudgetError, match="integer_outside_bounds"):
        budget.validate_run_budget(allocation)


@pytest.mark.parametrize("path", [(), ("experiment",), ("experiment", "capture_policy"),
    ("experiment", "runs", 0), ("experiment", "runs", 0, "geometry"),
    ("experiment", "runs", 0, "collector_journal"), ("experiment", "static_files", 0),
    ("experiment", "manifest"), ("experiment", "report")])
@pytest.mark.parametrize("change", ["missing", "extra", "non_object"])
def test_every_public_input_object_has_exact_mandatory_fields(path, change):
    value = budget.run_budget_inputs(selected())
    node = value
    for part in path: node = node[part]
    if change == "missing": node.pop(next(iter(node)))
    elif change == "extra": node["computed_bytes"] = 1
    elif not path: value = []
    else:
        owner = value
        for part in path[:-1]: owner = owner[part]
        owner[path[-1]] = []
    with pytest.raises(budget.BudgetError): budget.parse_run_budget(value)


@pytest.mark.parametrize("path,maximum", [(('experiment', 'runs'), 10),
    (('experiment', 'static_files'), 256), (('experiment', 'other_control'), 256),
    (('experiment', 'runs', 0, 'support'), 256)])
@pytest.mark.parametrize("change", ["tuple", "none", "oversize"])
def test_lists_are_bounded_before_member_processing(path, maximum, change):
    value = budget.run_budget_inputs(selected())
    node = value
    for part in path[:-1]: node = node[part]
    node[path[-1]] = () if change == "tuple" else None if change == "none" else [None] * (maximum + 1)
    with pytest.raises(budget.BudgetError, match="budget_array_invalid"):
        budget.parse_run_budget(value)


@pytest.mark.parametrize("kind", ["bool_policy", "zero_policy", "huge_policy", "empty_runs", "duplicate_run",
    "other_peers", "zero_journal", "global_overflow", "bool_pair", "unknown_variant"])
def test_parser_cannot_bypass_original_admission(kind):
    value = budget.run_budget_inputs(selected())
    experiment = value["experiment"]
    if kind == "bool_policy": experiment["capture_policy"]["status_body_bytes"] = True
    if kind == "zero_policy": experiment["capture_policy"]["metrics_body_bytes"] = 0
    if kind == "huge_policy": experiment["capture_policy"]["metrics_body_bytes"] = 1 << 128
    if kind == "empty_runs": experiment["runs"] = []
    if kind == "duplicate_run": experiment["runs"][-1] = deepcopy(experiment["runs"][0])
    if kind == "other_peers": experiment["runs"][1]["geometry"]["peers"] = 5
    if kind == "zero_journal": experiment["runs"][0]["collector_journal"]["max_bytes"] = 0
    if kind == "global_overflow":
        for run in experiment["runs"]: run["collector_journal"]["max_bytes"] = budget.MAX_FILE_BYTES
    if kind == "bool_pair": value["pair_index"] = True
    if kind == "unknown_variant": value["variant"] = "legacy"
    with pytest.raises(budget.BudgetError): budget.parse_run_budget(value)


def test_decoder_does_not_accept_bare_projection_or_infer_parent_admission():
    allocation = selected()
    value = dict(pair_index=1, variant="one_lane", resource_bytes=allocation.resource_bytes,
                 capture_count=allocation.capture_count, journal_bytes=allocation.journal.max_bytes)
    with pytest.raises(budget.BudgetError, match="budget_object_fields_invalid"):
        budget.parse_run_budget(value)


def test_maximum_control_ledger_fits_existing_config_framing_without_raising_cap():
    config = inputs()
    result = budget.admit_experiment(**config)
    extra = tuple(budget.FileBudget(f"extra{i}." + "x" * 100, 1) for i in range(256 - result.control_file_count))
    result = budget.admit_experiment(**(config | {"other_control": extra}))
    allocation = budget.select_run_budget(result, 5, "four_lane")
    value = budget.run_budget_inputs(allocation)
    assert len(json.dumps(value).encode()) < budget.MIB
    assert budget.parse_run_budget(value).experiment.control_file_count == 256
