"""Execute the fixed first-release scaling experiment through original owners.

The runtime admission, ten native trials, independent raw replay and final
report verification share one lifetime. The public report records observed
criteria; only this completed invocation can return a successful gate outcome.
"""
from dataclasses import dataclass
from pathlib import Path
import re
import time

from resource_experiment import ResourceExperiment
from resource_process import DarwinProcessReader
from scaling_experiment_custody import FixedExperimentCustody
from scaling_experiment_plan import RUN_KEYS


class ExperimentExecutionError(ValueError):
    """Closed execution failure; runtime credentials and child output stay private."""


@dataclass(frozen=True, slots=True)
class ExperimentOutcome:
    """Report identity and observed criteria returned by a completed invocation."""
    report_path: Path
    manifest_sha256: str
    report_sha256: str
    throughput_criterion_met: bool
    latency_criterion_met: bool
    observed_resource_criterion_met: bool


def _require(value):
    if not value:
        raise ExperimentExecutionError('fixed_scaling_execution_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt):
        raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit):
        raise SystemExit(1) from None
    if isinstance(error, GeneratorExit):
        raise GeneratorExit() from None
    raise ExperimentExecutionError('fixed_scaling_execution_failed') from None



class PendingExperimentCleanup:
    """Retain original owners until their child/descriptor cleanup completes.

    The caller must keep this handle and the live interpreter until poll_closed
    succeeds. It must not close the borrowed admission, images or worker source
    owner meanwhile. Evidence/runtime files remain available for diagnosis.
    """
    __slots__ = ('_retained', '_complete', '_busy')

    def __init__(self, *args, **kwargs):
        raise ExperimentExecutionError('fixed_scaling_execution_failed')

    def owns_admission(self, admission) -> bool:
        """Identify the exact borrowed runtime whose lifetime remains retained."""
        return admission is self._retained[2]

    def poll_closed(self) -> bool:
        """Poll original child handles and close only after every child is reaped.

        No signal, blocking wait, new deadline or runtime verification occurs.
        Failure, including interruption, leaves every original owner retained.
        The original experiment is permanently failed and cannot publish again.
        """
        if self._complete:
            return True
        _require(not self._busy)
        self._busy = True
        try:
            self._retained[0].close()
            self._complete = True
            return True
        except BaseException:
            return False
        finally:
            self._busy = False


class ExperimentCleanupRequired(ExperimentExecutionError):
    """Closed failure with retained runtime-only cleanup authority."""
    def __init__(self, cleanup: PendingExperimentCleanup, interruption: str):
        _require(type(cleanup) is PendingExperimentCleanup and interruption in
                 ('failure', 'keyboard_interrupt', 'system_exit', 'generator_exit'))
        super().__init__('fixed_scaling_cleanup_required')
        self.cleanup = cleanup
        self.interruption = interruption


def _failed_execution(error, owner, borrower, admission, stop_timeout_ns):
    if owner is None:
        _failure(error)
    cleanup = object.__new__(PendingExperimentCleanup)
    cleanup._retained = (owner, borrower, admission)
    cleanup._complete = cleanup._busy = False
    # One bounded reap attempt uses the stop policy captured before any trial.
    # Subsequent resolution polls original handles only; no deadline is renewed.
    try:
        active = owner._active_pin
        if active is not None and active[2] is not None:
            active[2].cleanup(time.monotonic_ns() + stop_timeout_ns)
    except BaseException as cleanup_error:
        if isinstance(error, Exception) and not isinstance(cleanup_error, Exception):
            error = cleanup_error
    if cleanup.poll_closed():
        _failure(error)
    interruption = ('keyboard_interrupt' if isinstance(error, KeyboardInterrupt)
                    else 'system_exit' if isinstance(error, SystemExit)
                    else 'generator_exit' if isinstance(error, GeneratorExit)
                    else 'failure')
    raise ExperimentCleanupRequired(cleanup, interruption) from None

def execute_experiment(admission, evidence_root: Path, runtime_root: Path,
                       budget, development_seed: str) -> ExperimentOutcome:
    """Run five fixed pairs once, retaining provenance through report readback.

    Admission is an actual release-runtime owner. Private seed material is used
    only by the native generator and is never written into public controls.
    The caller retains admission until this method returns or finishes cleanup.
    """
    from scaling_runtime_admission import RuntimeAdmission
    owner = borrower = None
    stop_timeout_ns = None
    try:
        _require(type(admission) is RuntimeAdmission
                 and type(development_seed) is str
                 and re.fullmatch('[a-f0-9]{64}', development_seed))
        admission.verify()
        plan = admission.plan
        stop_timeout_ns = plan.trials[0].stop_timeout_ns
        owner = FixedExperimentCustody(evidence_root, runtime_root, plan, budget,
            admission.runtime, DarwinProcessReader(), admission.validate,
            admission.identity, admission.worker_sources)
        for pair, variant in RUN_KEYS:
            admission.verify()
            slot = owner.begin_run(pair, variant)
            trial = slot.create_trial()
            trial.run(development_seed)
            handle = trial.handoff(slot)
            owner.publish_run(handle)
        del development_seed
        admission.verify()
        manifest = owner.publish_manifest()
        borrower = ResourceExperiment.from_completed(owner)
        borrower.collect_replay()
        report = owner.publish_report(borrower)
        # Full retained source verification happens before the owner's final
        # complete semantic/metadata/deadline fence. No native job follows it.
        admission.verify()
        _require(owner.verify_report() is report)
        metrics = owner._measurements
        outcome = ExperimentOutcome(evidence_root / 'report.json', manifest.sha256,
            report.sha256, metrics.throughput_criterion_met, metrics.latency_criterion_met,
            metrics.observed_resource_criterion_met)
        borrower.close()
        owner.close()
        return outcome
    except BaseException as error:
        _failed_execution(error, owner, borrower, admission, stop_timeout_ns)
