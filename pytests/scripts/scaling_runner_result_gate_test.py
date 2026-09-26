"""The sole fixed V1 runner publishes only a complete typed owner outcome."""

from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest

import run_multilane_scaling_gate as runner
from scaling_experiment_execution import ExperimentOutcome


ARGS = ('--launch-input-fd', '3', '--launch-input-sha256', 'a' * 64, '--seed-fd', '4')
SEED = 'b' * 64


def admitted_runner(monkeypatch, tmp_path, outcome, *, close_failure=False):
    """Hold the runner at its typed invocation boundary with exact cleanup reads."""
    import scaling_cli_bootstrap as bootstrap
    import scaling_experiment_cli_inputs as cli_inputs
    import scaling_experiment_invocation as invocation

    events = []
    launch = SimpleNamespace(evidence_root=tmp_path / 'evidence')

    class Dependencies:
        def close(self):
            events.append('dependencies_close')
            if close_failure:
                raise RuntimeError('private dependency detail')

    class Invocation:
        def run(self, seed):
            assert seed == SEED
            events.append('run')
            return outcome

        def poll_cleanup(self):
            events.append('poll_cleanup')
            return True

        def close(self):
            events.append('invocation_close')

    dependencies = Dependencies()
    owned = Invocation()
    monkeypatch.setattr(runner, '_install_private_paths', lambda: events.append('paths'))
    monkeypatch.setattr(cli_inputs, 'read_launch_descriptor', lambda fd, digest: b'launch')
    monkeypatch.setattr(cli_inputs, 'decode_launch_input', lambda raw: launch)
    monkeypatch.setattr(cli_inputs, 'read_seed_descriptor', lambda fd: SEED)
    monkeypatch.setattr(bootstrap, 'bootstrap_runtime', lambda raw: dependencies)

    def admit(selected, originals):
        assert selected is launch and originals is dependencies
        events.append('admit')
        return owned

    monkeypatch.setattr(invocation.InvocationResources, 'admit', admit)
    return launch, events


@pytest.mark.parametrize(
    'criteria,exit_code',
    (
        ((True, True, True), 0),
        ((False, True, True), 1),
        ((True, False, True), 1),
        ((True, True, False), 1),
    ),
)
def test_runner_reports_only_all_three_observed_criteria(
    monkeypatch, tmp_path, capsys, criteria, exit_code,
):
    outcome = ExperimentOutcome(
        tmp_path / 'evidence/report.json', 'c' * 64, 'd' * 64, *criteria,
    )
    _, events = admitted_runner(monkeypatch, tmp_path, outcome)
    assert runner.main(ARGS) == exit_code
    output = capsys.readouterr()
    assert output.out == f"[g-scale] {'PASS' if exit_code == 0 else 'FAIL'}: report_sha256={'d' * 64}\n"
    assert output.err == ''
    assert events == ['paths', 'admit', 'run', 'poll_cleanup', 'invocation_close', 'dependencies_close']


@pytest.mark.parametrize('mutation', ('foreign_path', 'bad_digest', 'integer_criterion', 'foreign_type'))
def test_runner_cannot_publish_a_foreign_or_malformed_outcome(
    monkeypatch, tmp_path, capsys, mutation,
):
    original = ExperimentOutcome(
        tmp_path / 'evidence/report.json', 'c' * 64, 'd' * 64, True, True, True,
    )
    if mutation == 'foreign_path':
        outcome = replace(original, report_path=Path('/tmp/foreign-report.json'))
    elif mutation == 'bad_digest':
        outcome = replace(original, report_sha256='D' * 64)
    elif mutation == 'integer_criterion':
        outcome = replace(original, throughput_criterion_met=1)
    else:
        outcome = SimpleNamespace(**{name: getattr(original, name) for name in original.__dataclass_fields__})
    _, events = admitted_runner(monkeypatch, tmp_path, outcome)
    assert runner.main(ARGS) == 2
    output = capsys.readouterr()
    assert output.out == '' and output.err == '[g-scale] FAIL: fixed experiment did not complete\n'
    assert events[-3:] == ['poll_cleanup', 'invocation_close', 'dependencies_close']


def test_runner_closure_failure_cannot_leave_a_pass_result(monkeypatch, tmp_path, capsys):
    outcome = ExperimentOutcome(
        tmp_path / 'evidence/report.json', 'c' * 64, 'd' * 64, True, True, True,
    )
    _, events = admitted_runner(monkeypatch, tmp_path, outcome, close_failure=True)
    assert runner.main(ARGS) == 2
    output = capsys.readouterr()
    assert output.out == '' and output.err == '[g-scale] FAIL: fixed experiment did not complete\n'
    assert events[-3:] == ['poll_cleanup', 'invocation_close', 'dependencies_close']


@pytest.mark.parametrize(
    'retired',
    ('--trial-command', '--min-interval-samples', '--drain-seconds',
     '--max-submission-lag-ms', '--skip', '--continue-on-failure', '--report'),
)
def test_runner_parser_rejects_retired_overrides_before_owner_admission(retired, capsys):
    with pytest.raises(SystemExit) as caught:
        runner.parse_args((*ARGS, retired, '1'))
    assert caught.value.code == 2
    assert capsys.readouterr().err == '[g-scale] FAIL: invalid fixed launch arguments\n'


def test_fixed_runner_source_has_no_cargo_or_external_trial_command():
    root = Path(__file__).resolve().parents[2]
    python_source = (root / 'scripts/nexus/run_multilane_scaling_gate.py').read_text()
    shell_source = (root / 'scripts/nexus/run_multilane_scaling_gate.sh').read_text()
    assert 'cargo' not in python_source.lower() + shell_source.lower()
    assert 'subprocess' not in python_source
    assert '--trial-command' not in python_source + shell_source
    assert '--skip' not in python_source + shell_source


def test_runbook_names_current_owner_measurements_and_parent_handoff():
    root = Path(__file__).resolve().parents[2]
    runbook = (root / 'specs/sumeragi_v2_multilane_scaling_gate.md').read_text()
    for required in (
        'five paired one-lane/four-lane measurements',
        'FixedExperimentCustody',
        'ResourceExperiment.from_completed',
        '`3/2`',
        '`5/4`',
        '`scaling-execution.json`',
        'native proof replays',
    ):
        assert required in runbook
    assert 'scaling_evidence.json' not in runbook
    assert 'validation_report.json' not in runbook


def test_third_failed_trial_never_starts_a_later_pair_or_publishes_report(
    monkeypatch, tmp_path,
):
    import scaling_experiment_execution as execution
    from scaling_experiment_execution_test import setup_service, execute
    from scaling_fixed_trial import FixedTrial

    fixture = setup_service(monkeypatch, tmp_path)
    original_run = FixedTrial.run

    def fail_third(self, seed):
        if self._service_index == 2:
            self._phase = 'failed'
            raise ValueError('private trial failure')
        return original_run(self, seed)

    monkeypatch.setattr(FixedTrial, 'run', fail_third)
    with pytest.raises(execution.ExperimentExecutionError):
        execute(fixture)
    events = fixture[4]
    assert [row[1] for row in events if row[0] == 'begin'] == [0, 1, 2]
    assert not any(row[0] in ('manifest', 'replay', 'report') for row in events)
    assert fixture[3].owner.closed
