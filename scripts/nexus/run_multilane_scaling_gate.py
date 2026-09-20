#!/usr/bin/env python3
"""Run the fixed first-release five-pair multilane scaling experiment.

The release bootstrap launches this exact source under its authenticated
Python runtime with -I -B -S and two inherited inputs. There is one native
pipeline and one originating experiment owner, with no external trial command.
"""
import argparse
import os
from pathlib import Path
import re
import sys
import time


class ScalingCommandError(ValueError):
    """Closed command failure with no launch contents or secret material."""


def _require(value):
    if not value:
        raise ScalingCommandError('fixed_scaling_command_failed')


class _Parser(argparse.ArgumentParser):
    """Keep rejected arguments out of error output."""
    def error(self, _message):
        self.exit(2, '[g-scale] FAIL: invalid fixed launch arguments\n')


class _UniqueInput(argparse.Action):
    """Reject repeated authority inputs instead of selecting the final value."""
    def __call__(self, parser, namespace, values, option_string=None):
        if getattr(namespace, self.dest, None) is not None:
            parser.error('duplicate fixed input')
        setattr(namespace, self.dest, values)


def _descriptor(value):
    if not re.fullmatch('[0-9]{1,7}', value):
        raise argparse.ArgumentTypeError('invalid descriptor')
    number = int(value)
    if not 2 < number < 1 << 20:
        raise argparse.ArgumentTypeError('invalid descriptor')
    return number


def _digest(value):
    if not re.fullmatch('[a-f0-9]{64}', value):
        raise argparse.ArgumentTypeError('invalid digest')
    return value


def parse_args(argv=None):
    """Accept only original launch/seed descriptors and the release-supplied hash."""
    parser = _Parser(description=__doc__, allow_abbrev=False)
    parser.add_argument('--launch-input-fd', required=True, type=_descriptor, action=_UniqueInput,
        help='Inherited read-only fixed launch input descriptor.')
    parser.add_argument('--launch-input-sha256', required=True, type=_digest, action=_UniqueInput,
        help='Release bootstrap SHA-256 of the exact launch input.')
    parser.add_argument('--seed-fd', required=True, type=_descriptor, action=_UniqueInput,
        help='Inherited ready nonblocking seed pipe; seed bytes never enter argv.')
    args = parser.parse_args(argv)
    if args.launch_input_fd == args.seed_fd:
        parser.error('inputs must have distinct original descriptors')
    return args


def _install_private_paths():
    """Enable only this authenticated entrypoint's two repository source roots."""
    _require(sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode)
    original = Path(os.path.abspath(__file__))
    _require(original.resolve(strict=True) == original
             and original.name == 'run_multilane_scaling_gate.py'
             and original.parent.name == 'nexus' and original.parent.parent.name == 'scripts')
    paths = (str(original.parent), str(original.parent.parent))
    _require(all(path not in sys.path for path in paths))
    sys.path[:0] = paths


def _await_cleanup(invocation):
    """Keep the original runtime alive until failed children have been reaped."""
    announced = False
    while True:
        try:
            complete = invocation.poll_cleanup()
            if type(complete) is bool and complete:
                return
        except BaseException:
            # Failure to observe completion never releases borrowed runtime
            # inputs. The originating cleanup owner remains retained here.
            pass
        if not announced:
            print('[g-scale] FAIL: cleanup pending; retaining runtime until original children exit',
                  file=sys.stderr, flush=True)
            announced = True
        try:
            time.sleep(1)
        except BaseException:
            # An interrupt cannot turn an unreaped child into completed cleanup.
            continue


def main(argv=None) -> int:
    """Run and verify actual evidence; success requires all three measured criteria."""
    args = parse_args(argv)
    dependencies = invocation = outcome = None
    exit_code = 2
    try:
        _install_private_paths()
        from scaling_experiment_cli_inputs import (
            read_launch_descriptor, read_seed_descriptor, decode_launch_input,
        )
        raw = read_launch_descriptor(args.launch_input_fd, args.launch_input_sha256)
        from scaling_cli_bootstrap import bootstrap_runtime
        dependencies = bootstrap_runtime(raw)
        launch = decode_launch_input(raw)
        from scaling_experiment_invocation import InvocationResources
        from scaling_experiment_execution import ExperimentCleanupRequired, ExperimentOutcome
        invocation = InvocationResources.admit(launch, dependencies)
        seed = read_seed_descriptor(args.seed_fd)
        try:
            outcome = invocation.run(seed)
        except ExperimentCleanupRequired as error:
            _await_cleanup(invocation)
            exit_code = 130 if error.interruption == 'keyboard_interrupt' else 2
        finally:
            del seed
        if outcome is not None:
            _require(type(outcome) is ExperimentOutcome
                     and type(outcome.report_sha256) is str
                     and re.fullmatch('[a-f0-9]{64}', outcome.report_sha256)
                     and type(outcome.manifest_sha256) is str
                     and re.fullmatch('[a-f0-9]{64}', outcome.manifest_sha256)
                     and type(outcome.report_path) is type(Path('/'))
                     and outcome.report_path == launch.evidence_root / 'report.json'
                     and all(type(value) is bool for value in (
                         outcome.throughput_criterion_met, outcome.latency_criterion_met,
                         outcome.observed_resource_criterion_met)))
            exit_code = 0 if (outcome.throughput_criterion_met and outcome.latency_criterion_met
                             and outcome.observed_resource_criterion_met) else 1
    except KeyboardInterrupt:
        exit_code = 130
    except (Exception, SystemExit, GeneratorExit):
        exit_code = 2
    finally:
        if invocation is not None:
            _await_cleanup(invocation)
            try:
                invocation.close()
            except BaseException:
                exit_code = 2
        if dependencies is not None:
            try:
                dependencies.close()
            except BaseException:
                exit_code = 2
    if outcome is not None and exit_code in (0, 1):
        print('[g-scale] ' + ('PASS' if exit_code == 0 else 'FAIL')
              + ': report_sha256=' + outcome.report_sha256, flush=True)
    else:
        print('[g-scale] FAIL: fixed experiment did not complete', file=sys.stderr, flush=True)
    return exit_code


if __name__ == '__main__':
    raise SystemExit(main())
