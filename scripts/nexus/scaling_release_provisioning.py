"""Prepare one parent-selected fixed collector invocation without launching it.

The protected parent retains this owner until its original process has naturally
been reaped. Socket requests and archived reports provide no provisioning input.
The child independently performs RuntimeAdmission under its own module origins.
TODO: wire this owner into the protected bootstrap's one-use handoff and final
publication join; this module alone establishes no release execution claim.
"""
from __future__ import annotations

from dataclasses import dataclass, fields
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import time

import compute_workspace_source_manifest as source_contract
import release_artifact_contract as artifact_contract
import sumeragi_v2_prebuilt_bundle as binary_contract
import write_sumeragi_v2_release_receipt as receipt_contract
import scaling_cli_bootstrap as python_contract
from scaling_experiment_cli_inputs import LAUNCH_SCHEMA, MAX_LAUNCH_BYTES, load_launch_value
from scaling_seed_pipe import DevelopmentSeedPipe

_NS = 1_000_000_000
_CONTROL_MAX = 8 * 1024 * 1024
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
_FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK


class ScalingProvisioningError(ValueError):
    """Closed provisioning failure without private paths, configuration or seed."""


class ScalingInputsBorrowedError(ScalingProvisioningError):
    """The original process owner has not declared its child reaped."""


def _require(value):
    if not value:
        raise ScalingProvisioningError('scaling_input_provisioning_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt): raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit): raise SystemExit(1) from None
    if isinstance(error, GeneratorExit): raise GeneratorExit() from None
    raise ScalingProvisioningError('scaling_input_provisioning_failed') from None


def _metadata(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _pin(fd, directory=False):
    info = _metadata(os.fstat(fd))
    return (info[:5] if directory else info,
            fcntl.fcntl(fd, fcntl.F_GETFL), fcntl.fcntl(fd, fcntl.F_GETFD))


def _close_original(fd, pin, directory=False):
    try:
        if _pin(fd, directory) == pin:
            os.close(fd)
    except OSError:
        pass


def _path(value):
    return python_contract._path(value)


def _separate(left, right):
    _require(left != right and left not in right.parents and right not in left.parents)


class _OriginalInputs:
    """Original no-follow edges and bounded files, including descriptor flags."""
    def __init__(self):
        self.directories, self.files, self.closed = {}, [], False

    def directory(self, path):
        _require(not self.closed)
        current, parent = Path('/'), None
        for offset, name in enumerate(_path(path).parts):
            current = Path('/') if offset == 0 else current / name
            if current not in self.directories:
                _require(len(self.directories) < 1024)
                fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
                pin = None
                try:
                    pin = _pin(fd, True)
                    _require(stat.S_ISDIR(pin[0][2]))
                    self.directories[current] = (fd, parent, name, pin)
                except BaseException:
                    if pin is not None: _close_original(fd, pin, True)
                    raise
            parent = self.directories[current][0]
        return parent

    def hold(self, path, maximum, expected=None, *, payload=True):
        _require(not self.closed and len(self.files) < 512
                 and type(maximum) is int and 0 < maximum <= binary_contract._MAX_BINARY_BYTES)
        parent = self.directory(_path(path).parent)
        fd = os.open(path.name, _FILE_FLAGS, dir_fd=parent)
        pin = None
        try:
            pin = _pin(fd)
            info = pin[0]
            _require(stat.S_ISREG(info[2]) and info[3] == os.geteuid()
                     and info[5] == 1 and info[2] & 0o7022 == 0
                     and 0 < info[6] <= maximum)
            digest, parts, offset = hashlib.sha256(), [], 0
            while offset < info[6]:
                piece = os.pread(fd, min(65536, info[6] - offset), offset)
                _require(type(piece) is bytes and 0 < len(piece) <= info[6] - offset)
                offset += len(piece); digest.update(piece)
                if payload: parts.append(piece)
            raw = b''.join(parts) if payload else None
            _require(expected is None or digest.hexdigest() == python_contract._digest(expected))
            _require(_pin(fd) == pin
                     and _metadata(os.stat(path.name, dir_fd=parent, follow_symlinks=False)) == info)
            self.files.append((fd, parent, path.name, pin))
            self.validate()
            return raw, fd
        except BaseException:
            if pin is not None and not any(row[0] == fd for row in self.files):
                _close_original(fd, pin)
            raise

    def validate(self):
        _require(not self.closed)
        for fd, parent, name, pin in self.directories.values():
            _require(_pin(fd, True) == pin
                     and _metadata(os.stat(name, dir_fd=parent, follow_symlinks=False))[:5] == pin[0])
        for fd, parent, name, pin in self.files:
            _require(_pin(fd) == pin
                     and _metadata(os.stat(name, dir_fd=parent, follow_symlinks=False)) == pin[0])

    def close(self):
        if self.closed: return
        self.closed = True
        for fd, _, _, pin in reversed(self.files): _close_original(fd, pin)
        for fd, _, _, pin in reversed(tuple(self.directories.values())):
            _close_original(fd, pin, True)


@dataclass(frozen=True, slots=True)
class ParentScalingSelection:
    """Trusted parent choices; never decoded from the gate channel.

    Launch time/output caps derive only from the admitted plan and budget after
    dependency loading. The parent selects only the bounded observation overhead.
    """
    source_root: Path
    source_paths: Path
    source_paths_sha256: str
    workspace_source_sha256: str
    cargo_target_root: Path
    artifact_root: Path
    binary_bundle: Path
    binary_manifest_sha256: str
    rustc_version: Path
    python_evidence: Path
    python_runtime_binding: Path
    python_runtime_binding_sha256: str
    installed_dependencies: Path
    plan_path: Path
    plan_sha256: str
    budget_path: Path
    budget_sha256: str
    control_root: Path
    evidence_root: Path
    runtime_root: Path
    machine_id: str
    storage_model: str
    source_revision: str
    environment: tuple[tuple[str, str], ...]
    observation_overhead_seconds: int


@dataclass(frozen=True, slots=True)
class PreparedScalingLaunch:
    """Immutable process-owner arguments; contains descriptor numbers, no seed."""
    python: Path
    source_root: Path
    argv: tuple[str, ...]
    launch_input_fd: int
    launch_input_sha256: str
    seed_fd: int
    cwd: Path
    environment: tuple[tuple[str, str], ...]
    timeout_seconds: int
    maximum_output_bytes: int
    evidence_root: Path
    manifest_max_bytes: int
    report_max_bytes: int
    original_started_ns: int
    deadline_ns: int


@dataclass(frozen=True, slots=True)
class PreparedScalingVerification:
    """Fresh reads of retained parent policy and the admitted native identity.

    These bytes are data for the parent's fixed verifier, not a release verdict.
    No path or digest is learned from a handoff request or an archived report.
    """
    plan_bytes: bytes
    budget_bytes: bytes
    kagami: Path
    kagami_sha256: str
    workspace_source_sha256: str
    source_revision: str
    executable_images: tuple[tuple[str, str], ...]
    worker_sources: tuple[tuple[str, int, str], ...]
    machine_id: str
    storage_model: str


def _selection(value):
    _require(type(value) is ParentScalingSelection)
    copied = []
    for field in fields(ParentScalingSelection):
        item = getattr(value, field.name)
        if field.name.endswith('sha256'):
            python_contract._digest(item)
        elif field.name in ('machine_id', 'storage_model'):
            _require(type(item) is str and 0 < len(item) <= 512
                     and all(32 <= ord(char) < 127 for char in item))
        elif field.name == 'source_revision':
            _require(type(item) is str and re.fullmatch('[a-f0-9]{40}|[a-f0-9]{64}', item))
        elif field.name == 'environment':
            _require(type(item) is tuple and len(item) <= 64)
            keys = []
            for row in item:
                _require(type(row) is tuple and len(row) == 2
                         and all(type(part) is str for part in row)
                         and re.fullmatch('[A-Z][A-Z0-9_]{0,127}', row[0])
                         and len(row[1]) <= 4096 and '\0' not in row[1])
                keys.append(row[0])
            _require(keys == sorted(set(keys)))
        elif field.name.endswith(('seconds', 'bytes')):
            _require(type(item) is int and 0 < item <= 512 * 1024 * 1024)
        else:
            _path(item)
        copied.append(item)
    result = ParentScalingSelection(*copied)
    _require(result.observation_overhead_seconds <= 600)
    return result


class PreparedScalingInputs:
    """One-use retained provisioning owner, released only by the live parent."""
    def __init__(self, *args, **kwargs):
        raise ScalingProvisioningError('scaling_input_provisioning_failed')

    @classmethod
    def prepare(cls, selection: ParentScalingSelection, seed_hex: str):
        """Verify selected release inputs and provision the sole fixed launch."""
        owner = object.__new__(cls)
        owner._started = time.monotonic_ns()
        owner._files, owner._dependencies, owner._seed = _OriginalInputs(), None, None
        owner._original_files = owner._files
        owner._original_dependencies = owner._original_seed = None
        owner._state, owner._busy, owner._failed = 'preparing', False, False
        try:
            _require(cls is PreparedScalingInputs)
            owner._selection = selected = _selection(selection)
            owner._selection_pin = tuple(getattr(selected, field.name) for field in fields(ParentScalingSelection))
            _require(type(seed_hex) is str and re.fullmatch('[a-f0-9]{64}', seed_hex)
                     and seed_hex != '0' * 64
                     and all(seed_hex not in str(getattr(selected, field.name))
                             for field in fields(ParentScalingSelection)))
            owner._scope()
            for path in (selected.control_root.parent, selected.evidence_root.parent,
                         selected.runtime_root.parent, selected.source_root,
                         selected.cargo_target_root, selected.artifact_root,
                         selected.binary_bundle, selected.python_evidence,
                         selected.python_evidence / 'python-runtime'):
                owner._files.directory(path)
            parent = owner._files.directory(selected.control_root.parent)
            os.mkdir(selected.control_root.name, 0o700, dir_fd=parent)
            owner._files.directory(selected.control_root)
            owner._files.hold(selected.source_paths, source_contract._MAX_PATH_LIST_BYTES,
                              selected.source_paths_sha256)
            binary_raw, _ = owner._files.hold(selected.binary_bundle / binary_contract._MANIFEST_NAME,
                binary_contract._MAX_MANIFEST_BYTES, selected.binary_manifest_sha256)
            owner._binary_manifest_bytes = binary_raw
            owner._binary = binary_contract._parse_manifest(binary_raw)
            rustc, _ = owner._files.hold(selected.rustc_version, binary_contract._MAX_TOOL_VERSION_BYTES,
                                       owner._binary['rustc_version_sha256'])
            _require(rustc.endswith(b'\n') and b'\0' not in rustc and b'\r' not in rustc)
            rustc.decode('ascii')
            _require(owner._binary['host_triple'] == owner._binary['target_triple']
                     and owner._binary['host_triple'] in ('aarch64-apple-darwin', 'x86_64-apple-darwin')
                     and [line for line in rustc.splitlines() if line.startswith(b'host: ')]
                     == [('host: ' + owner._binary['host_triple']).encode('ascii')])
            for label, relative, _ in binary_contract._BINARIES:
                _require(owner._binary[label + '_relative_path'] == relative)
                owner._files.hold(selected.binary_bundle / relative,
                    binary_contract._MAX_BINARY_BYTES, owner._binary[label + '_sha256'], payload=False)
            runtime_raw, _ = owner._files.hold(selected.python_runtime_binding, _CONTROL_MAX,
                                              selected.python_runtime_binding_sha256)
            owner._framework = receipt_contract._decode_canonical_json(runtime_raw, 'scaling Python runtime')
            owner._framework_pin = None
            owner._source_pin = None
            owner._verify_release()
            projection = receipt_contract._framework_runtime_projection(owner._framework['records'],
                                                                          'scaling Python runtime')
            executable = [row for row in projection if row['path'] == 'bin/python3' and row['kind'] == 'file']
            _require(len(executable) == 1)
            python = selected.python_evidence / 'python-runtime/bin/python3'
            owner._files.hold(python, 512 * 1024 * 1024, executable[0]['sha256'], payload=False)
            owner._sources = python_contract.provision_python_sources(selected.source_root, selected.source_paths,
                selected.source_paths_sha256, selected.workspace_source_sha256,
                selected.control_root / 'python-sources', selected.control_root / 'python-sources.json',
                selected.control_root / 'worker-sources')
            python_contract.stage_dependency_source(selected.installed_dependencies,
                                                   selected.control_root / 'dependency-source')
            owner._dependencies = python_contract.PythonDependencies.provision(
                selected.control_root / 'dependency-source', selected.control_root / 'dependency-bundle',
                selected.control_root / 'dependency-inventory.json')
            owner._original_dependencies = owner._dependencies
            owner._dependencies.load()
            # The existing decoder owns native policy types and reaches BLAKE3.
            # Import it only after exact dependency admission in -I -B -S.
            from scaling_experiment_config import decode_fixed_inputs, MAX_CONFIG_BYTES
            from scaling_runtime_admission import ReleaseRuntimePaths, _python_manifest
            from scaling_experiment_cli_inputs import decode_launch_input
            plan_raw, plan_fd = owner._files.hold(selected.plan_path, MAX_CONFIG_BYTES, selected.plan_sha256)
            budget_raw, budget_fd = owner._files.hold(selected.budget_path, MAX_CONFIG_BYTES, selected.budget_sha256)
            owner._policy_fds = (plan_fd, budget_fd)
            owner._policy_fd_pin = owner._policy_fds
            plan, budget, _ = decode_fixed_inputs(plan_raw, budget_raw)
            # Derive exactly once from the canonical admitted policy. Source and
            # dependency preparation elapsed since _started consumes this same
            # scope; completing admission never starts a fresh experiment budget.
            timeout_seconds = ((plan.experiment_timeout_ns + _NS - 1) // _NS
                               + selected.observation_overhead_seconds)
            manifest_max_bytes = budget.control_budgets[0].max_bytes
            report_max_bytes = budget.control_budgets[1].max_bytes
            maximum_output_bytes = manifest_max_bytes + report_max_bytes
            owner._deadline = owner._started + timeout_seconds * _NS
            _require(time.monotonic_ns() < owner._deadline)
            sources = owner._sources
            source_raw, _ = owner._files.hold(sources.manifest, _CONTROL_MAX, sources.manifest_sha256)
            owner._python_manifest = _python_manifest(source_raw)
            for row in owner._python_manifest['files']:
                owner._files.hold(sources.root / row['path'], row['size'], row['sha256'])
                if row['path'] in {'scripts/nexus/' + name for name in python_contract._WORKERS}:
                    owner._files.hold(sources.worker_sources / Path(row['path']).name, row['size'], row['sha256'])
            paths = ReleaseRuntimePaths(selected.source_root, selected.source_paths,
                selected.source_paths_sha256, selected.cargo_target_root, selected.artifact_root,
                selected.binary_bundle, selected.binary_manifest_sha256, selected.rustc_version,
                sources.root, sources.manifest, sources.manifest_sha256, python_contract._ENTRYPOINT,
                selected.python_evidence, selected.python_runtime_binding, selected.python_runtime_binding_sha256)
            dependency = owner._dependencies.paths
            value = dict(schema=LAUNCH_SCHEMA,
                runtime_paths={field.name: str(getattr(paths, field.name)) for field in fields(ReleaseRuntimePaths)},
                python_dependencies={field.name: str(getattr(dependency, field.name))
                                     for field in fields(python_contract.PythonDependencyPaths)},
                plan=dict(path=str(selected.plan_path), sha256=selected.plan_sha256),
                budget=dict(path=str(selected.budget_path), sha256=selected.budget_sha256),
                evidence_root=str(selected.evidence_root), runtime_root=str(selected.runtime_root),
                worker_sources=str(sources.worker_sources),
                identity=dict(machine_id=selected.machine_id, storage_model=selected.storage_model,
                              source_revision=selected.source_revision))
            raw = json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode('ascii')
            _require(load_launch_value(raw) == value)
            decode_launch_input(raw)
            digest = hashlib.sha256(raw).hexdigest()
            launch_path = selected.control_root / 'launch.json'
            python_contract.bundle_contract._publish_inventory(launch_path, raw)
            _, launch_fd = owner._files.hold(launch_path, MAX_LAUNCH_BYTES, digest)
            _require(stat.S_IMODE(os.fstat(launch_fd).st_mode) == 0o400)
            owner._seed = DevelopmentSeedPipe(seed_hex)
            owner._original_seed = owner._seed
            argv = python_contract.fixed_scaling_argv(python, sources, launch_fd, digest, owner._seed.fd)
            owner._launch = PreparedScalingLaunch(python, sources.root, argv, launch_fd, digest,
                owner._seed.fd, sources.root, selected.environment, timeout_seconds,
                maximum_output_bytes, selected.evidence_root, manifest_max_bytes,
                report_max_bytes, owner._started, owner._deadline)
            owner._launch_pin = tuple(getattr(owner._launch, field.name) for field in fields(PreparedScalingLaunch))
            owner._state = 'prepared'
            owner.validate()
            return owner
        except BaseException as error:
            owner.close()
            _failure(error)

    def _scope(self):
        selected = self._selection
        outputs = (selected.control_root, selected.evidence_root, selected.runtime_root)
        for index, output in enumerate(outputs):
            for other in outputs[index + 1:]: _separate(output, other)
            for tree in (selected.source_root, selected.cargo_target_root, selected.binary_bundle,
                         selected.python_evidence, selected.installed_dependencies):
                _separate(output, tree)
            for control in (selected.source_paths, selected.rustc_version, selected.python_runtime_binding,
                            selected.plan_path, selected.budget_path):
                _require(output != control and output not in control.parents)
        # artifact_root is a container; an existing root with a fresh evidence
        # child is the intended release layout. Equal/ancestor outputs reject.
        for output in outputs:
            _require(output != selected.artifact_root and output not in selected.artifact_root.parents)
        self._fresh(outputs)

    def _fresh(self, outputs):
        for output in outputs:
            _require(not output.exists() and not output.is_symlink())

    def _source_metadata(self):
        selected = self._selection
        names = source_contract.read_source_path_list(selected.source_paths)
        rows = source_contract._inspect_source_members(selected.source_root, names)
        return tuple((name, kind, mode, payload, None if info is None else _metadata(info))
                     for name, kind, mode, payload, info in rows)

    def _verify_release(self):
        selected = self._selection
        _require(source_contract.workspace_source_manifest_from_exact_path_list(
            selected.source_root, selected.source_paths) == selected.workspace_source_sha256)
        binary_contract.validate_bundle(selected.source_root, selected.workspace_source_sha256,
            selected.cargo_target_root, selected.artifact_root, selected.binary_bundle,
            selected.binary_manifest_sha256)
        framework = tuple(receipt_contract._validate_framework_python_runtime(self._framework,
                                                                            selected.python_evidence))
        source = self._source_metadata()
        if self._source_pin is None:
            self._source_pin, self._framework_pin = source, framework
        else:
            _require(self._source_pin == source and self._framework_pin == framework)
        self._files.validate()

    def validate(self):
        """Recheck actual inputs without renewing the original observation scope."""
        try:
            _require(not self._failed and not self._busy and self._state in ('prepared', 'borrowed', 'reaped'))
            self._busy = True
            for field, pin in zip(fields(ParentScalingSelection), self._selection_pin, strict=True):
                item = getattr(self._selection, field.name)
                _require(type(item) is type(pin) and item == pin)
            for field, pin in zip(fields(PreparedScalingLaunch), self._launch_pin, strict=True):
                item = getattr(self._launch, field.name)
                _require(type(item) is type(pin) and item == pin)
            _require(self._files is self._original_files
                     and self._dependencies is self._original_dependencies
                     and self._seed is self._original_seed)
            self._dependencies.verify()
            self._seed.validate()
            self._verify_release()
            artifact_contract.verify_private_python_source_closure(self._sources.root,
                self._python_manifest, self._sources.manifest_sha256, owner_uid=os.geteuid())
            _require(set(artifact_contract.scan_inventory_paths(self._sources.worker_sources))
                     == set(python_contract._WORKERS))
            if self._state == 'prepared':
                self._fresh((self._selection.evidence_root, self._selection.runtime_root))
                _require(time.monotonic_ns() < self._deadline)
            _require(self._source_metadata() == self._source_pin)
            self._files.validate()
        except BaseException as error:
            self._failed = True
            _failure(error)
        finally:
            self._busy = False

    def claim_launch(self) -> PreparedScalingLaunch:
        """Borrow once before Popen; only its original owner may declare reaping."""
        self.validate()
        _require(self._state == 'prepared')
        _require(time.monotonic_ns() < self._deadline)
        self._state = 'borrowed'
        return self._launch

    def verification_inputs(self) -> PreparedScalingVerification:
        """Read original bounded policy descriptors without adopting new inputs."""
        self.validate()
        _require(self._policy_fds == self._policy_fd_pin)
        from scaling_experiment_config import MAX_CONFIG_BYTES
        raw = []
        for fd, expected in zip(self._policy_fds,
                (self._selection.plan_sha256, self._selection.budget_sha256), strict=True):
            rows = [row for row in self._files.files if row[0] == fd]
            _require(len(rows) == 1 and _pin(fd) == rows[0][3])
            size = rows[0][3][0][6]
            _require(0 < size <= MAX_CONFIG_BYTES)
            parts, offset = [], 0
            while offset < size:
                piece = os.pread(fd, min(65536, size - offset), offset)
                _require(type(piece) is bytes and 0 < len(piece) <= size - offset)
                parts.append(piece); offset += len(piece)
            value = b''.join(parts)
            _require(hashlib.sha256(value).hexdigest() == expected)
            raw.append(value)
        self.validate()
        _require(hashlib.sha256(self._binary_manifest_bytes).hexdigest()
                 == self._selection.binary_manifest_sha256
                 and binary_contract._parse_manifest(self._binary_manifest_bytes) == self._binary)
        projection = receipt_contract._framework_runtime_projection(self._framework['records'],
                                                                    'scaling Python runtime')
        python = [row['sha256'] for row in projection
                  if row['path'] == 'bin/python3' and row['kind'] == 'file']
        _require(len(python) == 1)
        workers = {row['path']: row for row in self._python_manifest['files']}
        source_rows = tuple((name, workers['scripts/nexus/' + name]['size'],
                            workers['scripts/nexus/' + name]['sha256'])
                            for name in python_contract._WORKERS)
        return PreparedScalingVerification(raw[0], raw[1],
            self._selection.binary_bundle / self._binary['kagami_relative_path'],
            self._binary['kagami_sha256'], self._selection.workspace_source_sha256,
            self._selection.source_revision,
            (('kagami', self._binary['kagami_sha256']), ('cli', self._binary['iroha_sha256']),
             ('daemon', self._binary['irohad_sha256']), ('resource_program', python[0])),
            source_rows, self._selection.machine_id, self._selection.storage_model)

    def declare_child_reaped(self):
        """Trusted in-process action after original wait, or captured no-spawn.

        This is not an archive constructor, receipt field or supplied success
        flag. The protected process owner's finally path alone calls it, after
        it has finished naturally owning/draining/waiting for its original child.
        """
        _require(self._state == 'borrowed')
        self._state = 'reaped'

    def close(self):
        """Preserve pending-child inputs and foreign reused descriptor slots."""
        if self._state == 'borrowed':
            raise ScalingInputsBorrowedError('scaling_inputs_original_child_not_reaped')
        if self._state == 'closed': return
        self._state, self._failed = 'closed', True
        try:
            if self._original_seed is not None: self._original_seed.close()
        finally:
            try:
                if self._original_dependencies is not None: self._original_dependencies.close()
            finally:
                self._original_files.close()
