"""Compose original release inputs into the fixed scaling execution service.

Configuration is decoded before any native operation. This owner retains every
borrowed image, worker pack and runtime admission until the original experiment
has reaped its children, including a failed or interrupted experiment.
"""
from dataclasses import fields
from pathlib import Path

import sumeragi_v2_prebuilt_bundle as binary_contract
import write_sumeragi_v2_release_receipt as receipt_contract
from resource_process import ExecutableImage
from scaling_cli_bootstrap import PythonDependencies
from scaling_experiment_cli_inputs import LaunchInputs
from scaling_experiment_config import decode_fixed_inputs, MAX_CONFIG_BYTES
from scaling_experiment_execution import (
    execute_experiment, ExperimentCleanupRequired, PendingExperimentCleanup,
)
from scaling_experiment_inputs import ExperimentIdentity
from scaling_fixed_trial import TrialRuntime
from scaling_runtime_admission import (
    RuntimeAdmission, observe_host, _RetainedFiles, _python_manifest, _MAX_CONTROL_BYTES,
)
from scaling_worker_sources import WorkerSourceFiles, WorkerSourcePin, SOURCE_NAMES


class InvocationError(ValueError):
    """Closed invocation failure without credentials or private child output."""


def _require(value):
    if not value:
        raise InvocationError('fixed_scaling_invocation_failed')


def _separate(left, right):
    return left != right and left not in right.parents and right not in left.parents


def _output_scope(launch):
    """Keep fresh trial output outside every source, build and runtime input tree."""
    paths, dependencies = launch.runtime_paths, launch.python_dependencies
    _require(_separate(launch.evidence_root, launch.runtime_root))
    protected = (paths.source_root, paths.cargo_target_root, paths.binary_bundle,
        paths.python_sources, paths.python_evidence, launch.worker_sources,
        dependencies.source_root, dependencies.bundle_root)
    controls = (launch.plan_path, launch.budget_path, paths.source_paths,
        paths.rustc_version, paths.python_source_manifest, paths.python_runtime_binding,
        dependencies.inventory)
    for output in (launch.evidence_root, launch.runtime_root):
        _require(all(_separate(output, path) for path in protected)
                 and all(output != path and output not in path.parents for path in controls))
        # The directory owner creates these exact leaves with no replacement.
        # Source/path relationships are checked now, before creating any output.
        _require(not output.exists() and not output.is_symlink())


class InvocationResources:
    """Original input owners for exactly one fixed experiment execution."""
    def __init__(self, *args, **kwargs):
        raise InvocationError('fixed_scaling_invocation_failed')

    @classmethod
    def admit(cls, launch: LaunchInputs, dependencies: PythonDependencies):
        """Derive identities from authenticated manifests and observed host facts."""
        _require(cls is InvocationResources and type(launch) is LaunchInputs
                 and type(dependencies) is PythonDependencies)
        result = object.__new__(cls)
        result._files = _RetainedFiles()
        result._images = []
        result._workers = result._admission = result._pending = None
        result._dependencies = dependencies
        result._outputs = (launch.evidence_root, launch.runtime_root)
        result._used = result._closed = False
        try:
            _require(dependencies.paths == launch.python_dependencies)
            dependencies.verify()
            _output_scope(launch)
            for output in result._outputs:
                # Retain every original output-parent edge with O_NOFOLLOW
                # before any image owner or execution service is admitted.
                result._files.directory(output.parent)
            paths = launch.runtime_paths
            plan_raw = result._files.read(launch.plan_path, MAX_CONFIG_BYTES, launch.plan_sha256)
            budget_raw = result._files.read(launch.budget_path, MAX_CONFIG_BYTES, launch.budget_sha256)
            plan, result._budget, _ = decode_fixed_inputs(plan_raw, budget_raw)
            manifest_raw = result._files.read(paths.binary_bundle / binary_contract._MANIFEST_NAME,
                binary_contract._MAX_MANIFEST_BYTES, paths.binary_manifest_sha256)
            manifest = binary_contract._parse_manifest(manifest_raw)
            rustc = result._files.read(paths.rustc_version, binary_contract._MAX_TOOL_VERSION_BYTES,
                manifest['rustc_version_sha256'])
            _require(rustc and rustc.endswith(b'\n') and b'\0' not in rustc and b'\r' not in rustc)
            python_raw = result._files.read(paths.python_source_manifest, _MAX_CONTROL_BYTES,
                paths.python_source_manifest_sha256)
            python_sources = _python_manifest(python_raw)
            by_name = {row['path']: row for row in python_sources['files']}
            worker_pins = []
            for name in SOURCE_NAMES:
                row = by_name['scripts/nexus/' + name]
                worker_pins.append(WorkerSourcePin(name, row['sha256'], row['size']))
            result._workers = WorkerSourceFiles(launch.worker_sources, tuple(worker_pins))
            framework_raw = result._files.read(paths.python_runtime_binding, _MAX_CONTROL_BYTES,
                paths.python_runtime_binding_sha256)
            framework = receipt_contract._decode_canonical_json(framework_raw, 'scaling Python runtime')
            # The existing complete framework verifier owns this schema and
            # authenticates all entries before an executable owner is minted.
            receipt_contract._validate_framework_python_runtime(framework, paths.python_evidence)
            entries = receipt_contract._framework_runtime_projection(framework['records'], 'scaling Python runtime')
            python_image = [row for row in entries if row['path'] == 'bin/python3']
            _require(len(python_image) == 1 and python_image[0]['kind'] == 'file')
            for role in ('kagami', 'iroha', 'irohad'):
                result._images.append(ExecutableImage(paths.binary_bundle / manifest[role + '_relative_path'],
                    manifest[role + '_sha256']))
            result._images.append(ExecutableImage(paths.python_evidence / 'python-runtime/bin/python3',
                python_image[0]['sha256']))
            runtime = TrialRuntime(*result._images, result._workers.worker_path,
                                   result._workers.pins[0].sha256)
            host = observe_host()
            observed = {field.name: getattr(host, field.name)
                        for field in fields(host) if field.name != 'node_name'}
            identity = ExperimentIdentity(machine_id=launch.machine_id,
                storage_model=launch.storage_model, source_revision=launch.source_revision,
                rustc_version=rustc.splitlines()[0].decode('ascii'),
                workspace_source_sha256=manifest['source_manifest_sha256'], **observed)
            result._admission = RuntimeAdmission.admit(paths, identity, runtime,
                result._workers, plan, dependencies)
            result._files.validate()
            return result
        except BaseException:
            result.close()
            raise

    def run(self, development_seed: str):
        """Execute once; preserve original inputs when failed cleanup is pending."""
        _require(not self._closed and not self._used and self._admission is not None)
        self._used = True
        self._files.validate()
        _require(all(not output.exists() and not output.is_symlink()
                     for output in self._outputs))
        try:
            return execute_experiment(self._admission, *self._outputs,
                self._budget, development_seed)
        except ExperimentCleanupRequired as error:
            _require(type(error.cleanup) is PendingExperimentCleanup
                     and error.cleanup.owns_admission(self._admission))
            self._pending = error.cleanup
            raise

    def poll_cleanup(self) -> bool:
        """Poll only the original failed owner; keep all runtime inputs while false."""
        if self._pending is None:
            return True
        _require(type(self._pending) is PendingExperimentCleanup
                 and self._pending.owns_admission(self._admission))
        if not self._pending.poll_closed():
            return False
        self._pending = None
        return True

    def close(self):
        """Close borrowed runtime inputs only after original child cleanup succeeds."""
        if self._closed:
            return
        _require(self.poll_cleanup())
        errors = []
        owners = [self._admission, self._workers, *reversed(self._images), self._files]
        for owner in owners:
            if owner is not None:
                try:
                    owner.close()
                except BaseException as error:
                    errors.append(error)
        if errors:
            raise InvocationError('fixed_scaling_invocation_failed') from None
        self._closed = True
        # The bootstrap owns the dependency module and closes it last, after
        # this invocation returns. No module is unloaded during pending reap.
