"""Final guard units with real original inputs and explicitly synthetic run state.

These tests initialize an actual experiment's image/source/root/control custody.
They then install exact typed borrower, result and authority records to isolate
guard failures. They never admit a completed native trial or verify a proof;
the separately owned ten-trial composition covers that production pipeline.
"""
from types import SimpleNamespace

import pytest

from scaling_experiment_custody_test import experiment_setup, trial_setup, ready_setup
from scaling_experiment_custody import ExperimentCustodyError
from scaling_completed_authority import CompletedRunAuthority, CompletedAuthorityError
from scaling_experiment_final_projection import measurement_identity
from scaling_experiment_plan import RUN_KEYS
from scaling_measurements import measure_experiment
from scaling_measurements_test import run as synthetic_projection
from resource_bundle import ControlBinding
from resource_evidence_budget import select_run_budget
from resource_experiment import ResourceExperiment, RunReplayScope, RunResourceResult, ExperimentError
from resource_process import ProcessIdentity
from resource_replay import ExpectedPeer, ReplayGeometry


@pytest.fixture
def guarded_replay(experiment_setup):
    """Register only pure exact-type guard state, with no native admission claim."""
    callbacks = []
    state = {'owner': None, 'action': None, 'remaining': 0}

    def runtime_callback():
        callbacks.append('runtime')
        action = state['action']
        if action is not None:
            state['remaining'] -= 1
            if state['remaining']:
                return
            state['action'] = None
            action(state['owner'])

    owner = experiment_setup.create(verify_runtime=runtime_callback)
    values = tuple(synthetic_projection(trial) for trial in owner._plan.trials)
    # The pure measurement fixture permits PID 1; actual raw replay reserves it.
    # Supply the canonical replay's valid synthetic identities before admission.
    values = tuple(value._replace(peers=tuple(peer._replace(pid=index + 2)
        for index, peer in enumerate(value.peers))) for value in values)
    measured = measure_experiment(owner._plan, values)
    scopes = []
    for (pair, variant), value in zip(RUN_KEYS, values, strict=True):
        peers = tuple(ExpectedPeer(f'peer{i}', ProcessIdentity(*peer)) for i, peer in enumerate(value.peers))
        scopes.append(RunReplayScope(pair, variant,
            owner._directories.evidence / 'resources' / f'pair-{pair:02}' / variant,
            owner._directories.evidence / 'runs' / f'pair-{pair:02}' / variant / 'collector.jsonl',
            'a' * 64, peers, ReplayGeometry(*value.geometry), select_run_budget(owner._budget, pair, variant)))
    borrower = object.__new__(ResourceExperiment)
    token = object()
    borrower._initialize(owner, token, tuple(scopes))
    results = tuple(RunResourceResult(*key, value.resources, metric.observed_resources)
        for key, value, metric in zip(RUN_KEYS, values, measured.runs, strict=True))
    borrower._results = results
    borrower._results_pin = borrower._result_identity()
    borrower._phase = 'complete'
    owner._replay = borrower
    owner._replay_pin = (borrower, token, borrower._scope_pin)
    owner._replayed = owner._replayed_pin = values
    owner._replay_results = results
    owner._replay_results_pin = borrower._result_identity()
    owner._measurements = measured
    owner._measurement_pin = measurement_identity(measured)
    owner._manifest = owner._manifest_pin = (ControlBinding('manifest', 'manifest.json', 'b' * 64), b'{}')
    owner._phase = 'replayed'
    state['owner'] = owner
    callbacks.clear()
    # These checks exercise actual pure borrower and originating semantic owners.
    borrower._validate(('complete',))
    owner._semantic_pins()
    yield SimpleNamespace(owner=owner, borrower=borrower, results=results,
                          state=state, callbacks=callbacks, setup=experiment_setup)
    state['action'] = None


def test_unmodified_exact_guard_state_passes_without_any_native_admission(guarded_replay):
    c = guarded_replay
    assert c.borrower.verify() is c.results
    assert c.owner._phase == 'replayed' and c.borrower._phase == 'complete'
    assert len(c.owner._replay_results_pin) == len(c.owner._replayed) == 10
    assert c.owner._runs == [] and c.owner._run_pins == ()
    assert not c.setup.setup.factory.operations


def corrupt(owner, case):
    if case == 'measurements_none': owner._measurements = None
    elif case == 'measurement_pin_none': owner._measurement_pin = None
    elif case == 'measurement_and_pin_none': owner._measurements = owner._measurement_pin = None
    elif case == 'measurement_scalar':
        object.__setattr__(owner._measurements, 'one_lane_pooled_p95_latency_ns',
                           owner._measurements.one_lane_pooled_p95_latency_ns + 1)
    elif case == 'measurement_resource':
        resource = owner._measurements.runs[0].observed_resources
        object.__setattr__(resource, 'memory_bytes_max', resource.memory_bytes_max + 1)
    elif case == 'result_tuple_none': owner._replay_results = None
    elif case == 'result_pin_none': owner._replay_results_pin = None
    elif case == 'result_pin_empty': owner._replay_results_pin = ()
    elif case == 'result_tuple_and_pin_none': owner._replay_results = owner._replay_results_pin = None
    elif case == 'borrower_result_tuple_empty': owner._replay._results = ()
    elif case == 'borrower_result_pin_empty': owner._replay._results_pin = ()
    elif case == 'borrower_results_and_pin_empty': owner._replay._results = owner._replay._results_pin = ()
    elif case == 'replay_scope_mutation':
        object.__setattr__(owner._replay._scopes[0], 'journal_sha256', 'c' * 64)
    elif case == 'earlier_projection_replaced':
        original = owner._replayed[0]
        changed = original.requests[0]._replace(applied_offset_ns=original.requests[0].applied_offset_ns + 1)
        owner._replayed = (original._replace(requests=(changed, *original.requests[1:])), *owner._replayed[1:])
    elif case == 'projection_pin_cleared': owner._replayed_pin = ()
    elif case == 'original_token_changed': owner._token = object()
    elif case == 'manifest_tuple_replaced': owner._manifest = tuple(list(owner._manifest))
    elif case == 'manifest_pin_cleared': owner._manifest_pin = None
    elif case == 'borrower_token_changed': owner._replay._token = object()
    elif case == 'borrower_scope_pin_replaced': owner._replay._scope_pin = tuple(list(owner._replay._scope_pin))
    else: raise AssertionError('unknown test mutation')


@pytest.mark.parametrize('case', ['measurements_none', 'measurement_pin_none', 'measurement_and_pin_none',
    'measurement_scalar', 'measurement_resource', 'result_tuple_none', 'result_pin_none', 'result_pin_empty',
    'result_tuple_and_pin_none', 'borrower_result_tuple_empty', 'borrower_result_pin_empty',
    'borrower_results_and_pin_empty', 'replay_scope_mutation', 'earlier_projection_replaced',
    'projection_pin_cleared', 'original_token_changed', 'manifest_tuple_replaced', 'manifest_pin_cleared',
    'borrower_token_changed', 'borrower_scope_pin_replaced'])
def test_last_runtime_callback_mutations_fail_the_original_public_verification(guarded_replay, case):
    c = guarded_replay
    c.state['action'] = lambda owner: corrupt(owner, case)
    # The second _full_check follows _measurement_check. No later recomputation
    # can accidentally mask a missing terminal semantic fence in this test.
    c.state['remaining'] = 2
    with pytest.raises((ExperimentError, ExperimentCustodyError)):
        c.borrower.verify()
    assert c.state['action'] is None and c.callbacks == ['runtime', 'runtime']
    assert c.owner._phase == 'failed' and c.borrower._phase == 'failed'
    assert c.owner._report is None
    with pytest.raises(ExperimentError): c.borrower.verify()
    assert c.owner._phase == 'failed'
    assert not c.setup.setup.factory.operations


def test_final_full_check_orders_semantic_values_before_metadata_without_later_callback(guarded_replay, monkeypatch):
    c = guarded_replay
    events = []
    checked = {'values': False}
    original_callback = c.owner._callback
    def callback():
        assert not checked['values']
        events.append('runtime')
        original_callback()
    c.owner._callback = c.owner._original_callback = callback
    verify = c.owner._controls.verify
    def control_body_reads():
        assert not checked['values']
        events.append('body_reads')
        return verify()
    monkeypatch.setattr(c.owner._controls, 'verify', control_body_reads)
    semantic = c.owner._semantic_pins
    def values():
        assert events[-1] == 'body_reads'
        semantic()
        events.append('values')
        checked['values'] = True
    monkeypatch.setattr(c.owner, '_semantic_pins', values)
    metadata = c.owner._metadata_fence
    def fence():
        assert checked['values'] and events[-1] == 'values'
        metadata()
        events.append('metadata')
    monkeypatch.setattr(c.owner, '_metadata_fence', fence)
    c.owner._full_check()
    assert events == ['runtime', 'body_reads', 'values', 'metadata']
    assert c.owner._phase == 'replayed'
    # Fixture cleanup closes custody only; it does not invoke the callback.


@pytest.mark.parametrize('phase', ['replayed', 'reported', 'replaying'])
def test_terminal_phase_cannot_clear_all_measurement_and_result_pins(guarded_replay, phase):
    owner = guarded_replay.owner
    owner._phase = phase
    if phase == 'replaying':
        # _replay_finish has installed its results, but not advanced the owner.
        guarded_replay.borrower._phase = 'finishing'
    owner._measurements = owner._measurement_pin = None
    owner._replay_results = owner._replay_results_pin = None
    with pytest.raises(ExperimentCustodyError): owner._semantic_pins()


def authority(token):
    """Exact authority object for close/provenance units, without a source proof."""
    value = object.__new__(CompletedRunAuthority)
    value._token = token
    value._phase = 'committed'
    value._busy = False
    value._source = value._original_source = object()
    value._trial = value._files = value._captures = None
    return value


@pytest.mark.parametrize('failed_index', [0, 1, 2])
def test_poison_continues_invalidating_each_original_authority_after_one_rejects(experiment_setup, failed_index):
    owner = experiment_setup.create()
    originals = [authority(owner._original_token) for _ in range(3)]
    originals[failed_index]._token = object()
    pending = authority(owner._original_token)
    owner._run_pins = tuple((object(), value, None, None) for value in originals)
    owner._pending_authority = pending
    try:
        owner._token = object()  # Cleanup must still use the immutable original token.
        owner._poison()
        assert owner._phase == 'failed'
        assert [value._phase for value in originals] == ['failed' if i == failed_index else 'closed' for i in range(3)]
        assert pending._phase == 'closed'
        for value in originals:
            with pytest.raises(CompletedAuthorityError): value.check_provenance(owner._original_token)
        assert not experiment_setup.setup.factory.operations
    finally:
        # The pure unit supplies no physical run owners for fixture close().
        owner._run_pins = ()
        owner._pending_authority = None


def test_scalar_guard_rejects_original_token_change_before_any_projection_use(experiment_setup):
    owner = experiment_setup.create()
    owner._token = object()
    with pytest.raises(ExperimentCustodyError): owner._scalar_pins()
    with pytest.raises(ExperimentCustodyError): owner._scope_check()
    assert owner._phase == 'failed'
    assert not experiment_setup.setup.factory.operations
