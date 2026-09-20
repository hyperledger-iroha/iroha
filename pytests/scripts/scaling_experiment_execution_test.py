"""Execution glue with explicit mock services; real files and FixedTrial cleanup."""
from pathlib import Path
from types import SimpleNamespace
import hashlib
import pytest
import scaling_experiment_execution as service
from scaling_runtime_admission import RuntimeAdmission
from scaling_fixed_trial import FixedTrial, FixedTrialError

SEED = 'ab' * 32


def setup_service(monkeypatch, tmp_path, *, fail=None, interrupt=None, pending=False, criteria=(True, True, True)):
    events = []
    state = SimpleNamespace(alive=pending, owner=None, borrower=None, verifications=0, cleanup_deadlines=[])
    admission = object.__new__(RuntimeAdmission)
    admission._plan = SimpleNamespace(trials=tuple(SimpleNamespace(stop_timeout_ns=25) for _ in range(10)))
    admission._runtime = object()
    admission._identity = object()
    admission._workers = object()
    def verify(self):
        assert self is admission
        state.verifications += 1
        events.append(('verify', state.verifications))
        if fail == ('verify', state.verifications): raise ValueError('private verification details')
    monkeypatch.setattr(RuntimeAdmission, 'verify', verify)
    monkeypatch.setattr(RuntimeAdmission, 'validate', lambda self: events.append(('validate',)))
    monkeypatch.setattr(service, 'DarwinProcessReader', lambda: object())
    monkeypatch.setattr(service.time, 'monotonic_ns', lambda: 1000)
    evidence = tmp_path / 'evidence'
    runtime = tmp_path / 'runtime'

    class NativeOwner:
        def __init__(self):
            child = SimpleNamespace(process=SimpleNamespace(poll=lambda: None if state.alive else 0))
            self._commands = SimpleNamespace(_children=[child], _bound=lambda child: None)
        def cleanup(self, deadline):
            state.cleanup_deadlines.append(deadline)
            events.append(('cleanup', deadline))
            if fail == 'cleanup': raise RuntimeError('private cleanup details')
            return ('generator',) if state.alive else ()
        def close(self): events.append(('native_close',))

    def actual_trial(index, slot):
        result = object.__new__(FixedTrial)
        for name in ('_proof', '_vectors', '_facts', '_load', '_launch', '_captures', '_outputs', '_peer_inputs', '_directories', '_readiness'):
            setattr(result, name, None)
        result._generator = NativeOwner()
        result._phase = 'ready'
        result._service_index = index
        result._service_slot = slot
        return result
    def trial_run(self, seed):
        assert seed == SEED
        events.append(('run', self._service_index))
        if fail in ('run', 'cleanup'):
            self._phase = 'failed'
            raise interrupt if interrupt is not None else ValueError('private seed child stderr')
        self._phase = 'complete'
    def handoff(self, slot):
        assert slot is self._service_slot
        events.append(('handoff', self._service_index))
        if fail == 'handoff': raise ValueError('private handoff details')
        self.close()
        state.owner._active_pin = None
        return self._service_index
    monkeypatch.setattr(FixedTrial, 'run', trial_run)
    monkeypatch.setattr(FixedTrial, 'handoff', handoff)

    class Owner:
        def __init__(self, got_evidence, got_runtime, plan, budget, native, reader, guard, identity, workers):
            assert got_evidence == evidence and got_runtime == runtime
            assert plan is admission._plan and native is admission._runtime
            assert identity is admission._identity and workers is admission._workers
            assert guard.__self__ is admission
            evidence.mkdir()
            runtime.mkdir()
            self._active_pin = None
            self._plan = plan
            self._measurements = SimpleNamespace(throughput_criterion_met=criteria[0], latency_criterion_met=criteria[1], observed_resource_criterion_met=criteria[2])
            self.closed = False
            self.index = 0
            state.owner = self
        def begin_run(self, pair, variant):
            assert (pair, variant) == service.RUN_KEYS[self.index]
            events.append(('begin', self.index, pair, variant))
            if fail == 'begin': raise ValueError('private begin details')
            index = self.index
            owner = self
            class Slot:
                def create_trial(self):
                    if fail == 'create': raise ValueError('private create details')
                    trial = actual_trial(index, self)
                    owner._active_pin = (self, index, trial, 1100)
                    return trial
            return Slot()
        def publish_run(self, handle):
            assert handle == self.index
            events.append(('publish_run', handle))
            if fail == 'publish_run': raise ValueError('private publication details')
            self.index += 1
        def publish_manifest(self):
            assert self.index == 10
            events.append(('manifest',))
            if fail == 'manifest': raise ValueError('private manifest details')
            (evidence / 'manifest.json').write_bytes(b'manifest')
            self.manifest = SimpleNamespace(sha256=hashlib.sha256(b'manifest').hexdigest())
            return self.manifest
        def publish_report(self, borrower):
            assert borrower is state.borrower and borrower.replayed
            events.append(('report',))
            if fail == 'report': raise ValueError('private report details')
            (evidence / 'report.json').write_bytes(b'report')
            self.report = SimpleNamespace(sha256=hashlib.sha256(b'report').hexdigest())
            return self.report
        def verify_report(self):
            events.append(('verify_report',))
            if fail == 'verify_report': raise ValueError('private reconciliation details')
            assert (evidence / 'report.json').read_bytes() == b'report'
            return self.report if fail != 'binding_identity' else SimpleNamespace(sha256=self.report.sha256)
        def close(self):
            events.append(('owner_close',))
            if self._active_pin is not None:
                self._active_pin[2].close()
            if fail == 'owner_close': raise ValueError('private close details')
            self.closed = True
    class Borrower:
        @classmethod
        def from_completed(cls, owner):
            assert owner is state.owner
            result = cls()
            result.replayed = result.closed = False
            state.borrower = result
            return result
        def collect_replay(self):
            events.append(('replay',))
            if fail == 'replay': raise ValueError('private replay details')
            self.replayed = True
        def close(self):
            events.append(('borrower_close',))
            if fail == 'borrower_close': raise ValueError('private borrower close details')
            self.closed = True
    monkeypatch.setattr(service, 'FixedExperimentCustody', Owner)
    monkeypatch.setattr(service, 'ResourceExperiment', Borrower)
    return admission, evidence, runtime, state, events


def execute(setup):
    admission, evidence, runtime, _, _ = setup
    return service.execute_experiment(admission, evidence, runtime, object(), SEED)


def test_complete_ten_runs_keep_order_and_verify_actual_report_file(monkeypatch, tmp_path):
    fixture = setup_service(monkeypatch, tmp_path)
    result = execute(fixture)
    _, evidence, _, state, events = fixture
    assert [(x[2], x[3]) for x in events if x[0] == 'begin'] == list(service.RUN_KEYS)
    assert [x[1] for x in events if x[0] == 'publish_run'] == list(range(10))
    assert result.report_path == evidence / 'report.json'
    assert result.report_sha256 == hashlib.sha256(result.report_path.read_bytes()).hexdigest()
    assert state.owner.closed and state.borrower.closed
    assert events.index(('manifest',)) < events.index(('replay',)) < events.index(('report',)) < events.index(('verify_report',)) < events.index(('borrower_close',))
    assert not state.cleanup_deadlines


@pytest.mark.parametrize('criteria', [(False, True, True), (True, False, True), (True, True, False), (False, False, False)])
def test_false_criteria_remain_false_observations(monkeypatch, tmp_path, criteria):
    result = execute(setup_service(monkeypatch, tmp_path, criteria=criteria))
    assert (result.throughput_criterion_met, result.latency_criterion_met, result.observed_resource_criterion_met) == criteria


@pytest.mark.parametrize('fail', ['begin', 'create', 'run', 'handoff', 'publish_run', 'manifest', 'replay', 'report', 'verify_report', 'binding_identity', 'borrower_close', ('verify', 1), ('verify', 12), ('verify', 13)])
def test_failure_never_returns_success_or_private_details(monkeypatch, tmp_path, fail):
    fixture = setup_service(monkeypatch, tmp_path, fail=fail)
    with pytest.raises(service.ExperimentExecutionError) as caught:
        execute(fixture)
    assert 'private' not in str(caught.value) and SEED not in str(caught.value)
    assert fixture[3].owner is None or fixture[3].owner.closed


@pytest.mark.parametrize('interrupt', [KeyboardInterrupt(), SystemExit(17), GeneratorExit()])
def test_completed_cleanup_preserves_cancellation_type(monkeypatch, tmp_path, interrupt):
    fixture = setup_service(monkeypatch, tmp_path, fail='run', interrupt=interrupt)
    with pytest.raises(type(interrupt)) as caught: execute(fixture)
    assert fixture[3].owner.closed
    if type(interrupt) is SystemExit: assert caught.value.code == 1


def test_pending_actual_fixed_trial_cleanup_preserves_original_owners(monkeypatch, tmp_path):
    fixture = setup_service(monkeypatch, tmp_path, fail='run', pending=True)
    with pytest.raises(service.ExperimentExecutionError) as caught: execute(fixture)
    error = caught.value
    assert type(error).__name__ == 'ExperimentCleanupRequired'
    assert str(error) == 'fixed_scaling_cleanup_required'
    assert error.cleanup.owns_admission(fixture[0])
    assert not error.cleanup.poll_closed()
    assert not fixture[3].owner.closed
    assert fixture[3].cleanup_deadlines == [1025]
    fixture[3].alive = False
    assert error.cleanup.poll_closed()
    assert error.cleanup.poll_closed()
    assert fixture[3].owner.closed
    assert fixture[3].cleanup_deadlines == [1025]


@pytest.mark.parametrize('interrupt, label', [(KeyboardInterrupt(), 'keyboard_interrupt'), (SystemExit(17), 'system_exit'), (GeneratorExit(), 'generator_exit')])
def test_pending_cancellation_retains_cleanup_authority(monkeypatch, tmp_path, interrupt, label):
    fixture = setup_service(monkeypatch, tmp_path, fail='run', pending=True, interrupt=interrupt)
    with pytest.raises(service.ExperimentExecutionError) as caught: execute(fixture)
    assert type(caught.value).__name__ == 'ExperimentCleanupRequired'
    assert caught.value.interruption == label
    assert not caught.value.cleanup.poll_closed()
    fixture[3].alive = False
    assert caught.value.cleanup.poll_closed()


def test_terminal_descriptor_close_failure_keeps_original_owner(monkeypatch, tmp_path):
    fixture = setup_service(monkeypatch, tmp_path, fail='owner_close')
    with pytest.raises(service.ExperimentExecutionError) as caught: execute(fixture)
    assert type(caught.value).__name__ == 'ExperimentCleanupRequired'
    assert caught.value.cleanup.owns_admission(fixture[0])
    assert not caught.value.cleanup.poll_closed()
    assert not fixture[3].cleanup_deadlines


def test_cleanup_handle_cannot_be_constructed_or_match_foreign_admission(monkeypatch, tmp_path):
    with pytest.raises(service.ExperimentExecutionError): service.PendingExperimentCleanup()
    fixture = setup_service(monkeypatch, tmp_path, fail='run', pending=True)
    with pytest.raises(service.ExperimentCleanupRequired) as caught: execute(fixture)
    class Foreign:
        def __eq__(self, other): raise AssertionError('must compare original identity only')
    assert not caught.value.cleanup.owns_admission(Foreign())
    assert not caught.value.cleanup.owns_admission(object())
    caught.value.cleanup._busy = True
    with pytest.raises(service.ExperimentExecutionError): caught.value.cleanup.poll_closed()
    caught.value.cleanup._busy = False
    fixture[3].alive = False
    assert caught.value.cleanup.poll_closed()


def test_cleanup_exception_keeps_original_child_until_readonly_poll(monkeypatch, tmp_path):
    fixture = setup_service(monkeypatch, tmp_path, fail='cleanup', pending=True)
    with pytest.raises(service.ExperimentCleanupRequired) as caught: execute(fixture)
    assert caught.value.interruption == 'failure'
    assert fixture[3].cleanup_deadlines == [1025]
    fixture[3].alive = False
    assert caught.value.cleanup.poll_closed()
    assert fixture[3].cleanup_deadlines == [1025]


@pytest.mark.parametrize('seed', [None, 12, '', 'a' * 63, 'A' * 64, 'a' * 65])
def test_invalid_seed_rejected_before_any_owner_or_publication(monkeypatch, tmp_path, seed):
    fixture = setup_service(monkeypatch, tmp_path)
    with pytest.raises(service.ExperimentExecutionError):
        service.execute_experiment(fixture[0], fixture[1], fixture[2], object(), seed)
    assert fixture[3].owner is None
    assert not fixture[1].exists()


def test_foreign_runtime_admission_rejected_before_owner(monkeypatch, tmp_path):
    fixture = setup_service(monkeypatch, tmp_path)
    class Child(RuntimeAdmission): pass
    for admission in (None, object(), object.__new__(Child)):
        with pytest.raises(service.ExperimentExecutionError):
            service.execute_experiment(admission, fixture[1], fixture[2], object(), SEED)
    assert fixture[3].owner is None
