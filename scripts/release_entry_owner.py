"""Structured release-pipeline entry; no environment-selected executable authority.

The pipeline supplies independently authenticated executable and provenance digests,
its reviewed canonical source modules, and exact invocation roots. Python retains
read-only originals only. The native Rust entry owns every private allocation,
lock, build and seal. The maintained pipeline calls this owner for native
release-program preparation.
The reviewed pipeline supplies the independently admitted shared Rust process
bridge on every admitted host; Unix root observation does not establish descendant completion.
"""
from __future__ import annotations

from contextlib import ExitStack, nullcontext
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import struct
import stat
import sys

from sumeragi_prebuilt_bundle import PrebuiltBundleError, _acknowledgement, _path

_DIGEST = re.compile(r'[0-9a-f]{64}')
_MAX_ENTRY = 2 * 1024 * 1024 * 1024
_MAX_PROVENANCE = 512 * 1024


@dataclass(frozen=True)
class EntryPins:
    """Independent pipeline trust, never hashes calculated and adopted from inputs."""
    executable_sha256: str
    provenance_sha256: str
    source_manifest_sha256: str
    native_target: str

    def validate(self):
        for value in (self.executable_sha256, self.provenance_sha256, self.source_manifest_sha256):
            if not isinstance(value, str) or _DIGEST.fullmatch(value) is None:
                raise PrebuiltBundleError('pipeline entry pin is not lowercase SHA-256')
        _machine(self.native_target)


@dataclass(frozen=True)
class Invocation:
    """Explicit release operation; process environment supplies no artifact anchor."""
    repo_root: Path
    entry_root: Path
    cargo_root: Path
    artifact_root: Path
    cargo_home: Path
    cargo_bin: Path
    rustc_bin: Path
    cancel_request: Path
    name: str
    inherited_bundle: Path | None = None
    inherited_manifest: str | None = None

    def validate(self):
        for value in (self.repo_root, self.entry_root, self.cargo_root, self.artifact_root,
                      self.cargo_home, self.cargo_bin, self.rustc_bin, self.cancel_request):
            _path(str(value))
        if re.fullmatch(r'invocation\.[A-Za-z0-9]{6}', self.name) is None:
            raise PrebuiltBundleError('pipeline invocation name is not canonical')
        if (self.inherited_bundle is None) != (self.inherited_manifest is None):
            raise PrebuiltBundleError('pipeline inherited anchors are incomplete')
        if self.inherited_bundle is not None:
            _path(str(self.inherited_bundle))
            if _DIGEST.fullmatch(self.inherited_manifest) is None:
                raise PrebuiltBundleError('pipeline inherited manifest pin is invalid')


def _machine(target):
    machine = platform.machine().lower()
    x64 = machine in ('x86_64', 'amd64')
    arm64 = machine in ('aarch64', 'arm64')
    if sys.platform == 'win32':
        expected = 'x86_64-pc-windows-msvc' if x64 else 'aarch64-pc-windows-msvc' if arm64 else None
        if target != expected or expected is None:
            raise PrebuiltBundleError('entry provenance is not this native Windows process host')
        return 'pe', 0x8664 if x64 else 0xAA64
    if sys.platform.startswith('linux'):
        prefix = 'x86_64' if x64 else 'aarch64' if arm64 else None
        if prefix is None or target not in (prefix + '-unknown-linux-gnu', prefix + '-unknown-linux-musl'):
            raise PrebuiltBundleError('entry provenance is not this native Linux process host')
        return 'elf', 62 if x64 else 183
    raise PrebuiltBundleError('retained entry execution is not qualified on this host')


def _filename():
    return 'iroha_release_bundle.exe' if os.name == 'nt' else 'iroha_release_bundle'


def _unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise PrebuiltBundleError('entry provenance repeats a field')
        result[key] = value
    return result


def _provenance(raw, pins, environment_policy, *, expected_filename=None):
    if len(raw) > _MAX_PROVENANCE or hashlib.sha256(raw).hexdigest() != pins.provenance_sha256:
        raise PrebuiltBundleError('entry provenance differs from independent pipeline trust')
    try:
        value = json.loads(raw.decode('utf-8'), object_pairs_hook=_unique)
    except (ValueError, UnicodeError) as error:
        raise PrebuiltBundleError('entry provenance is not bounded UTF-8 JSON') from error
    expected = {'schema', 'source_manifest_sha256', 'cargo_lock_sha256', 'native_target', 'package_id',
                'executable_filename', 'executable_sha256', 'executable_bytes', 'cargo_tool', 'rustc_tool',
                'cargo_version', 'rustc_version', 'configurations', 'build_environment_policy_sha256',
                'bootstrap_build_environment'}
    if not isinstance(value, dict) or set(value) != expected:
        raise PrebuiltBundleError('entry provenance has an unknown or incomplete field inventory')
    if (value['schema'] != 'iroha.publisher.v1'
            or value['source_manifest_sha256'] != pins.source_manifest_sha256
            or value['native_target'] != pins.native_target
            or value['build_environment_policy_sha256'] != hashlib.sha256(environment_policy).hexdigest()
            or value['executable_filename'] != (expected_filename or _filename())
            or value['executable_sha256'] != pins.executable_sha256
            or type(value['executable_bytes']) is not int
            or not 0 < value['executable_bytes'] <= _MAX_ENTRY):
        raise PrebuiltBundleError('entry provenance does not bind the expected source, host and executable')
    def sha(candidate):
        return isinstance(candidate, str) and _DIGEST.fullmatch(candidate) is not None
    if not sha(value['cargo_lock_sha256']):
        raise PrebuiltBundleError('entry lock binding is not SHA-256')
    package = value['package_id']
    if not isinstance(package, str) or not 0 < len(package.encode('utf-8')) <= 16 * 1024 or any(ord(c) < 32 for c in package):
        raise PrebuiltBundleError('entry Cargo package identifier is invalid')
    for field in ('cargo_tool', 'rustc_tool'):
        tool = value[field]
        if (not isinstance(tool, dict) or set(tool) != {'bytes', 'sha256'} or not sha(tool['sha256'])
                or type(tool['bytes']) is not int or not 0 < tool['bytes'] <= _MAX_ENTRY):
            raise PrebuiltBundleError('entry tool byte binding is invalid')
    for field in ('cargo_version', 'rustc_version'):
        text = value[field]
        if (not isinstance(text, str) or not 0 < len(text.encode('utf-8')) <= 64 * 1024
                or not text.endswith('\n') or '\0' in text or '\r' in text):
            raise PrebuiltBundleError('entry tool transcript is invalid')
    if (not value['cargo_version'].startswith('cargo ')
            or not value['rustc_version'].startswith('rustc ')
            or value['rustc_version'].splitlines().count('host: ' + pins.native_target) != 1):
        raise PrebuiltBundleError('entry transcripts do not identify the native tools')
    configurations = value['configurations']
    if not isinstance(configurations, list) or not 0 < len(configurations) <= 4096:
        raise PrebuiltBundleError('entry configuration inventory is invalid')
    for config in configurations:
        if (not isinstance(config, dict) or set(config) != {'path', 'present', 'config_sha256', 'config_toml_sha256'}
                or type(config['present']) is not bool or not isinstance(config['path'], str)):
            raise PrebuiltBundleError('entry configuration binding is incomplete')
        _path(config['path'])
        for field in ('config_sha256', 'config_toml_sha256'):
            if config[field] is not None and (not config['present'] or not sha(config[field])):
                raise PrebuiltBundleError('entry configuration content binding is invalid')
    environment = value['bootstrap_build_environment']
    if not isinstance(environment, dict) or any(not isinstance(k, str) or not isinstance(v, str) for k, v in environment.items()):
        raise PrebuiltBundleError('entry build environment is invalid')
    policy = json.loads(environment_policy.decode('utf-8'), object_pairs_hook=_unique)
    allowed = set(policy['allowed']) | set(policy['owned'])
    if any(name not in allowed for name in environment):
        raise PrebuiltBundleError('entry provenance includes an unreviewed environment value')
    # Apply the same parser to all approved OS values and its size/timestamp bounds.
    _environment(environment_policy, environment)
    required = {'CARGO_BUILD_JOBS':'1', 'CARGO_INCREMENTAL':'0', 'RUSTUP_AUTO_INSTALL':'0',
                'ENABLE_RANS_BUNDLES':'1', 'NORITO_SKIP_BINDINGS_SYNC':'1'}
    if any(environment.get(name) != expected for name, expected in required.items()):
        raise PrebuiltBundleError('entry bootstrap environment differs from the owned build policy')
    for name in ('CARGO', 'RUSTC', 'CARGO_HOME', 'CARGO_TARGET_DIR'):
        if name not in environment:
            raise PrebuiltBundleError('entry bootstrap environment omits an owned path')
        _path(environment[name])
    if sum(len(k.encode('utf-8')) + len(v.encode('utf-8')) for k,v in environment.items()) > 256 * 1024:
        raise PrebuiltBundleError('entry configured environment exceeds its bound')
    return value


def _executable(reader, pins, size, *, library=False):
    if os.name == 'posix':
        metadata = os.fstat(reader.fileno())
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1
                or metadata.st_uid not in (0, os.geteuid())
                or metadata.st_mode & 0o7022 or not metadata.st_mode & stat.S_IXUSR):
            raise PrebuiltBundleError('entry executable has unsafe Unix ownership or access')
    digest = hashlib.sha256()
    prefix = bytearray()
    observed = 0
    while block := reader.read(64 * 1024):
        observed += len(block)
        if observed > size:
            raise PrebuiltBundleError('entry executable exceeds its trusted extent')
        digest.update(block)
        if len(prefix) < 1024 * 1024:
            prefix.extend(block[:1024 * 1024 - len(prefix)])
    if observed != size or digest.hexdigest() != pins.executable_sha256:
        raise PrebuiltBundleError('entry executable differs from independent pipeline trust')
    kind, machine = _machine(pins.native_target)
    if kind == 'elf':
        if (len(prefix) < 64 or prefix[:7] != b'\x7fELF\x02\x01\x01'
                or struct.unpack_from('<H', prefix, 16)[0] not in (2, 3)
                or struct.unpack_from('<H', prefix, 18)[0] != machine
                or (library and struct.unpack_from('<H', prefix, 16)[0] != 3)):
            raise PrebuiltBundleError('entry is not a supported native ELF executable')
    else:
        if len(prefix) < 64 or prefix[:2] != b'MZ':
            raise PrebuiltBundleError('entry is not a native PE executable')
        offset = struct.unpack_from('<I', prefix, 60)[0]
        if (offset < 64 or offset + 26 > len(prefix) or prefix[offset:offset + 4] != b'PE\0\0'
                or struct.unpack_from('<H', prefix, offset + 4)[0] != machine
                or struct.unpack_from('<H', prefix, offset + 24)[0] != 0x20B
                or bool(struct.unpack_from('<H', prefix, offset + 22)[0] & 0x2000) != library):
            raise PrebuiltBundleError('entry PE machine or optional header differs from native host')


def _environment(raw, inherited):
    """One shared reviewed policy, applied before the entry loader can see overrides."""
    if len(raw) > 64 * 1024:
        raise PrebuiltBundleError('build environment policy exceeds its bound')
    try:
        policy = json.loads(raw.decode('utf-8'), object_pairs_hook=_unique)
    except (ValueError, UnicodeError) as error:
        raise PrebuiltBundleError('build environment policy is not UTF-8 JSON') from error
    keys = {'schema', 'allowed', 'owned', 'ignored_names', 'ignored_prefixes', 'refused_names', 'refused_prefixes'}
    if not isinstance(policy, dict) or set(policy) != keys or policy['schema'] != 'iroha.release-build-environment.v1':
        raise PrebuiltBundleError('unknown build environment policy')
    for key in keys - {'schema'}:
        values = policy[key]
        if (not isinstance(values, list) or len(values) > 128 or any(not isinstance(value, str)
                or re.fullmatch(r'[A-Z0-9_()]{1,64}', value) is None for value in values)
                or len(set(values)) != len(values)):
            raise PrebuiltBundleError('build environment policy has invalid names')
    result = {}
    size = 0
    for key, value in inherited.items():
        name = key.upper() if os.name == 'nt' else key
        permitted = (name in policy['allowed'] or name in policy['owned'] or name in policy['ignored_names']
                     or any(name.startswith(prefix) for prefix in policy['ignored_prefixes']))
        if not permitted and (name in policy['refused_names'] or any(name.startswith(prefix) for prefix in policy['refused_prefixes'])):
            raise PrebuiltBundleError(f'unsupported inherited compiler, linker or Cargo override: {name}')
        if name == 'RUSTUP_AUTO_INSTALL' and value != '0':
            raise PrebuiltBundleError('caller rustup auto-install policy is not disabled')
        if name not in policy['allowed']:
            continue
        if len(value.encode('utf-8')) > 64 * 1024 or '\0' in value:
            raise PrebuiltBundleError('reviewed build environment value exceeds its bound')
        if name == 'SOURCE_DATE_EPOCH' and (not value or not value.isascii() or not value.isdigit()
                or len(value) > 20 or int(value) > 2**64 - 1):
            raise PrebuiltBundleError('SOURCE_DATE_EPOCH is not a bounded unsigned decimal')
        size += len(name.encode('utf-8')) + len(value.encode('utf-8'))
        if size > 256 * 1024 or name in result:
            raise PrebuiltBundleError('build environment is excessive or repeats a native name')
        result[name] = value
    result.update(CARGO_BUILD_JOBS='1', CARGO_INCREMENTAL='0', RUSTUP_AUTO_INSTALL='0')
    return result


class RootSecurity:
    """Read-only exact directory/ancestor security while Rust changes child contents.

    This reuses the canonical source owner's original types. It grants no creator,
    repair or private-permission admission; Rust must independently admit privacy.
    On Windows, the parent security latch also catches relax-and-restore of this
    exact root without confusing legitimate descendant creation with root mutation.
    """
    def __init__(self, path, custody, *, native_module=None):
        self.path = _path(str(path))
        self.originals = []
        self.watch = None
        self.failed = False
        self.closed = False
        original_type = custody._PosixOriginal
        native = None
        try:
            if os.name == 'nt':
                if native_module is None:
                    raise PrebuiltBundleError('reviewed native custody backend is required')
                native = native_module
                original_type = native.WindowsOriginal
                for parent in reversed((self.path, *self.path.parents)):
                    self.originals.append(original_type(parent, ancestor=True))
                native.require_local_ntfs(self.path, self.originals[-1])
                self.watch = native.SecurityLatch(self.path.parent)
            else:
                for parent in reversed((self.path, *self.path.parents)):
                    self.originals.append(original_type(parent, ancestor=True))
        except BaseException:
            self.close()
            raise
        try:
            self.revalidate()
        except BaseException:
            self.close()
            raise

    @property
    def identity(self):
        return self.originals[-1].identity

    @property
    def ancestry(self):
        return {original.identity for original in self.originals}

    def revalidate(self):
        if self.failed or self.closed:
            raise PrebuiltBundleError('output root security owner is closed or refused')
        try:
            if self.watch is not None:
                self.watch.revalidate()
            for original in self.originals:
                original.revalidate()
            if self.watch is not None:
                self.watch.revalidate()
        except BaseException:
            self.failed = True
            raise

    def close(self):
        self.closed = True
        if self.watch is not None:
            self.watch.close()
        for original in reversed(self.originals):
            original.close()

    def __enter__(self): return self
    def __exit__(self, kind, _error, _traceback):
        try:
            if kind is None: self.revalidate()
        finally:
            self.close()


def prepare_sumeragi_programs(invocation, pins, *, source_module, custody_module, environment_policy, native_module=None, owners=None, bridge_factory=None):
    """Run only independently pinned entry bytes while all original owners survive.

    `source_module` and `custody_module` come from the pipeline's reviewed bootstrap
    module registry, never from arguments or an environment-selected import path.
    No acknowledgement escapes until the original child and all context exits pass.
    """
    invocation.validate()
    pins.validate()
    if bridge_factory is None:
        raise PrebuiltBundleError('first entry requires the independently admitted process bridge')
    if os.name == 'nt' and native_module is None:
        raise PrebuiltBundleError('Windows first entry requires native source custody')
    if owners is not None and not isinstance(owners, ExitStack):
        raise PrebuiltBundleError('entry ownership must use the complete pipeline ExitStack')
    environment = _environment(environment_policy, os.environ)
    with (ExitStack() if owners is None else nullcontext(owners)) as stack:
        source = stack.enter_context(source_module.retained_workspace_source(invocation.repo_root, pins.source_manifest_sha256, custody_module=custody_module, native_module=native_module))
        entry = stack.enter_context(custody_module.SourceKeeper(invocation.entry_root, maximum_members=2, native_module=native_module))
        entry.capture([_filename(), 'publisher-source.json'])
        if set(os.listdir(invocation.entry_root)) != {_filename(), 'publisher-source.json'}:
            raise PrebuiltBundleError('native entry directory is not the exact two-entry artifact')
        entry.revalidate()
        roots = [stack.enter_context(RootSecurity(path, custody_module, native_module=native_module)) for path in
                 (invocation.repo_root, invocation.entry_root, invocation.cargo_root, invocation.artifact_root)]
        for index, root in enumerate(roots):
            for other in roots[index + 1:]:
                if root.identity in other.ancestry or other.identity in root.ancestry:
                    raise PrebuiltBundleError('entry/source/output roots physically overlap')
        with entry.reader(invocation.entry_root / 'publisher-source.json') as reader:
            provenance = reader.read(_MAX_PROVENANCE + 1)
        evidence = _provenance(provenance, pins, environment_policy)
        with source.reader(invocation.repo_root / 'Cargo.lock') as lock:
            lock_hash = hashlib.sha256()
            observed = 0
            while block := lock.read(64 * 1024):
                observed += len(block)
                if observed > 16 * 1024 * 1024:
                    raise PrebuiltBundleError('original source Cargo.lock exceeds its bound')
                lock_hash.update(block)
        if lock_hash.hexdigest() != evidence['cargo_lock_sha256']:
            raise PrebuiltBundleError('entry provenance differs from the independent original Cargo.lock')
        size = evidence['executable_bytes']
        with entry.reader(invocation.entry_root / _filename()) as executable:
            _executable(executable, pins, size)
            source.revalidate(); entry.revalidate()
            for root in roots: root.revalidate()
            if os.name == 'nt':
                program = str(invocation.entry_root / _filename())
            else:
                descriptor = executable.fileno()
                program = f'/proc/self/fd/{descriptor}'
                held = os.fstat(descriptor)
                selected = os.stat(program)
                if (held.st_dev, held.st_ino) != (selected.st_dev, selected.st_ino):
                    raise PrebuiltBundleError('descriptor execution projection differs from original')
            command = [program, 'bootstrap-build-publish']
            for name, value in (
                ('--repo-root', invocation.repo_root), ('--source-sha256', pins.source_manifest_sha256),
                ('--cargo-target-dir', invocation.cargo_root), ('--artifact-root', invocation.artifact_root),
                ('--cargo-home', invocation.cargo_home), ('--cargo-bin', invocation.cargo_bin),
                ('--rustc-bin', invocation.rustc_bin), ('--cancel-request', invocation.cancel_request),
                ('--name', invocation.name)):
                command.extend([name, str(value)])
            if invocation.inherited_bundle is not None:
                command.extend(['--bundle-dir', str(invocation.inherited_bundle),
                                '--manifest-sha256', invocation.inherited_manifest])
            bridge = bridge_factory(source, stack)
            acknowledgement = bridge.run({'program':program, 'directory':str(invocation.repo_root),
                'arguments':command[1:], 'environment':environment}, stack)
            source.revalidate(); entry.revalidate()
            for root in roots: root.revalidate()
            programs = invocation.artifact_root / 'sumeragi-release' / pins.source_manifest_sha256 / 'programs'
            result = _acknowledgement(acknowledgement, programs)
            if str(result[0]) != str(programs / invocation.name):
                raise PrebuiltBundleError('entry acknowledged a different invocation')
            if invocation.inherited_bundle is not None and (
                    str(result[0]) != str(invocation.inherited_bundle) or result[1] != invocation.inherited_manifest):
                raise PrebuiltBundleError('entry changed independent inherited anchors')
    return result
