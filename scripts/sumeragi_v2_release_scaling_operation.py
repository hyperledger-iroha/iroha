"""The protected parent's fixed collector and complete archive verification owner.

No operation, command, path, result or callback is decoded from the handoff.
Bootstrap supplies its own exact classes and bounded natural-reap command API.
All checks consume the unchanged original parent deadline. Returned observations
are data for the final authenticated parent receipt, never standalone authority.
"""
from __future__ import annotations

from dataclasses import dataclass, fields
import hashlib
import json
from pathlib import Path
import re
import secrets
import time

from scaling_release_provisioning import (
    PreparedScalingInputs, PreparedScalingLaunch, PreparedScalingVerification,
    ScalingInputsBorrowedError,
)

NS = 1_000_000_000


class ScalingOperationError(ValueError):
    """Closed parent operation failure without input contents or native stderr."""


def _require(value):
    if not value:
        raise ScalingOperationError('fixed_scaling_operation_failed')


def _digest(value):
    _require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None)
    return value


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _environment(value):
    _require(type(value) is dict and all(type(key) is type(item) is str
                                       for key, item in value.items()))
    return _sha((json.dumps(value, sort_keys=True, separators=(',', ':')) + '\n').encode('ascii'))


@dataclass(frozen=True, slots=True)
class BootstrapScalingApi:
    """Trusted direct bootstrap bindings; never serialize or decode this record.

    Bootstrap constructs this with its existing named classes/functions. The
    callback is its fixed natural-reap command implementation, not a success or
    command-selection callback. This record avoids importing the bootstrap main.
    """
    launch_type: type
    command_owner_type: type
    terminal_type: type
    publication_type: type
    artifact_type: type
    result_type: type
    run_bounded: object
    validated_pass_fds: object


@dataclass(frozen=True, slots=True)
class NativeArchiveReplayObservation:
    """Fresh native terminal and complete row join for one archived run."""
    pair_index: int
    variant: str
    invocation_id: str
    request_sha256: str
    proof_sha256: str
    proof_iroha_hash: str
    proof_bytes: int
    row_count: int
    reply_bytes: int
    reply_sha256: str
    joined_rows_sha256: str
    command: object


@dataclass(frozen=True, slots=True)
class ScalingPublicationVerification:
    """Completed parent observations awaiting the final parent authentication.

    The collector and verification subprocesses remain separate observations.
    This record does not deserialize a live execution or qualify a release.
    """
    invocation_sha256: str
    launch_input_sha256: str
    plan_sha256: str
    budget_sha256: str
    kagami_sha256: str
    collector: object
    artifacts: tuple
    inventory: object
    archive_checks: object
    native_replays: tuple[NativeArchiveReplayObservation, ...]
    original_deadline_ns: int
    verification_completed_ns: int


def _api_pin(api):
    _require(type(api) is BootstrapScalingApi)
    names = ('FixedScalingLaunch', 'CommandObservationOwner', 'TerminalCommandObservation',
             'ParentScalingObservation', 'ScalingArtifactObservation', 'CommandResult')
    values = tuple(getattr(api, field.name) for field in fields(BootstrapScalingApi))
    _require(all(isinstance(value, type) and value.__name__ == name
                 for value, name in zip(values[:6], names, strict=True)))
    _require(callable(values[6]) and getattr(values[6], '__name__', None) == '_run_bounded'
             and callable(values[7]) and getattr(values[7], '__name__', None) == '_validated_pass_fds')
    return values


def _launch_pin(launch, expected_type):
    _require(type(launch) is expected_type)
    _require(tuple(field.name for field in fields(expected_type))
             == tuple(field.name for field in fields(PreparedScalingLaunch)))
    values = []
    for field in fields(PreparedScalingLaunch):
        value = getattr(launch, field.name)
        if field.name == 'environment':
            _environment(value)
            value = tuple(sorted(value.items()))
        values.append(value)
    return tuple(values)


class FixedScalingOperation:
    """One collector handoff, ten fixed native replays and exact data checks."""
    def __init__(self, prepared: PreparedScalingInputs, invocation_sha256: str,
                 api: BootstrapScalingApi):
        _require(type(prepared) is PreparedScalingInputs)
        self._api, self._api_original = api, api
        self._api_values = _api_pin(api)
        self._prepared = self._prepared_original = prepared
        self._invocation = _digest(invocation_sha256)
        self._invocation_pin = self._invocation
        self._phase, self._busy, self._failed = 'new', False, False
        self._launch = self._launch_original = None
        self._collector = None
        self._native_owners = []
        self._verification = None
        self._inputs = prepared.verification_inputs()
        _require(type(self._inputs) is PreparedScalingVerification)
        self._input_pin = tuple(getattr(self._inputs, field.name)
                                for field in fields(PreparedScalingVerification))
        # Admission has loaded the authenticated dependency closure already.
        from scaling_experiment_config import decode_fixed_inputs
        self._plan, self._budget, _ = decode_fixed_inputs(
            self._inputs.plan_bytes, self._inputs.budget_bytes)

    def _identity(self):
        _require(self._api is self._api_original and self._prepared is self._prepared_original
                 and self._invocation == self._invocation_pin)
        _require(all(left is right for left, right in
                     zip(_api_pin(self._api), self._api_values, strict=True)))
        current = self._prepared.verification_inputs()
        _require(tuple(getattr(current, field.name) for field in fields(PreparedScalingVerification))
                 == self._input_pin)
        _require(type(self._inputs) is PreparedScalingVerification and self._inputs == current)
        from scaling_experiment_config import decode_fixed_inputs
        plan, budget, _ = decode_fixed_inputs(current.plan_bytes, current.budget_bytes)
        _require(self._plan == plan and self._budget == budget)

    def _check_launch(self, launch):
        _require(launch is self._launch_original is self._launch)
        _require(_launch_pin(launch, self._api.launch_type) == self._launch_values)

    def _deadline(self):
        _require(type(self._launch.deadline_ns) is int
                 and time.monotonic_ns() < self._launch.deadline_ns)

    def prepare(self):
        """Borrow the original descriptors once and construct the exact bootstrap row."""
        _require(not self._failed and not self._busy and self._phase == 'new')
        self._phase = 'preparing'
        try:
            self._identity()
            prepared = self._prepared.claim_launch()
        except BaseException:
            self._failed = True
            raise
        # No operation after claim may lose knowledge of the outstanding borrow.
        self._phase = 'borrowed'
        try:
            _require(type(prepared) is PreparedScalingLaunch)
            values = {field.name: getattr(prepared, field.name) for field in fields(PreparedScalingLaunch)}
            values['environment'] = dict(prepared.environment)
            self._launch = self._launch_original = self._api.launch_type(**values)
            self._launch_values = _launch_pin(self._launch, self._api.launch_type)
            self._descriptors = self._api.validated_pass_fds((prepared.launch_input_fd, prepared.seed_fd))
            self.validate(self._launch)
            return self._launch
        except BaseException:
            # Popen has not been called; this operation owns the exact no-spawn fact.
            self._prepared.declare_child_reaped()
            self._phase, self._failed = 'reaped', True
            raise

    def validate(self, launch):
        """Recheck retained selections and the unchanged original deadline."""
        _require(not self._failed and not self._busy
                 and self._phase in ('borrowed', 'reaped', 'verifying', 'verified'))
        self._busy = True
        try:
            self._check_launch(launch)
            self._identity()
            self._deadline()
            if self._verification is not None:
                from scaling_archive_data import inventory_archive
                _require(inventory_archive(launch.evidence_root, self._plan, self._budget)
                         == self._verification.inventory)
                self._deadline()
        except BaseException:
            self._failed = True
            raise
        finally:
            self._busy = False

    def _terminal(self, terminal, argv, cwd, environment, descriptors):
        _require(type(terminal) is self._api.terminal_type
                 and terminal.argv == argv and terminal.cwd == str(cwd)
                 and terminal.environment_sha256 == _environment(environment)
                 and terminal.descriptors == descriptors
                 and type(terminal.pid) is int and terminal.pid > 0
                 and type(terminal.started_ns) is int
                 and type(terminal.completed_ns) is int
                 and self._launch.original_started_ns <= terminal.started_ns
                 <= terminal.completed_ns <= terminal.deadline_ns == self._launch.deadline_ns
                 and type(terminal.returncode) is int and terminal.returncode == 0
                 and type(terminal.violations) is tuple and terminal.violations == ())
        for name in ('stdout_bytes', 'stderr_bytes'):
            _require(type(getattr(terminal, name)) is int and getattr(terminal, name) >= 0)
        for name in ('stdout_sha256', 'stderr_sha256'): _digest(getattr(terminal, name))

    def child_finished(self, launch, observation):
        """Accept the handoff's original natural-reap/no-spawn notification once.

        Even a failing or late child has been reaped; release its borrow before
        rejecting later publication. No recorded archive field calls this method.
        """
        _require(not self._busy and self._phase == 'borrowed')
        _require(launch is self._launch_original is self._launch)
        _require(observation is None or type(observation) is self._api.terminal_type)
        self._prepared.declare_child_reaped()
        self._phase, self._collector = 'reaped', observation

    def _replay(self, run):
        from scaling_canonical_proof import (
            AppliedObservation, ReplayPlan, ReplayBindings, _plan_snapshot,
            _bindings_snapshot, _ProjectionJoiner, MAX_CHUNK_BYTES,
        )
        from scaling_measurements import _schedule
        from scaling_experiment_plan import encode
        self.validate(self._launch)
        data = run.measurement
        _, load, accounts, counts, _ = _schedule(data)
        plan = ReplayPlan(load.seed, data.plan.generator.lane_count, accounts,
            counts[0], counts[1], max(row.block_height for row in data.requests),
            tuple(AppliedObservation(row.transaction_hash, row.block_height, row.local_block_height)
                  for row in data.requests))
        files = {item.role: item for item in run.files}
        request, proof = files['native_request'], files['canonical_proof']
        recorded = json.loads(run.canonical_receipt_json)
        _require(encode(recorded, len(run.canonical_receipt_json)) == run.canonical_receipt_json)
        _require(recorded['verifier_sha256'] == self._inputs.kagami_sha256)
        bindings = ReplayBindings(self._launch.evidence_root / request.binding.path,
            request.binding.sha256, request.max_bytes,
            self._launch.evidence_root / proof.binding.path, proof.binding.sha256,
            recorded['proof_iroha_hash'], proof.bytes, proof.max_bytes,
            data.plan.replay_reply_max_bytes)
        values = _bindings_snapshot(bindings)
        invocation = secrets.token_hex(32)
        _require(_digest(invocation) != '0' * 64)
        joiner = _ProjectionJoiner(_plan_snapshot(plan), values, invocation)
        request_path, request_sha, request_cap, proof_path, proof_sha, proof_hash, size, proof_cap, reply_cap = values
        argv = (str(self._inputs.kagami), '--ui-mode', 'plain', 'advanced', 'kura',
            'scaling-evidence', 'replay', '--invocation-id', invocation,
            '--request', request_path, '--request-sha256', request_sha,
            '--request-max-bytes', str(request_cap), '--input', proof_path,
            '--input-sha256', proof_sha, '--input-max-bytes', str(proof_cap),
            '--reply-max-bytes', str(reply_cap), '--proof-iroha-hash', proof_hash)
        owner = self._api.command_owner_type()
        self._native_owners.append(owner)
        result = self._api.run_bounded(self._inputs.kagami, argv[1:], cwd=self._launch.cwd,
            environment=dict(self._launch.environment), timeout_seconds=self._launch.timeout_seconds,
            maximum_output_bytes=reply_cap, pass_fds=(), observation=owner,
            deadline_ns=self._launch.deadline_ns)
        _require(type(owner) is self._api.command_owner_type and owner._process is not None
                 and owner._reaped and owner.terminal is not None
                 and type(result) is self._api.result_type
                 and type(result.returncode) is int and result.returncode == 0
                 and type(result.stdout) is bytes and type(result.stderr) is bytes)
        terminal = owner.terminal
        self._terminal(terminal, argv, self._launch.cwd, self._launch.environment, ())
        _require(terminal.stdout_bytes == len(result.stdout)
                 and terminal.stderr_bytes == len(result.stderr)
                 and terminal.stdout_sha256 == _sha(result.stdout)
                 and terminal.stderr_sha256 == _sha(result.stderr)
                 and len(result.stdout) + len(result.stderr) <= reply_cap)
        self.validate(self._launch)
        for offset in range(0, len(result.stdout), MAX_CHUNK_BYTES):
            joiner.consume(result.stdout[offset:offset + MAX_CHUNK_BYTES])
        count, reply_size, reply_sha, joined = joiner.finish()
        _require((reply_size, reply_sha) == (terminal.stdout_bytes, terminal.stdout_sha256))
        self._deadline()
        return NativeArchiveReplayObservation(run.pair_index, run.variant, invocation,
            request_sha, proof_sha, proof_hash, size, count, reply_size, reply_sha, joined, terminal)

    def verify_publication(self, launch, observation):
        """Inspect all archive bytes and join ten real fixed native replay results."""
        _require(not self._failed and not self._busy and self._phase == 'reaped')
        self.validate(launch)
        self._phase = 'verifying'
        try:
            _require(type(observation) is self._api.publication_type
                     and observation.invocation_sha256 == self._invocation
                     and observation.command is self._collector
                     and observation.launch_input_sha256 == launch.launch_input_sha256
                     and observation.original_started_ns == launch.original_started_ns)
            self._terminal(self._collector, launch.argv, launch.cwd,
                           launch.environment, self._descriptors)
            _require(self._collector.stdout_bytes + self._collector.stderr_bytes
                     <= launch.maximum_output_bytes)
            artifacts = observation.artifacts
            _require(type(artifacts) is tuple and len(artifacts) == 2)
            for artifact, path, maximum in zip(artifacts, ('manifest.json', 'report.json'),
                    (launch.manifest_max_bytes, launch.report_max_bytes), strict=True):
                _require(type(artifact) is self._api.artifact_type
                         and artifact.relative_path == path and type(artifact.size_bytes) is int
                         and 0 < artifact.size_bytes <= maximum and type(artifact.mode) is int
                         and artifact.mode in (0o400, 0o600))
                _digest(artifact.sha256)
            from scaling_archive_data import ArchiveBindings, inventory_archive, inspect_archive
            from scaling_experiment_plan import RUN_KEYS
            # This first portable inventory is observed by the original parent
            # after actual collector termination; final receipt authentication is
            # still required. It is never supplied as archive-origin provenance.
            inventory = inventory_archive(launch.evidence_root, self._plan, self._budget)
            expected = ArchiveBindings(artifacts[0].sha256, artifacts[1].sha256, inventory.sha256)
            checked = inspect_archive(launch.evidence_root, self._plan, self._budget, expected)
            _require(checked.inventory == inventory and tuple((run.pair_index, run.variant)
                         for run in checked.native_runs) == RUN_KEYS)
            self._join_source_inputs(checked)
            measurements = checked.measurements
            _require(measurements.throughput_criterion_met is True
                     and measurements.latency_criterion_met is True
                     and measurements.observed_resource_criterion_met is True)
            replays = tuple(self._replay(run) for run in checked.native_runs)
            _require(len({row.invocation_id for row in replays}) == 10)
            final = inspect_archive(launch.evidence_root, self._plan, self._budget, expected)
            _require(final == checked)
            self.validate(launch)
            self._verification = ScalingPublicationVerification(self._invocation,
                launch.launch_input_sha256, _sha(self._inputs.plan_bytes), _sha(self._inputs.budget_bytes),
                self._inputs.kagami_sha256, self._collector, artifacts, inventory, checked, replays,
                launch.deadline_ns, time.monotonic_ns())
            self._deadline()
            self._phase = 'verified'
        except BaseException:
            self._failed = True
            raise

    def _join_source_inputs(self, checked):
        """Join archive data to original retained source/image/worker selections."""
        identity = json.loads(checked.identity_json)
        source = json.loads(checked.source_closure_json)
        expected = self._inputs
        for value in (identity['software'], source):
            _require(value['source_revision'] == expected.source_revision
                     and value['workspace_source_sha256'] == expected.workspace_source_sha256)
        images = dict(expected.executable_images)
        _require(source['executable_images'] == images
                 and identity['software']['irohad_sha256'] == images['daemon']
                 and identity['software']['iroha_cli_sha256'] == images['cli']
                 and identity['hardware']['machine_id'] == expected.machine_id
                 and identity['hardware']['storage_model'] == expected.storage_model)
        workers = tuple((row['name'], row['bytes'], row['sha256']) for row in source['resource_sources'])
        _require(workers == expected.worker_sources)

    @property
    def verification(self) -> ScalingPublicationVerification:
        """Read completed data for the final original-parent receipt join."""
        _require(not self._failed and self._phase in ('verified', 'closed')
                 and type(self._verification) is ScalingPublicationVerification)
        return self._verification

    def close(self):
        """Release no descriptor while its original collector/verifier is pending."""
        _require(not self._busy)
        if self._phase == 'closed': return
        if self._phase == 'borrowed' or any(owner._process is not None
                and (not owner._reaped or owner.terminal is None) for owner in self._native_owners):
            raise ScalingInputsBorrowedError('scaling_inputs_original_child_not_reaped')
        self._prepared.close()
        self._phase = 'closed'
