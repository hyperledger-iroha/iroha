"""Retain the release inputs actually used by one fixed scaling experiment.

This composes the existing source-seal, prebuilt-binary, archived Python-runtime
and private Python-source/dependency contracts. Inherited digests locate release inputs;
they never substitute for reading and validating those inputs. Lab inventory
labels and the signed-release coordinator remain external trust boundaries.
Resource limits are fixed in the experiment plan and checked against observed
intervals by scaling_measurements; this is not a continuous host quota service.
"""
from __future__ import annotations

import ctypes
from dataclasses import dataclass, fields
import hashlib
import os
from pathlib import Path
import re
import stat
import sys

import compute_workspace_source_manifest as source_contract
import release_artifact_contract as artifact_contract
import sumeragi_v2_prebuilt_bundle as binary_contract
import write_sumeragi_v2_release_receipt as receipt_contract
from resource_process import ExecutableImage
from scaling_cli_bootstrap import PythonDependencies
from scaling_experiment_inputs import ExperimentIdentity, public_inputs
from scaling_experiment_plan import ExperimentPlan, plan_bytes
from scaling_fixed_trial import TrialRuntime
from scaling_structural_identity import pin_experiment_plan
from scaling_worker_sources import WorkerSourceFiles, SOURCE_NAMES

_MAX_CONTROL_BYTES = 8 * 1024 * 1024
_MAX_PYTHON_FILES = 256
_MAX_PYTHON_FILE_BYTES = 8 * 1024 * 1024
_DIRECTORY_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK
_FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK


class RuntimeAdmissionError(ValueError):
    """Closed failure excluding private filesystem and child output details."""


def _require(value):
    if not value:
        raise RuntimeAdmissionError('scaling_runtime_admission_failed')


def _failure(error):
    if isinstance(error, KeyboardInterrupt):
        raise KeyboardInterrupt() from None
    if isinstance(error, SystemExit):
        raise SystemExit(1) from None
    if isinstance(error, GeneratorExit):
        raise GeneratorExit() from None
    raise RuntimeAdmissionError('scaling_runtime_admission_failed') from None


def _path(value):
    _require(type(value) is type(Path('/')) and value.is_absolute()
             and str(value) == os.path.abspath(value)
             and len(value.parts) <= 64 and len(os.fsencode(value)) <= 4096)
    return value


def _digest(value):
    _require(type(value) is str and re.fullmatch(r'[0-9a-f]{64}', value))
    return value


def _metadata(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def _directory_metadata(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid)


@dataclass(frozen=True, slots=True)
class ReleaseRuntimePaths:
    """Original paths and inherited anchors supplied by the release coordinator.

    python_runtime_binding is the existing public framework-runtime projection,
    rendered using the receipt owner's canonical JSON. No new runtime schema
    or boolean qualification receipt is accepted.
    """
    source_root: Path
    source_paths: Path
    source_paths_sha256: str
    cargo_target_root: Path
    artifact_root: Path
    binary_bundle: Path
    binary_manifest_sha256: str
    rustc_version: Path
    python_sources: Path
    python_source_manifest: Path
    python_source_manifest_sha256: str
    python_entrypoint: str
    python_evidence: Path
    python_runtime_binding: Path
    python_runtime_binding_sha256: str


@dataclass(frozen=True, slots=True)
class HostObservation:
    """Direct host facts; machine inventory and storage labels are declarations."""
    cpu_model: str
    physical_cores: int
    logical_cores: int
    memory_bytes: int
    os: str
    kernel: str
    architecture: str
    python_version: str
    node_name: str


def observe_host() -> HostObservation:
    """Read the supported Darwin host through bounded kernel calls, no child."""
    _require(sys.platform == 'darwin')
    library = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
    read = library.sysctlbyname
    read.argtypes = (ctypes.c_char_p, ctypes.c_void_p,
                     ctypes.POINTER(ctypes.c_size_t), ctypes.c_void_p, ctypes.c_size_t)
    read.restype = ctypes.c_int

    def value(name, width=None):
        buffer = ctypes.create_string_buffer(512)
        size = ctypes.c_size_t(len(buffer))
        _require(read(name, buffer, ctypes.byref(size), None, 0) == 0
                 and 0 < size.value <= len(buffer))
        raw = buffer.raw[:size.value]
        if width is not None:
            _require(len(raw) == width)
            return int.from_bytes(raw, sys.byteorder)
        _require(raw.endswith(b'\0') and b'\0' not in raw[:-1])
        text = raw[:-1].decode('ascii')
        _require(text and all(32 <= ord(char) < 127 for char in text))
        return text

    uname = os.uname()
    result = HostObservation(value(b'machdep.cpu.brand_string'),
        value(b'hw.physicalcpu', 4), value(b'hw.logicalcpu', 4),
        value(b'hw.memsize', 8), 'macOS ' + value(b'kern.osproductversion'),
        uname.release, uname.machine,
        'Python ' + '.'.join(str(part) for part in sys.version_info[:3]), uname.nodename)
    _require(0 < result.physical_cores <= result.logical_cores <= 65_536
             and 0 < result.memory_bytes < 1 << 63)
    return result


class _RetainedFiles:
    """Small original-descriptor input set; no subprocesses or file mutation."""
    def __init__(self):
        self.directories = {}
        self.files = []
        self.closed = False

    def directory(self, path):
        _path(path)
        current, parent = Path('/'), None
        for offset, name in enumerate(path.parts):
            current = Path('/') if offset == 0 else current / name
            if current not in self.directories:
                _require(len(self.directories) < 1024)
                fd = os.open(name, _DIRECTORY_FLAGS, dir_fd=parent)
                try:
                    info = os.fstat(fd)
                    _require(stat.S_ISDIR(info.st_mode))
                    self.directories[current] = (fd, parent, name, _directory_metadata(info))
                except BaseException:
                    os.close(fd)
                    raise
            parent = self.directories[current][0]
        return parent

    def read(self, path, maximum, expected=None):
        _require(not self.closed and len(self.files) < _MAX_PYTHON_FILES + 16)
        parent = self.directory(_path(path).parent)
        fd = os.open(path.name, _FILE_FLAGS, dir_fd=parent)
        try:
            info = os.fstat(fd)
            _require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                     and info.st_uid == os.geteuid() and info.st_mode & 0o7022 == 0
                     and 0 < info.st_size <= maximum)
            pin = _metadata(info)
            raw = bytearray()
            while len(raw) < info.st_size:
                piece = os.pread(fd, min(65536, info.st_size - len(raw)), len(raw))
                _require(bool(piece) and len(piece) <= info.st_size - len(raw))
                raw.extend(piece)
            data = bytes(raw)
            _require(expected is None or hashlib.sha256(data).hexdigest() == _digest(expected))
            _require(_metadata(os.fstat(fd)) == pin
                     and _metadata(os.stat(path.name, dir_fd=parent, follow_symlinks=False)) == pin)
            self.files.append((fd, parent, path.name, pin))
            self.validate()
            return data
        except BaseException:
            if not any(row[0] == fd for row in self.files):
                os.close(fd)
            raise

    def validate(self):
        _require(not self.closed)
        for fd, parent, name, pin in self.directories.values():
            _require(_directory_metadata(os.fstat(fd)) == pin
                     and _directory_metadata(os.stat(name, dir_fd=parent, follow_symlinks=False)) == pin)
        for fd, parent, name, pin in self.files:
            _require(_metadata(os.fstat(fd)) == pin
                     and _metadata(os.stat(name, dir_fd=parent, follow_symlinks=False)) == pin)

    def close(self):
        if self.closed:
            return
        self.closed = True
        for fd, _, _, pin in reversed(self.files):
            try:
                if _metadata(os.fstat(fd))[:5] == pin[:5]:
                    os.close(fd)
            except OSError:
                pass
        for fd, _, _, pin in reversed(tuple(self.directories.values())):
            try:
                if _directory_metadata(os.fstat(fd)) == pin:
                    os.close(fd)
            except OSError:
                pass


def _python_manifest(raw):
    value = artifact_contract.load_json_object(raw, 'scaling Python sources')
    _require(artifact_contract.canonical_json_bytes(value) == raw
             and set(value) == {'schema', 'files'}
             and type(value['schema']) is str and len(value['schema']) <= 128
             and type(value['files']) is list and 1 <= len(value['files']) <= _MAX_PYTHON_FILES)
    for row in value['files']:
        _require(type(row) is dict and set(row) == {'path', 'sha256', 'size'}
                 and type(row['path']) is str and len(row['path']) <= 512
                 and row['path'].startswith('scripts/') and row['path'].endswith('.py')
                 and artifact_contract.canonical_relative_path(row['path']) == row['path']
                 and type(row['size']) is int and 0 < row['size'] <= _MAX_PYTHON_FILE_BYTES)
        _digest(row['sha256'])
    _require([row['path'] for row in value['files']] == sorted({row['path'] for row in value['files']}))
    return value


class RuntimeAdmission:
    """One live source-bound runtime, borrowed by FixedExperimentCustody.

    Call verify before each trial and after its complete public handoff, and
    after final report reconciliation. Use validate as the original frequent
    callback. Keep this owner and its borrowed images, workers and dependency owner alive
    throughout; closing admission does not close those borrowed owners.
    """
    def __init__(self, *args, **kwargs):
        raise RuntimeAdmissionError('scaling_runtime_admission_failed')

    @classmethod
    def admit(cls, paths: ReleaseRuntimePaths, identity: ExperimentIdentity,
              runtime: TrialRuntime, workers: WorkerSourceFiles, plan: ExperimentPlan,
              dependencies: PythonDependencies):
        """Read and authenticate original inputs; no receipt-only constructor."""
        _require(cls is RuntimeAdmission and type(paths) is ReleaseRuntimePaths
                 and type(identity) is ExperimentIdentity and type(runtime) is TrialRuntime
                 and type(workers) is WorkerSourceFiles and type(plan) is ExperimentPlan
                 and type(dependencies) is PythonDependencies)
        result = object.__new__(cls)
        result._failed = False
        result._busy = False
        result._files = _RetainedFiles()
        try:
            path_values = []
            for field in fields(ReleaseRuntimePaths):
                value = getattr(paths, field.name)
                if field.name.endswith('sha256'):
                    value = _digest(value)
                elif field.name == 'python_entrypoint':
                    _require(type(value) is str and len(value) <= 512
                             and artifact_contract.canonical_relative_path(value) == value
                             and value.startswith('scripts/') and value.endswith('.py'))
                else:
                    value = _path(value)
                path_values.append(value)
            result._paths = ReleaseRuntimePaths(*path_values)
            result._paths_pin = tuple(path_values)
            _require(paths.python_sources / 'scripts/nexus/scaling_runtime_admission.py'
                     == Path(__file__).absolute())
            result._dependencies = result._original_dependencies = dependencies
            dependencies.verify()
            dependencies.module
            result._identity = identity
            result._runtime = runtime
            result._workers = workers
            result._plan = plan
            result._plan_raw = plan_bytes(plan)
            result._plan_pin = pin_experiment_plan(plan, result._plan_raw)
            # This exact existing projector admits identity field types before
            # reading or comparing them; no generic copying invokes callbacks.
            public_inputs(identity, runtime, result._plan_raw, workers)
            result._identity_pin = tuple(getattr(identity, field.name) for field in fields(ExperimentIdentity))
            result._runtime_pin = tuple(getattr(runtime, name) for name in
                ('kagami', 'cli', 'daemon', 'resource_program'))
            result._worker_pin = (runtime.resource_worker, runtime.resource_worker_sha256, workers.pins)
            result._image_pins = tuple((image.path, image.fd, image.identity, image.sha256, image.uuids)
                                      for image in result._runtime_pin)
            result._host = observe_host()
            for field in fields(HostObservation):
                if field.name != 'node_name':
                    _require(getattr(identity, field.name) == getattr(result._host, field.name))
            for root in (paths.source_root, paths.cargo_target_root, paths.artifact_root,
                         paths.binary_bundle, paths.python_sources, paths.python_evidence,
                         paths.python_evidence / 'python-runtime'):
                result._files.directory(root)
            result._files.read(paths.source_paths, source_contract._MAX_PATH_LIST_BYTES, paths.source_paths_sha256)
            binary_raw = result._files.read(paths.binary_bundle / binary_contract._MANIFEST_NAME,
                binary_contract._MAX_MANIFEST_BYTES, paths.binary_manifest_sha256)
            result._binary = binary_contract._parse_manifest(binary_raw)
            result._binary_raw = binary_raw
            rustc = result._files.read(paths.rustc_version, binary_contract._MAX_TOOL_VERSION_BYTES,
                                      result._binary['rustc_version_sha256'])
            _require(rustc.endswith(b'\n') and b'\0' not in rustc and b'\r' not in rustc
                     and rustc.splitlines()[0].decode('ascii') == identity.rustc_version)
            triple = {'arm64': 'aarch64-apple-darwin',
                      'x86_64': 'x86_64-apple-darwin'}.get(identity.architecture)
            _require(triple is not None and result._binary['host_triple'] == triple
                     and result._binary['target_triple'] == triple
                     and [line for line in rustc.splitlines() if line.startswith(b'host: ')]
                     == [('host: ' + triple).encode('ascii')])
            python_raw = result._files.read(paths.python_source_manifest, _MAX_CONTROL_BYTES,
                                           paths.python_source_manifest_sha256)
            result._python = _python_manifest(python_raw)
            result._python_raw = python_raw
            _require(paths.python_entrypoint in {row['path'] for row in result._python['files']})
            for row in result._python['files']:
                result._files.read(paths.python_sources / row['path'], row['size'], row['sha256'])
            runtime_raw = result._files.read(paths.python_runtime_binding, _MAX_CONTROL_BYTES,
                                            paths.python_runtime_binding_sha256)
            result._python_runtime = receipt_contract._decode_canonical_json(runtime_raw, 'scaling Python runtime')
            result._python_runtime_raw = runtime_raw
            result._source_pin = result._framework_pin = None
            result.verify()
            return result
        except BaseException as error:
            result.close()
            _failure(error)

    def _check(self):
        _require(not self._failed and type(self._paths) is ReleaseRuntimePaths
                 and type(self._identity) is ExperimentIdentity
                 and type(self._runtime) is TrialRuntime and type(self._workers) is WorkerSourceFiles)
        # Exact primitive types precede equality, including allegedly frozen
        # dataclasses which can still be tampered with using object.__setattr__.
        for field, pin in zip(fields(ReleaseRuntimePaths), self._paths_pin, strict=True):
            value = getattr(self._paths, field.name)
            _require(type(value) is type(pin) and value == pin)
        for field, pin in zip(fields(ExperimentIdentity), self._identity_pin, strict=True):
            value = getattr(self._identity, field.name)
            _require(type(value) is type(pin) and value == pin)
        _require(type(self._dependencies) is PythonDependencies
                 and self._dependencies is self._original_dependencies)
        self._dependencies.validate()
        self._dependencies.module
        self._plan_pin.checked_bytes(self._plan, self._plan_raw)
        _require(all(getattr(self._runtime, name) is image for name, image in zip(
            ('kagami', 'cli', 'daemon', 'resource_program'), self._runtime_pin, strict=True)))
        for image, pin in zip(self._runtime_pin, self._image_pins, strict=True):
            _require(type(image) is ExecutableImage)
            image.validate()
            _require((image.path, image.fd, image.identity, image.sha256, image.uuids) == pin)
        _require(type(self._runtime.resource_worker) is type(Path('/'))
                 and type(self._runtime.resource_worker_sha256) is str
                 and (self._runtime.resource_worker, self._runtime.resource_worker_sha256,
                      self._workers.pins) == self._worker_pin)
        self._files.validate()
        _require(observe_host() == self._host)

    def validate(self):
        """Check original live inputs without rereading the whole release tree."""
        try:
            _require(not self._busy)
            self._busy = True
            self._check()
            self._files.validate()
        except BaseException as error:
            self._failed = True
            _failure(error)
        finally:
            self._busy = False

    def _source_metadata(self):
        paths = source_contract.read_source_path_list(self._paths.source_paths)
        rows = source_contract._inspect_source_members(self._paths.source_root, paths)
        return tuple((member, kind, mode, payload, None if info is None else _metadata(info))
                     for member, kind, mode, payload, info in rows)

    def verify(self):
        """Fully replay existing release verifiers under the original ownership."""
        try:
            _require(not self._busy)
            self._busy = True
            self._check()
            paths, identity = self._paths, self._identity
            self._dependencies.verify()
            _require(self._binary == binary_contract._parse_manifest(self._binary_raw)
                     and artifact_contract.canonical_json_bytes(self._python) == self._python_raw
                     and receipt_contract._canonical_json(self._python_runtime) == self._python_runtime_raw)
            _require(source_contract.workspace_source_manifest_from_exact_path_list(
                paths.source_root, paths.source_paths) == identity.workspace_source_sha256)
            binary_contract.validate_bundle(paths.source_root, identity.workspace_source_sha256,
                paths.cargo_target_root, paths.artifact_root, paths.binary_bundle,
                paths.binary_manifest_sha256)
            for role, label in (('kagami', 'kagami'), ('cli', 'iroha'), ('daemon', 'irohad')):
                image = getattr(self._runtime, role)
                _require(image.path == paths.binary_bundle / self._binary[label + '_relative_path']
                         and image.sha256 == self._binary[label + '_sha256'])
            artifact_contract.verify_private_python_source_closure(paths.python_sources,
                self._python, paths.python_source_manifest_sha256, owner_uid=os.geteuid(),
                entrypoint=paths.python_entrypoint, require_isolated_runtime=True)
            for row in self._python['files']:
                found = artifact_contract.stable_hash_relative(paths.source_root, row['path'],
                                                               max_size=_MAX_PYTHON_FILE_BYTES)
                _require(found.sha256 == row['sha256'] and found.size == row['size'])
            python_rows = {row['path']: row for row in self._python['files']}
            for name, pin in zip(SOURCE_NAMES, self._workers.pins, strict=True):
                source = python_rows.get('scripts/nexus/' + name)
                _require(source is not None and source['sha256'] == pin.sha256 and source['size'] == pin.bytes)
            framework = receipt_contract._validate_framework_python_runtime(self._python_runtime,
                                                                            paths.python_evidence)
            runtime_root = paths.python_evidence / 'python-runtime'
            _require(self._runtime.resource_program.path == runtime_root / 'bin/python3'
                     and Path(sys.executable).resolve(strict=True) == self._runtime.resource_program.path
                     and all(Path(prefix) == runtime_root for prefix in
                             (sys.prefix, sys.exec_prefix, sys.base_prefix, sys.base_exec_prefix)))
            source_pin = self._source_metadata()
            if self._source_pin is None:
                self._source_pin = source_pin
                self._framework_pin = tuple(framework)
            else:
                _require(source_pin == self._source_pin and tuple(framework) == self._framework_pin)
            self._check()
            # All full reads precede the final source metadata and retained-FD
            # fence. A later verifier cannot rewrite an earlier source member.
            _require(self._source_metadata() == self._source_pin)
            self._files.validate()
        except BaseException as error:
            self._failed = True
            _failure(error)
        finally:
            self._busy = False

    @property
    def identity(self):
        """The original bounded hardware/build identity."""
        self.validate()
        return self._identity

    @property
    def runtime(self):
        """Original retained executable owners for the fixed native pipeline."""
        self.validate()
        return self._runtime

    @property
    def worker_sources(self):
        """The original five-file worker source owner."""
        self.validate()
        return self._workers

    @property
    def plan(self):
        """The fixed ten-run plan, including unchanged resource thresholds."""
        self.validate()
        return self._plan

    def close(self):
        """Close only this owner's descriptors, preserving all borrowed inputs."""
        self._failed = True
        self._files.close()
