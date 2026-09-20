"""Fixed CLI argument and ownership orchestration tests with explicit service doubles."""
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import dataclass
import importlib.util
import io
import itertools
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch

_ROOT = Path(__file__).resolve().parents[2]
_SPEC = importlib.util.spec_from_file_location('fixed_command_under_test',
    _ROOT / 'scripts/nexus/run_multilane_scaling_gate.py')
command = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(command)
_ARGS = ['--launch-input-fd', '10', '--launch-input-sha256', 'a' * 64, '--seed-fd', '11']


@dataclass(frozen=True)
class OutcomeFixture:
    """Service-result seam; it does not represent executed native evidence."""
    report_path: Path
    manifest_sha256: str
    report_sha256: str
    throughput_criterion_met: bool
    latency_criterion_met: bool
    observed_resource_criterion_met: bool


class CleanupFixture(ValueError):
    """Explicit pending-cleanup control at the tested command/service boundary."""
    def __init__(self, interruption):
        self.interruption = interruption


class CommandArgumentsTests(unittest.TestCase):
    """Retired commands, duplicate inputs and aliased descriptors cannot enter launch."""
    def test_exact_inputs(self):
        args = command.parse_args(_ARGS)
        self.assertEqual((args.launch_input_fd, args.launch_input_sha256, args.seed_fd),
                         (10, 'a' * 64, 11))

    def test_rejects_retired_duplicate_unknown_missing_and_abbreviated_arguments(self):
        variants = [_ARGS + ['--trial-command', 'PRIVATE_VALUE'], _ARGS + ['--seed', 'PRIVATE_VALUE'],
                    _ARGS + ['--seed-fd', '12'], _ARGS + ['--launch-input-fd', '12'],
                    _ARGS + ['--launch-input-sha256', 'b' * 64], _ARGS[:-2],
                    ['--launch-input-f', *_ARGS[1:]], _ARGS[:-1] + ['10']]
        for argv in variants:
            error = io.StringIO()
            with self.subTest(argv=argv), redirect_stderr(error), self.assertRaises(SystemExit) as raised:
                command.parse_args(argv)
            self.assertEqual(raised.exception.code, 2)
            self.assertEqual(error.getvalue(), '[g-scale] FAIL: invalid fixed launch arguments\n')
            self.assertNotIn('PRIVATE_VALUE', error.getvalue())

    def test_rejects_invalid_descriptor_and_digest_forms(self):
        for value in ('-1', '0', '1', '2', str(1 << 20), '+3', '3.0', '1e3', 'PRIVATE_VALUE'):
            with self.subTest(value=value), redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                command.parse_args(_ARGS[:-1] + [value])
        for value in ('A' * 64, 'a' * 63, 'a' * 65, 'z' * 64, 'PRIVATE_VALUE'):
            with self.subTest(value=value), redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                command.parse_args(_ARGS[:3] + [value] + _ARGS[4:])

    def test_help_needs_no_runtime_or_secret(self):
        with redirect_stdout(io.StringIO()) as output, self.assertRaises(SystemExit) as raised:
            command.main(['--help'])
        self.assertEqual(raised.exception.code, 0)
        self.assertIn('--seed-fd', output.getvalue())
        self.assertNotIn('--trial-command', output.getvalue())

    def test_private_paths_require_actual_isolated_flags(self):
        with patch.object(command.sys, 'flags', types.SimpleNamespace(
                isolated=0, no_site=1, dont_write_bytecode=1)), self.assertRaises(command.ScalingCommandError):
            command._install_private_paths()

    def test_private_paths_only_entrypoint_relative_and_not_duplicate(self):
        flags = types.SimpleNamespace(isolated=1, no_site=1, dont_write_bytecode=1)
        original = command.__file__
        paths = [str(Path(original).parent), str(Path(original).parent.parent)]
        with patch.object(command.sys, 'flags', flags), patch.object(command.sys, 'path', ['stdlib']):
            command._install_private_paths()
            self.assertEqual(command.sys.path, paths + ['stdlib'])
            with self.assertRaises(command.ScalingCommandError):
                command._install_private_paths()


class CommandOwnershipTests(unittest.TestCase):
    """No success before cleanup, and no borrowed runtime closure during pending reap."""
    def run_command(self, *, criteria=(True, True, True), failure=None,
                    phase=None, pending=(), invalid_outcome=False, close_failure=None):
        events = []
        launch = types.SimpleNamespace(evidence_root=Path('/private/evidence'))
        polls = iter(pending)
        def event(name):
            events.append(name)
            if name == phase:
                raise ValueError('PRIVATE_SECRET_SHOULD_NOT_PRINT')
        class Dependencies:
            def close(self):
                event('dependencies.close')
                if close_failure == 'dependencies':
                    raise RuntimeError('PRIVATE_SECRET_SHOULD_NOT_PRINT')
        dependencies = Dependencies()
        class Invocation:
            @classmethod
            def admit(cls, got_launch, got_dependencies):
                event('admit')
                assert got_launch is launch and got_dependencies is dependencies
                return cls()
            def run(self, seed):
                event('run')
                assert seed == 'b' * 64
                if failure is not None:
                    raise failure
                if invalid_outcome:
                    return OutcomeFixture(Path('/private/evidence/report.json'), 'c' * 64,
                        'PRIVATE_SECRET_SHOULD_NOT_PRINT', *criteria)
                return OutcomeFixture(Path('/private/evidence/report.json'), 'c' * 64, 'd' * 64, *criteria)
            def poll_cleanup(self):
                event('poll')
                value = next(polls, True)
                if isinstance(value, BaseException):
                    raise value
                return value
            def close(self):
                event('invocation.close')
                if close_failure == 'invocation':
                    raise RuntimeError('PRIVATE_SECRET_SHOULD_NOT_PRINT')
        def read_launch(fd, digest):
            event('read_launch'); assert fd == 10 and digest == 'a' * 64
            return b'launch'
        def bootstrap(raw):
            event('bootstrap'); assert raw == b'launch'
            return dependencies
        def decode(raw):
            event('decode'); assert raw == b'launch'
            return launch
        def read_seed(fd):
            event('read_seed'); assert fd == 11
            return 'b' * 64
        definitions = {
            'scaling_experiment_cli_inputs': dict(read_launch_descriptor=read_launch,
                read_seed_descriptor=read_seed, decode_launch_input=decode),
            'scaling_cli_bootstrap': dict(bootstrap_runtime=bootstrap),
            'scaling_experiment_invocation': dict(InvocationResources=Invocation),
            'scaling_experiment_execution': dict(ExperimentCleanupRequired=CleanupFixture,
                ExperimentOutcome=OutcomeFixture),
        }
        modules = {}
        for name, attrs in definitions.items():
            module = types.ModuleType(name); module.__dict__.update(attrs); modules[name] = module
        output, error = io.StringIO(), io.StringIO()
        with patch.dict(sys.modules, modules), patch.object(command, '_install_private_paths', lambda: event('paths')):
            with patch.object(command.time, 'sleep', lambda _: event('sleep')):
                with redirect_stdout(output), redirect_stderr(error):
                    code = command.main(_ARGS)
        combined = output.getvalue() + error.getvalue()
        self.assertNotIn('PRIVATE_SECRET_SHOULD_NOT_PRINT', combined)
        self.assertNotIn('b' * 64, combined)
        return code, events, output.getvalue(), error.getvalue()

    def test_all_eight_measured_criterion_combinations(self):
        for criteria in itertools.product((False, True), repeat=3):
            code, events, output, _ = self.run_command(criteria=criteria)
            with self.subTest(criteria=criteria):
                self.assertEqual(code, 0 if all(criteria) else 1)
                self.assertEqual(events[:6], ['paths', 'read_launch', 'bootstrap', 'decode', 'admit', 'read_seed'])
                self.assertEqual(events[-3:], ['poll', 'invocation.close', 'dependencies.close'])
                self.assertIn(('PASS' if all(criteria) else 'FAIL') + ': report_sha256=' + 'd' * 64, output)

    def test_every_prelaunch_failure_closes_only_available_owners(self):
        for phase in ('paths', 'read_launch', 'bootstrap', 'decode', 'admit', 'read_seed', 'run'):
            code, events, output, _ = self.run_command(phase=phase)
            with self.subTest(phase=phase):
                self.assertEqual(code, 2)
                self.assertNotIn('PASS', output)
                if phase in ('decode', 'admit', 'read_seed', 'run'):
                    self.assertEqual(events[-1], 'dependencies.close')
                if phase in ('read_seed', 'run'):
                    self.assertIn('invocation.close', events)

    def test_pending_cleanup_waits_before_closing_inputs(self):
        code, events, output, error = self.run_command(failure=CleanupFixture('failure'),
            pending=(False, KeyboardInterrupt(), False, True))
        self.assertEqual(code, 2)
        self.assertEqual(events.count('sleep'), 3)
        self.assertLess(max(index for index, value in enumerate(events) if value == 'sleep'),
                        events.index('invocation.close'))
        self.assertEqual(events[-1], 'dependencies.close')
        self.assertNotIn('PASS', output)
        self.assertEqual(error.count('cleanup pending'), 1)

    def test_pending_keyboard_interrupt_retains_failure_status(self):
        code, events, output, _ = self.run_command(failure=CleanupFixture('keyboard_interrupt'), pending=(False, True))
        self.assertEqual(code, 130)
        self.assertLess(events.index('sleep'), events.index('dependencies.close'))
        self.assertNotIn('PASS', output)

    def test_cleanup_errors_and_invalid_report_identity_cannot_return_success(self):
        for options in ({'close_failure': 'invocation'}, {'close_failure': 'dependencies'}, {'invalid_outcome': True}):
            code, _, output, _ = self.run_command(**options)
            with self.subTest(options=options):
                self.assertEqual(code, 2)
                self.assertNotIn('PASS', output)

    def test_ordinary_interrupts_fail_after_closing_inputs(self):
        for error, expected in ((KeyboardInterrupt(), 130), (SystemExit(0), 2), (GeneratorExit(), 2)):
            code, events, output, _ = self.run_command(failure=error)
            with self.subTest(error=type(error)):
                self.assertEqual(code, expected)
                self.assertEqual(events[-1], 'dependencies.close')
                self.assertNotIn('PASS', output)


if __name__ == '__main__':
    unittest.main()
