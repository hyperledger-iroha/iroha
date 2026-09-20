"""Original-parent ownership of the complete fixed scaling preflight.

This module schedules only the source-bound canonical cases. The protected
bootstrap supplies its existing process owner; archive reports never replace
original child observations. This is a test prerequisite, not native scaling
or release qualification.
"""
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
import time

from scaling_preflight_archive import (
    ARCHIVE_ID, DATA_FIXTURES, INVENTORY_PATH, MAX_COMMAND_BYTES, MAX_INDEX_BYTES,
    MAX_INVENTORY_BYTES, MAX_OUTPUT_BYTES, MAX_RESULT_BYTES, MAX_SOURCE_BYTES,
    MAX_SOURCE_TOTAL_BYTES, NATIVE_FIXTURES, PHASE_COUNTS, PHASE_DRIVER,
    PYTEST_DRIVER, REQUIRED_MIGRATION_OUTCOMES, REQUIRED_PYTEST_SUITES,
    ScalingPreflightError, archive_census, bounded_json, canonical,
    decode_inventory, encode_index, member_caps, ordered_units, phase_inputs,
    project_unit_command, require, unit_argv, validate_binding,
    validate_command_inputs, validate_unit_result,
)


@dataclass(frozen=True)
class PreflightUnitObservation:
    """Exact original command and its source-bound required result."""
    name: str
    terminal: object
    result_sha256: str
    node_ids: tuple[str, ...]


@dataclass(frozen=True)
class PreflightApi:
    """Trusted integration callbacks supplied by the protected bootstrap."""
    command_owner: object
    run_bounded: object
    read_file: object
    unchanged: object


class CompleteScalingPreflight:
    """Retain all original phase commands until natural terminal observation.

    The owner is allocated before .run(). A failed wait/drain never means that
    Popen did not return a child and never releases the original source owner.
    """
    def __init__(self, repository_root, dependency_root, work_root, python,
                 environment, timeout_seconds, api, dependency_api, *, archive,
                 candidate_identity, invocation_sha256):
        require(type(api) is PreflightApi)
        require(type(timeout_seconds) is int and 600 <= timeout_seconds <= 86400)
        for path in (repository_root, dependency_root, work_root, python):
            require(type(path) is type(Path('/')) and path.is_absolute()
                and path == path.resolve())
        require(not work_root.exists() and repository_root not in work_root.parents
            and dependency_root not in work_root.parents)
        self._api, self._root, self._dependency = api, repository_root, dependency_root
        self._dependency_api = dependency_api
        self._blake3 = self._pytest = None
        self._work, self._python = work_root, python
        self._environment = dict(environment)
        self._timeout = timeout_seconds
        self._phase, self._commands, self._observations = 'new', [], []
        self._sources, self._results = [], []
        self._started = self._deadline = self._completed = None
        self._inventory = self._inventory_snapshot = None
        self._result_pin = None
        self._archive = archive
        self._candidate_json = canonical({name:candidate_identity[name] for name in
            ('head_commit','head_tree','workspace_source_manifest_sha256')})
        self._invocation_sha256 = invocation_sha256
        self._command_context_json = self._archive_index = self._archive_binding_json = None
        self._verification_completed_ns = None
        self._archive_units = []
        self._archive_caps = None

    def _capture(self, path, limit):
        return self._api.read_file(path, 'fixed scaling preflight input', maximum_bytes=limit)

    def _check_deadline(self):
        require(type(self._deadline) is int and time.monotonic_ns() < self._deadline,
            'original fixed scaling preflight deadline expired')

    def _check_sources(self):
        for owner in (self._blake3,self._pytest):
            if owner is not None: owner.verify()
        for row in self._sources:
            self._api.unchanged(row, 'fixed scaling preflight input', maximum_bytes=MAX_SOURCE_BYTES)
        self._api.unchanged(self._inventory_snapshot, 'fixed scaling preflight inventory',
            maximum_bytes=MAX_INVENTORY_BYTES)

    def _source_buffers(self):
        return {str(row.path.relative_to(self._root)):row.data for row in self._sources}

    def _phase_inputs(self):
        return phase_inputs(self._inventory, self._inventory_snapshot.sha256, self._source_buffers())

    def _validate_result(self, name, raw, expected):
        require(tuple(self._inventory['phases' if name in PHASE_COUNTS else 'pytest_suites'][name]) == expected)
        return validate_unit_result(name, raw, self._inventory,
            self._inventory_snapshot.sha256, self._source_buffers())

    def _publish(self, name, raw):
        self._check_deadline()
        observed = self._archive.write(name, raw, self._archive_caps[name])
        require(observed.sha256 == hashlib.sha256(raw).hexdigest() and observed.size == len(raw))
        self._check_deadline()
        return dict(sha256=observed.sha256, size_bytes=observed.size)

    def _capture_command_context(self):
        def paths(owner):
            original = owner.paths
            return dict(source_root=str(original.source_root), bundle_root=str(original.bundle_root),
                inventory=str(original.inventory), inventory_sha256=original.inventory_sha256)
        self._command_context_json = canonical(dict(python=str(self._python),
            repository_root=str(self._root), work_root=str(self._work),
            environment_sha256=hashlib.sha256(canonical(self._environment)).hexdigest(),
            pytest=paths(self._pytest), blake3=paths(self._blake3)))

    def _run_unit(self, name, expected):
        self._check_deadline(); self._check_sources()
        index = len(self._commands)
        kind = 'phase' if name in PHASE_COUNTS else 'pytest'
        context = json.loads(self._command_context_json)
        full_argv = unit_argv(index, kind, name, context)
        require(full_argv[0] == str(self._python) and context['repository_root'] == str(self._root)
            and context['work_root'] == str(self._work)
            and context['environment_sha256'] == hashlib.sha256(canonical(self._environment)).hexdigest())
        validate_command_inputs(full_argv, str(self._root))
        argv = full_argv[1:]
        result_path = self._work/('result-%03d.json' % index)
        command = self._api.command_owner()
        self._commands.append(command)
        result = self._api.run_bounded(self._python, argv, cwd=self._root,
            environment=dict(self._environment), timeout_seconds=self._timeout,
            maximum_output_bytes=MAX_OUTPUT_BYTES, pass_fds=(), observation=command,
            deadline_ns=self._deadline)
        terminal = command.terminal
        require(command._reaped is True and terminal is not None and terminal.returncode == 0
            and result.returncode == 0 and terminal.violations == () and terminal.descriptors == ()
            and terminal.argv == (str(self._python),*argv) and terminal.cwd == str(self._root)
            and terminal.environment_sha256 == hashlib.sha256(canonical(self._environment)).hexdigest()
            and terminal.deadline_ns == self._deadline and type(terminal.completed_ns) is int
            and self._started <= terminal.started_ns < terminal.completed_ns <= self._deadline)
        self._check_deadline(); self._check_sources()
        snapshot = self._capture(result_path, MAX_RESULT_BYTES)
        self._results.append(snapshot)
        self._validate_result(name, snapshot.data, expected)
        require(type(result.stdout) is bytes and type(result.stderr) is bytes
            and len(result.stdout) + len(result.stderr) <= MAX_OUTPUT_BYTES
            and terminal.stdout_bytes == len(result.stdout) and terminal.stderr_bytes == len(result.stderr)
            and terminal.stdout_sha256 == hashlib.sha256(result.stdout).hexdigest()
            and terminal.stderr_sha256 == hashlib.sha256(result.stderr).hexdigest())
        prefix = 'unit-%03d.' % index
        command_raw = project_unit_command(index, name, terminal)
        row = dict(index=index, kind=kind, name=name,
            command=self._publish(prefix+'command.json', command_raw),
            result=self._publish(prefix+'result.json', snapshot.data),
            stdout=self._publish(prefix+'stdout', result.stdout),
            stderr=self._publish(prefix+'stderr', result.stderr))
        self._archive_units.append(row)
        self._observations.append(PreflightUnitObservation(name,terminal,snapshot.sha256,expected))
        # CommandResult's bounded stream buffers leave scope here; the archive
        # retains exact files and metadata, never all units' output in memory.

    def run(self):
        require(self._phase == 'new')
        self._phase = 'running'
        self._started = time.monotonic_ns()
        self._deadline = self._started + self._timeout * 1_000_000_000
        try:
            self._inventory_snapshot = self._capture(self._root/INVENTORY_PATH, MAX_INVENTORY_BYTES)
            self._inventory = decode_inventory(self._inventory_snapshot.data)
            total = 0
            for name, digest in self._inventory['sources'].items():
                row = self._capture(self._root/name, MAX_SOURCE_BYTES)
                total += len(row.data)
                require(total <= MAX_SOURCE_TOTAL_BYTES, 'preflight total source bound exceeded')
                require(row.sha256 == digest, 'preflight source inventory changed')
                self._sources.append(row)
            self._archive_caps = member_caps(self._inventory)
            self._check_deadline()
            self._archive.create(self._archive_caps)
            self._publish('inventory.json', self._inventory_snapshot.data)
            self._work.mkdir(mode=0o700)
            contract = self._dependency_api
            contract.stage_dependency_source(self._dependency,self._work/'blake3-source')
            self._blake3 = contract.PythonDependencies.provision(self._work/'blake3-source',
                self._work/'blake3-bundle',self._work/'blake3-inventory.json')
            contract.stage_test_dependency_source(self._dependency,self._work/'dependencies-source')
            self._pytest = contract.PythonTestDependencies.provision(self._work/'dependencies-source',
                self._work/'dependencies-bundle',self._work/'dependencies-inventory.json')
            self._check_deadline(); self._check_sources()
            self._capture_command_context()
            for phase in PHASE_COUNTS:
                self._run_unit(phase, tuple(self._inventory['phases'][phase]))
            for suite, nodes in self._inventory['pytest_suites'].items():
                self._run_unit(suite, tuple(nodes))
            self._check_deadline(); self._check_sources()
            # Every required assertion must have a source-bound current owner
            # and an observed passing outcome in this original invocation.
            require(not self._inventory['pending_outcomes'], 'required preflight outcomes remain unresolved')
            self._result_pin = tuple(self._observations)
            self._verify_original_units()
            self._archive.verify()
            self._verification_completed_ns = time.monotonic_ns()
            self._check_deadline()
            self._archive_index = encode_index(json.loads(self._candidate_json),
                self._invocation_sha256,
                dict(timeout_seconds=self._timeout, original_started_ns=self._started,
                    deadline_ns=self._deadline, verification_completed_ns=self._verification_completed_ns),
                dict(sha256=self._inventory_snapshot.sha256, size_bytes=len(self._inventory_snapshot.data)),
                self._archive_units, command_context=json.loads(self._command_context_json))
            index = self._publish('index.json', self._archive_index)
            self._check_sources()
            self._archive.verify()
            self._completed = time.monotonic_ns()
            require(self._started < self._completed <= self._deadline,
                'original fixed scaling preflight deadline expired during archive publication')
            binding = dict(archive_id=ARCHIVE_ID,
                scope=dict(timeout_seconds=self._timeout,original_started_ns=self._started,
                    deadline_ns=self._deadline,completed_ns=self._completed),
                index=dict(**index,mode='0400'), inventory=archive_census(self._archive.rows))
            self._archive_binding_json = canonical(validate_binding(binding))
            self._phase = 'passed'
            self.verify()
            self._completed = time.monotonic_ns()
            require(self._completed <= self._deadline,
                'original preflight deadline expired during final retained archive check')
            binding['scope']['completed_ns'] = self._completed
            self._archive_binding_json = canonical(validate_binding(binding))
            return self._result_pin
        except BaseException:
            self._phase = 'failed'
            raise

    def _verify_original_units(self):
        require(self._result_pin is not None and tuple(self._observations) == self._result_pin
            and not self._inventory['pending_outcomes'])
        require(hashlib.sha256(self._inventory_snapshot.data).hexdigest() == self._inventory_snapshot.sha256
            and self._inventory == decode_inventory(self._inventory_snapshot.data))
        expected = ordered_units(self._inventory)
        require(tuple(row.name for row in self._result_pin) == tuple(row[1] for row in expected)
            and len(self._commands) == len(expected)
            and len(self._results) == len(expected) == len(self._archive_units))
        context = json.loads(self._command_context_json)
        for index, (command, observed, snapshot) in enumerate(zip(self._commands,self._result_pin,self._results)):
            terminal = command.terminal
            require(command._reaped is True and terminal is observed.terminal
                and terminal.returncode == 0 and terminal.violations == ()
                and terminal.descriptors == () and terminal.deadline_ns == self._deadline
                and self._started <= terminal.started_ns < terminal.completed_ns <= self._deadline
                and terminal.argv == unit_argv(index,expected[index][0],observed.name,context)
                and terminal.cwd == str(self._root)
                and terminal.environment_sha256 == context['environment_sha256'])
            require(hashlib.sha256(snapshot.data).hexdigest() == snapshot.sha256 == observed.result_sha256)
            self._validate_result(observed.name,snapshot.data,observed.node_ids)
            row = self._archive_units[index]
            raw = project_unit_command(index, observed.name, terminal)
            require(row == dict(index=index,kind=expected[index][0],name=observed.name,
                command=dict(sha256=hashlib.sha256(raw).hexdigest(),size_bytes=len(raw)),
                result=dict(sha256=snapshot.sha256,size_bytes=len(snapshot.data)),
                stdout=dict(sha256=terminal.stdout_sha256,size_bytes=terminal.stdout_bytes),
                stderr=dict(sha256=terminal.stderr_sha256,size_bytes=terminal.stderr_bytes)))

    def verify_retained(self):
        require(self._phase in ('passed','closed') and self._archive_binding_json is not None
            and type(self._completed) is int and self._started < self._completed <= self._deadline)
        self._verify_original_units()
        self._archive.verify()
        binding = validate_binding(json.loads(self._archive_binding_json))
        require(encode_index(json.loads(self._candidate_json),self._invocation_sha256,
            dict(timeout_seconds=self._timeout,original_started_ns=self._started,
                deadline_ns=self._deadline,verification_completed_ns=self._verification_completed_ns),
            dict(sha256=self._inventory_snapshot.sha256,
                size_bytes=len(self._inventory_snapshot.data)),self._archive_units,
            command_context=json.loads(self._command_context_json)) == self._archive_index)
        require(binding['inventory'] == archive_census(self._archive.rows)
            and binding['index']['sha256'] == hashlib.sha256(self._archive_index).hexdigest()
            and binding['index']['size_bytes'] == len(self._archive_index)
            and binding['scope'] == dict(timeout_seconds=self._timeout,
                original_started_ns=self._started,deadline_ns=self._deadline,completed_ns=self._completed))

    @property
    def archive_binding(self):
        """Project only after rejoining this owner's original terminal observations."""
        self.verify_retained()
        return json.loads(self._archive_binding_json)

    def verify(self):
        self.verify_retained(); self._check_sources()
        for row in self._results:
            self._api.unchanged(row, 'fixed scaling preflight result', maximum_bytes=MAX_RESULT_BYTES)

    def require_terminal(self):
        # Refuse release even after a wait/drain exception. None is no-spawn
        # only when Popen never returned an owned original process.
        require(all(command._process is None or
            (command._reaped is True and command.terminal is not None)
            for command in self._commands), 'original preflight child has no natural terminal observation')

    def close(self):
        self.require_terminal()
        was_passed = self._phase in ('passed','closed')
        try:
            try:
                if self._phase == 'passed': self.verify()
            finally:
                try:
                    if self._pytest is not None: self._pytest.close()
                finally:
                    if self._blake3 is not None: self._blake3.close()
        except BaseException:
            self._phase = 'failed'
            raise
        self._phase = 'closed' if was_passed else 'failed'

    def release(self):
        """Release archive custody only after every original child is terminal."""
        self.require_terminal()
        try:
            self.close()
        finally:
            self._archive.close()
