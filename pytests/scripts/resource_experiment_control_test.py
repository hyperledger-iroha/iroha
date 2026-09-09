"""Retained control reads compose with the actual ten-run replay context."""
from dataclasses import replace
from pathlib import Path
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import resource_bundle as bundle
import resource_experiment as experiment
from resource_experiment_test import published, valid


def control(valid):
    label = valid.budget.runs[0].collector_journal.label
    return next(item for item in valid.controls if item.label == label)


def test_control_bytes_before_and_after_actual_ten_run_replay_remain_in_one_owner(valid):
    binding = control(valid)
    expected = (valid.root / binding.path).read_bytes()
    with valid.owner() as owner:
        assert owner.read_control(binding, max_bytes=len(expected)) == expected
        results = owner.collect_replay()
        assert len(results) == 10
        assert owner.read_control(binding, max_bytes=len(expected)) == expected
        assert owner.verify() is results
    with pytest.raises(experiment.ExperimentError, match='experiment_closed'):
        owner.read_control(binding, max_bytes=len(expected))


def test_control_read_never_substitutes_for_required_replay(valid):
    binding = control(valid)
    with valid.owner() as owner:
        assert owner.read_control(binding, max_bytes=1_000_000)
        with pytest.raises(experiment.ExperimentError, match='resource_replay_incomplete'):
            owner.verify()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.read_control(binding, max_bytes=1_000_000)


@pytest.mark.parametrize('kind', ('equal_clone', 'unknown', 'digest', 'path', 'label', 'cap'))
def test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify(valid, kind):
    original = control(valid)
    request, cap = original, 1_000_000
    if kind == 'equal_clone': request = replace(original)
    elif kind == 'unknown': request = object()
    elif kind == 'digest': request = replace(original, sha256='e' * 64)
    elif kind == 'path': request = replace(original, path='other/path')
    elif kind == 'label': request = replace(original, label='other')
    else: cap = True
    with valid.owner() as owner:
        with pytest.raises(bundle.BundleError):
            owner.read_control(request, max_bytes=cap)
        assert owner._results == ()
        for action in (owner.collect_replay, owner.verify):
            with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
                action()


@pytest.mark.parametrize('kind', ('run_geometry', 'executable', 'control', 'root'))
def test_scope_drift_rejected_before_control_reader_io(valid, monkeypatch, kind):
    binding = control(valid)
    calls = []
    with valid.owner() as owner:
        monkeypatch.setattr(owner._bundle, 'read_control', lambda *a, **kw: calls.append(kw))
        if kind == 'run_geometry':
            owner._runs = (replace(owner._runs[0], geometry=replace(valid.geometry, warmup_ns=1)), *owner._runs[1:])
        elif kind == 'executable': owner._expected_executable_sha256 = 'b' * 64
        elif kind == 'control': owner._controls = tuple(replace(item, sha256='c' * 64) for item in owner._controls)
        else: owner._root = valid.root / 'other'
        with pytest.raises(ValueError):
            owner.read_control(binding, max_bytes=1_000_000)
        assert calls == []
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()


@pytest.mark.parametrize('error_type', (OSError, RuntimeError, MemoryError, KeyboardInterrupt))
def test_reader_baseexception_invalidates_prior_actual_replay_results(valid, monkeypatch, error_type):
    with valid.owner() as owner:
        assert len(owner.collect_replay()) == 10
        sentinel = error_type('synthetic controlled read interruption')
        def interrupted(*args, **kwargs):
            raise sentinel
        monkeypatch.setattr(owner._bundle, 'read_control', interrupted)
        with pytest.raises(error_type) as raised:
            owner.read_control(control(valid), max_bytes=1_000_000)
        assert raised.value is sentinel and owner._results == ()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()


def test_scope_drift_after_actual_reader_rejects_returned_bytes(valid, monkeypatch):
    binding = control(valid)
    with valid.owner() as owner:
        actual = owner._bundle.read_control
        calls = []
        def changed(request, *, max_bytes):
            value = actual(request, max_bytes=max_bytes)
            calls.append((request, max_bytes, len(value)))
            owner._expected_executable_sha256 = 'b' * 64
            return value
        monkeypatch.setattr(owner._bundle, 'read_control', changed)
        with pytest.raises(experiment.ExperimentError):
            owner.read_control(binding, max_bytes=1_000_000)
        assert calls == [(binding, 1_000_000, (valid.root / binding.path).stat().st_size)]
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.verify()


def test_successful_read_cannot_mask_later_physical_capture_mutation(valid):
    with valid.owner() as owner:
        assert owner.read_control(control(valid), max_bytes=1_000_000)
        assert len(owner.collect_replay()) == 10
        path = valid.member()
        raw = path.read_bytes()
        path.write_bytes(raw.replace(b'queue_size', b'queue_Size'))
        with pytest.raises(bundle.BundleError):
            owner.verify()
        with pytest.raises(experiment.ExperimentError, match='experiment_failed'):
            owner.read_control(control(valid), max_bytes=1_000_000)
