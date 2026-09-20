"""Current originating custody outcomes with actual files and raw replays.

Native completion/proof authority is an explicit controlled boundary described
in resource_original_custody_fixture. These outcomes do not qualify native trial
handoff or the complete release gate. No child or signal is used.
"""
from dataclasses import replace
import os
from pathlib import Path
import shutil
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts/nexus'))
import resource_experiment as experiment
import scaling_experiment_custody as custody
import scaling_experiment_files as control_files
import scaling_experiment_final_projection as projection
import scaling_measurements as measurements
from resource_original_custody_fixture import (
    original_custody, original_raw, experiment_setup, trial_setup, ready_setup,
)


def test_control_bytes_before_and_after_actual_ten_run_replay_remain_in_one_owner(original_custody):
    c = original_custody
    binding = c.manifest
    expected = (c.root / binding.path).read_bytes()
    assert c.owner._controls.read_control(binding, max_bytes=len(expected)) == expected
    results = c.borrower.collect_replay()
    assert len(results) == 10
    assert c.owner._controls.read_control(binding, max_bytes=len(expected)) == expected
    assert c.borrower.verify() is results
    c.owner.publish_report(c.borrower)
    c.owner.verify_report()
    c.borrower.close()
    c.owner.close()
    with pytest.raises(control_files.ExperimentFileError):
        c.owner._controls.read_control(binding, max_bytes=len(expected))
    assert not c.setup.setup.factory.operations


def test_control_read_never_substitutes_for_required_replay(original_custody):
    c = original_custody
    assert c.owner._controls.read_control(c.manifest, max_bytes=1_000_000)
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    with pytest.raises(control_files.ExperimentFileError):
        c.owner._controls.read_control(c.manifest, max_bytes=1_000_000)


@pytest.mark.parametrize('kind', ('equal_clone', 'unknown', 'digest', 'path', 'label', 'cap'))
def test_bad_control_request_poison_cannot_be_cleared_by_replay_or_verify(original_custody, kind):
    c = original_custody
    original = c.manifest
    request, cap = original, 1_000_000
    if kind == 'equal_clone': request = replace(original)
    elif kind == 'unknown': request = object()
    elif kind == 'digest': request = replace(original, sha256='e' * 64)
    elif kind == 'path': request = replace(original, path='other/path')
    elif kind == 'label': request = replace(original, label='other')
    else: cap = True
    with pytest.raises(control_files.ExperimentFileError):
        c.owner._controls.read_control(request, max_bytes=cap)
    assert c.borrower._results == ()
    for action in (c.borrower.collect_replay, c.borrower.verify):
        with pytest.raises(experiment.ExperimentError): action()
    assert c.owner._phase == 'failed'


@pytest.mark.parametrize('kind', ('run_geometry', 'executable', 'control', 'root'))
def test_scope_drift_rejected_before_control_reader_io(original_custody, monkeypatch, kind):
    c = original_custody
    binding = c.manifest
    calls = []
    selected = c.owner._controls._files['manifest'].fd
    actual = os.pread

    def read(fd, *args):
        if fd == selected: calls.append(args)
        return actual(fd, *args)

    monkeypatch.setattr(os, 'pread', read)
    image = c.owner._runtime.daemon
    original_digest = image.sha256
    try:
        if kind == 'run_geometry': object.__setattr__(c.owner._plan.trials[0].load, 'warmup_ns', 1)
        elif kind == 'executable': image._sha256 = 'b' * 64
        elif kind == 'control': object.__setattr__(binding, 'sha256', 'c' * 64)
        else: c.owner._directories.evidence = c.root / 'other'
        with pytest.raises(ValueError): c.owner._controls.read_control(binding, max_bytes=1_000_000)
        assert calls == []
        with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    finally:
        image._sha256 = original_digest


@pytest.mark.parametrize('error_type', (OSError, RuntimeError, MemoryError, KeyboardInterrupt))
def test_reader_baseexception_invalidates_prior_actual_replay_results(original_custody, monkeypatch, error_type):
    c = original_custody
    assert len(c.borrower.collect_replay()) == 10
    sentinel = error_type('synthetic controlled read interruption')
    selected = c.owner._controls._files['manifest'].fd
    actual = os.pread
    observed = []

    def interrupted(fd, *args):
        if fd == selected:
            observed.append(sentinel)
            raise sentinel
        return actual(fd, *args)

    monkeypatch.setattr(os, 'pread', interrupted)
    public_error = KeyboardInterrupt if error_type is KeyboardInterrupt else control_files.ExperimentFileError
    with pytest.raises(public_error) as raised:
        c.owner._controls.read_control(c.manifest, max_bytes=1_000_000)
    assert observed == [sentinel] and observed[0] is sentinel
    assert raised.value is not sentinel and 'synthetic controlled read interruption' not in str(raised.value)
    # The file owner invalidates its own scope immediately. The borrower cannot
    # use its retained result after consulting that same failed original owner.
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    assert c.borrower._results == () and c.owner._phase == 'failed'


def test_scope_drift_after_actual_reader_rejects_returned_bytes(original_custody, monkeypatch):
    c = original_custody
    binding = c.manifest
    selected = c.owner._controls._files['manifest'].fd
    actual = os.pread
    calls = []
    image = c.owner._runtime.daemon
    original_digest = image.sha256

    def changed(fd, count, offset):
        value = actual(fd, count, offset)
        if fd == selected:
            calls.append((binding, 1_000_000, len(value)))
            image._sha256 = 'b' * 64
        return value

    monkeypatch.setattr(os, 'pread', changed)
    try:
        with pytest.raises(control_files.ExperimentFileError):
            c.owner._controls.read_control(binding, max_bytes=1_000_000)
        assert calls == [(binding, 1_000_000, (c.root / binding.path).stat().st_size)]
        with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    finally:
        image._sha256 = original_digest


def test_successful_read_cannot_mask_later_physical_capture_mutation(original_custody):
    c = original_custody
    assert c.owner._controls.read_control(c.manifest, max_bytes=1_000_000)
    assert len(c.borrower.collect_replay()) == 10
    path = c.member()
    raw = path.read_bytes()
    path.write_bytes(raw.replace(b'queue_size', b'queue_Size'))
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    with pytest.raises(control_files.ExperimentFileError):
        c.owner._controls.read_control(c.manifest, max_bytes=1_000_000)


@pytest.mark.parametrize('when', ['during', 'after'])
def test_tamper_after_a_run_was_replayed_is_caught_by_final_census(original_custody, monkeypatch, when):
    c = original_custody
    actual = experiment.replay
    calls = 0

    def tampered(*args, **kwargs):
        nonlocal calls
        value = actual(*args, **kwargs)
        calls += 1
        if calls == 10 and when == 'during': c.member().write_bytes(b'{"queue_size":999}')
        return value

    monkeypatch.setattr(experiment, 'replay', tampered)
    if when == 'during':
        with pytest.raises(experiment.ExperimentError): c.borrower.collect_replay()
    else:
        c.borrower.collect_replay()
        c.member().write_bytes(b'{"queue_size":999}')
        with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    assert calls == 10 and c.owner._phase == 'failed'
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()


@pytest.mark.parametrize('field', ['runs', 'budget', 'controls', 'root', 'stage', 'executable'])
@pytest.mark.parametrize('when', ['during', 'after'])
def test_mutable_trusted_scope_cannot_drift_across_replay(original_custody, monkeypatch, field, when):
    c = original_custody
    actual = experiment.replay
    calls = 0
    image = c.owner._runtime.daemon
    original_digest = image.sha256

    def change():
        if field == 'runs': object.__setattr__(c.borrower._scopes[-1].peers[0].identity, 'start_abstime', 999)
        elif field == 'budget': object.__setattr__(c.owner._budget.policy, 'status_body_bytes', 1024)
        elif field == 'controls': object.__setattr__(c.manifest, 'sha256', 'd' * 64)
        elif field == 'root': c.owner._directories.evidence = c.root.parent / 'other'
        elif field == 'stage': c.owner._phase = 'reported'
        else: image._sha256 = 'd' * 64

    def changed(*args, **kwargs):
        nonlocal calls
        value = actual(*args, **kwargs)
        calls += 1
        if calls == 1 and when == 'during': change()
        return value

    monkeypatch.setattr(experiment, 'replay', changed)
    try:
        if when == 'during':
            with pytest.raises(experiment.ExperimentError): c.borrower.collect_replay()
            assert calls == 1
        else:
            c.borrower.collect_replay()
            change()
            with pytest.raises(experiment.ExperimentError): c.borrower.verify()
        with pytest.raises(experiment.ExperimentError): c.borrower.verify()
        assert c.owner._phase == 'failed'
    finally:
        image._sha256 = original_digest


def test_root_namespace_replacement_while_semantics_run_fails(original_custody):
    c = original_custody
    c.borrower.collect_replay()
    moved = c.root.with_name('retained')
    c.root.rename(moved)
    shutil.copytree(moved, c.root)
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    assert c.owner._phase == 'failed'


def test_new_reported_context_requires_fresh_ten_run_replay(original_custody):
    c = original_custody
    results = c.borrower.collect_replay()
    assert len(results) == 10
    c.owner.publish_report(c.borrower)
    assert c.owner.verify_report() is c.owner._report[0]
    assert len(c.borrower.verify()) == 10
    c.borrower.close()
    # First release removes report/path readmission entirely. A new borrower
    # cannot refresh this same completed-and-released original owner.
    with pytest.raises(experiment.ExperimentError): experiment.ResourceExperiment.from_completed(c.owner)
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    with pytest.raises(experiment.ExperimentError):
        experiment.ResourceExperiment(root=c.root, reported=True)
    assert c.owner._phase == 'failed'


def test_context_owns_geometry_reconciles_before_final_scan_and_rejects_closed_use(original_custody):
    c = original_custody
    result = c.borrower.collect_replay()[0]
    # Reconciliation now runs inside actual _replay_finish/_measurement_check.
    c.owner._measurement_check(c.borrower)
    assert c.owner._measurements.runs[0].observed_resources == result.maxima
    assert c.borrower.verify()[0] == result
    c.owner.publish_report(c.borrower)
    assert c.owner.verify_report() is c.owner._report[0]
    c.borrower.close()
    c.owner.close()
    with pytest.raises(custody.ExperimentCustodyError): c.owner.publish_report(c.borrower)


@pytest.mark.parametrize('case', ['before_replay', 'wrong_report', 'wrong_pair', 'wrong_variant',
    'geometry_drift', 'interrupt'])
def test_failed_context_reconciliation_cannot_be_promoted_by_a_final_scan(original_custody, monkeypatch, case):
    c = original_custody
    if case in ('wrong_pair', 'wrong_variant'):
        authority = c.owner._runs[0].authority
        actual = authority.reconcile

        def wrong(*args):
            value = actual(*args)
            return value._replace(**({'pair_index': 2} if case == 'wrong_pair' else {'variant': 'four_lane'}))

        monkeypatch.setattr(authority, 'reconcile', wrong)
        action = c.borrower.collect_replay
    elif case == 'interrupt':
        def interrupted(*_): raise KeyboardInterrupt
        monkeypatch.setattr(measurements, 'measure_experiment', interrupted)
        action = c.borrower.collect_replay
    elif case == 'before_replay':
        action = lambda: c.owner.publish_report(c.borrower)
    else:
        c.borrower.collect_replay()
        if case == 'geometry_drift':
            # A valid warmup change preserves the final sample; original scope
            # ownership must reject it before a recomputed measurement can pass.
            object.__setattr__(c.borrower._scopes[0].geometry, 'warmup_ns', 101_000_000)
            action = lambda: c.owner.publish_report(c.borrower)
        else:
            c.owner.publish_report(c.borrower)
            path = c.root / c.owner._report[0].path
            raw = path.read_bytes()
            assert b'"disk_bytes_max":806' in raw
            path.write_bytes(raw.replace(b'"disk_bytes_max":806', b'"disk_bytes_max":807', 1))
            action = c.owner.verify_report
    with pytest.raises((ValueError, KeyboardInterrupt)): action()
    with pytest.raises(experiment.ExperimentError): c.borrower.verify()
    assert c.owner._phase == 'failed' and c.owner._controls._closed is False
