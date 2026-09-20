"""Original-policy guards without per-file JSON; pure and real-file regressions."""
from dataclasses import fields, is_dataclass
import os

import pytest

import resource_evidence_budget as budget
import scaling_experiment_plan as plans
import scaling_structural_identity as identity
import scaling_experiment_custody as custody
import scaling_experiment_files as controls
import scaling_public_files as public
import scaling_trial_captures as captures
from scaling_experiment_custody_test import (
    fixed_plan, budget_for, experiment_setup, trial_setup, ready_setup,
)
from scaling_experiment_files_test import budget as control_budget
from scaling_public_files_test import TrialFiles
from resource_replay_test import Fixture


def values():
    declared = fixed_plan()
    plan, experiment, raw = plans.admit_plan(declared, budget_for(declared))
    selected = budget.select_run_budget(experiment, 1, 'one_lane')
    canonical = budget.canonical_run_budget_bytes(selected)
    return ((plan, raw, identity.pin_experiment_plan),
            (experiment, canonical, identity.pin_experiment_budget),
            (selected, canonical, identity.pin_run_budget))


def clone(value):
    if type(value) is tuple: return tuple(clone(item) for item in value)
    if is_dataclass(value):
        return type(value)(*(clone(getattr(value, field.name)) for field in fields(type(value))))
    return value


def paths(value, path=()):
    yield path, value
    if type(value) is tuple:
        for index, item in enumerate(value): yield from paths(item, (*path, index))
    elif is_dataclass(value):
        for field in fields(type(value)):
            yield from paths(getattr(value, field.name), (*path, field.name))


def replace_path(root, path, replacement):
    if not path: return replacement
    parent = root
    for name in path[:-1]:
        parent = parent[name] if type(name) is int else getattr(parent, name)
    name = path[-1]
    if type(name) is str:
        object.__setattr__(parent, name, replacement)
        return root
    changed = tuple(replacement if index == name else item for index, item in enumerate(parent))
    return replace_path(root, path[:-1], changed)


class Hostile:
    def __eq__(self, other): raise AssertionError('foreign equality must not run')
    def __deepcopy__(self, memo): raise AssertionError('foreign copying must not run')


class IntSubclass(int):
    def __eq__(self, other): raise AssertionError('integer subclass equality must not run')


class StrSubclass(str):
    def __eq__(self, other): raise AssertionError('string subclass equality must not run')


class TupleSubclass(tuple):
    pass


@pytest.mark.parametrize('index,expected', [(0, 2607), (1, 1573), (2, 1719)])
def test_every_nested_value_type_container_and_summary_is_pinned(index, expected):
    original, raw, factory = values()[index]
    pin = factory(original, raw)
    assert pin.checked_bytes(original, raw) is raw
    assert pin.checked_bytes(clone(original), raw) is raw
    rejected = 0
    for path, value in paths(original):
        if type(value) is int:
            cases = (value + 1, True, IntSubclass(value), Hostile())
        elif type(value) is str:
            cases = (value + 'x', StrSubclass(value), Hostile())
        elif type(value) is tuple:
            cases = (list(value), TupleSubclass(value), (*value, None), Hostile())
        else:
            cases = (Hostile(),)
        for changed in cases:
            with pytest.raises(identity.StructuralIdentityError):
                pin.checked_bytes(replace_path(clone(original), path, changed), raw)
            rejected += 1
    assert rejected == expected
    with pytest.raises(identity.StructuralIdentityError): pin.checked_bytes(original, raw + b' ')
    with pytest.raises(identity.StructuralIdentityError): pin.checked_bytes(original, bytearray(raw))


@pytest.mark.parametrize('index', range(3))
def test_pin_has_no_source_record_alias_and_guard_never_serializes(index, monkeypatch):
    original, raw, factory = values()[index]
    pin = factory(original, raw)
    def fail(*args, **kwargs): raise AssertionError('guard must not reserialize or readmit')
    for name in ('canonical_run_budget_bytes', 'run_budget_inputs', 'select_run_budget', '_readmit'):
        monkeypatch.setattr(budget, name, fail)
    monkeypatch.setattr(plans, 'plan_bytes', fail)
    assert pin.checked_bytes(original, raw) == raw
    def check(item):
        assert not is_dataclass(item) or type(item) is type
        if type(item) is tuple:
            for child in item: check(child)
    check(pin._schema);check(pin._values)
    assert all(type(item) in (int, str) for item in pin._values)


@pytest.mark.parametrize('index', range(3))
def test_factory_rejects_foreign_roots_and_unmatched_original_bytes(index):
    original, raw, factory = values()[index]
    with pytest.raises(identity.StructuralIdentityError): factory(original, raw + b' ')
    with pytest.raises(identity.StructuralIdentityError): factory(Hostile(), raw)
    with pytest.raises(identity.StructuralIdentityError): factory(original, b'x' * (1024*1024+1))


@pytest.mark.parametrize('selected', [False, True])
def test_factory_preserves_semantic_rejection_of_forged_ledger_summaries(selected):
    original, raw, factory = values()[2 if selected else 1]
    ledger = original.experiment if selected else original
    object.__setattr__(ledger, 'total_bytes', ledger.total_bytes + 1)
    with pytest.raises(budget.BudgetError): factory(original, raw)


@pytest.mark.parametrize('case', ['nodes', 'depth', 'tuple_size', 'integer_size', 'string_size'])
def test_capture_bounds_all_nodes_before_semantic_encoder(case, monkeypatch):
    original, raw, factory = values()[1]
    if case == 'nodes':
        object.__setattr__(original, 'static_files', tuple(tuple(() for _ in range(64)) for _ in range(256)))
    elif case == 'depth':
        nested = ()
        for _ in range(9): nested = (nested,)
        object.__setattr__(original, 'static_files', nested)
    elif case == 'tuple_size': object.__setattr__(original, 'static_files', ((),) * 257)
    elif case == 'integer_size': object.__setattr__(original, 'total_bytes', 1 << 1000)
    else: object.__setattr__(original.static_files[0], 'label', 'x' * 4097)
    def fail(*args, **kwargs): raise AssertionError('unbounded tree reached semantic encoder')
    monkeypatch.setattr(budget, 'canonical_run_budget_bytes', fail)
    with pytest.raises(identity.StructuralIdentityError): factory(original, raw)


@pytest.mark.parametrize('index', range(3))
def test_exact_root_record_type_rejects_dataclass_subclasses(index):
    original, raw, factory = values()[index]
    pin = factory(original, raw)
    foreign = type('ForeignRecord', (type(original),), {})(
        *(getattr(original, field.name) for field in fields(type(original))))
    with pytest.raises(identity.StructuralIdentityError): pin.checked_bytes(foreign, raw)
    with pytest.raises(identity.StructuralIdentityError): factory(foreign, raw)


def test_public_file_guard_rechecks_future_run_without_serialization(tmp_path, monkeypatch):
    owner = TrialFiles(tmp_path.resolve())
    try:
        def fail(*args, **kwargs): raise AssertionError('hot guard serialized')
        monkeypatch.setattr(public, 'canonical_run_budget_bytes', fail)
        monkeypatch.setattr(budget, 'canonical_run_budget_bytes', fail)
        owner.public._check()
        future = owner.public._allocation.experiment.runs[-1].genesis_anchors
        object.__setattr__(future, 'max_bytes', future.max_bytes + 1)
        with pytest.raises(identity.StructuralIdentityError): owner.public._check()
    finally: owner.close()


def test_experiment_control_guard_pins_all_derived_totals_without_serialization(tmp_path, monkeypatch):
    root = tmp_path.resolve()/'evidence';root.mkdir(mode=0o700)
    fd = os.open(root, controls._DIRECTORY)
    owner = controls.FixedExperimentFiles(root, fd, control_budget(), lambda: None)
    try:
        def fail(*args, **kwargs): raise AssertionError('hot guard serialized')
        monkeypatch.setattr(controls, '_budget_bytes', fail)
        owner.check_namespace()
        object.__setattr__(owner._allocation, 'resource_member_count',
                           owner._allocation.resource_member_count + 1)
        with pytest.raises(controls.ExperimentFileError): owner.check_namespace()
        with pytest.raises(controls.ExperimentFileError): owner.check_namespace()
    finally: owner.close();os.close(fd)


def test_transferred_capture_keeps_original_pin_after_private_close(tmp_path, monkeypatch):
    fixture = Fixture(tmp_path.resolve())
    owner = captures.TrialCaptures(fixture.directory, fixture.allocation, lambda: None)
    transferred = owner.transfer(lambda: None)
    owner.close()
    try:
        def fail(*args, **kwargs): raise AssertionError('hot guard serialized')
        monkeypatch.setattr(captures, 'canonical_run_budget_bytes', fail)
        transferred.check_namespace()
        future = transferred._allocation.experiment.runs[-1].native_bundle
        object.__setattr__(future, 'label', StrSubclass(future.label))
        with pytest.raises(identity.StructuralIdentityError): transferred.check_namespace()
        with pytest.raises(captures.TrialCaptureError): transferred.check_namespace()
    finally: transferred.close()


@pytest.mark.parametrize('case', ['plan_value', 'plan_type', 'budget', 'plan_bytes', 'budget_bytes'])
def test_original_experiment_scope_has_no_json_and_rejects_any_later_policy_change(experiment_setup, monkeypatch, case):
    owner = experiment_setup.create()
    def fail(*args, **kwargs): raise AssertionError('hot guard serialized')
    monkeypatch.setattr(custody, 'plan_bytes', fail)
    monkeypatch.setattr(custody, 'canonical_run_budget_bytes', fail)
    monkeypatch.setattr(controls, '_budget_bytes', fail)
    owner._scope_check()
    if case == 'plan_value':
        final = owner._plan.trials[-1].load
        object.__setattr__(final, 'offered_load_tps', '501')
    elif case == 'plan_type':
        object.__setattr__(owner._plan.trials[-1].generator, 'lane_count', IntSubclass(4))
    elif case == 'budget':
        future = owner._budget.runs[-1].genesis_manifest
        object.__setattr__(future, 'max_bytes', future.max_bytes + 1)
    elif case == 'plan_bytes': owner._plan_raw += b' '
    else: owner._budget_raw += b' '
    with pytest.raises(identity.StructuralIdentityError): owner._scope_check()
    assert owner._phase == 'failed'
    with pytest.raises(custody.ExperimentCustodyError): owner._scope_check()
